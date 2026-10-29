package dev.markit

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.graphics.Bitmap
import android.text.TextUtils
import android.text.method.LinkMovementMethod
import android.util.AttributeSet
import android.view.Gravity
import android.view.ViewGroup
import android.widget.*
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import java.io.Closeable
import java.util.concurrent.Executors

/** Native block virtualization. Call close() when the owning screen is destroyed.
 * Image resolvers return already-authorized, decoded bytes; no network is used. */
class MarkitView @JvmOverloads constructor(context: Context, attrs: AttributeSet? = null) : RecyclerView(context,attrs), Closeable {
    var onReference: (MarkitReference) -> Unit = {}
    var onLink: (String) -> Unit = { url -> context.startActivity(Intent(Intent.ACTION_VIEW,Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
    var resolveImage: ((String) -> Bitmap?)? = null
    /** Dialect of the next render. Changing it renders the source again from a new stream. */
    @Volatile var flavor: MarkitFlavor = MarkitFlavor.DEFAULT
        set(value) { if (field != value) { field = value; lastRequested = null } }
    var onError: (Throwable) -> Unit = {}
    private data class Row(val docIndex: Int, val document: MarkitDocument, val start: Int, val end: Int, val kind: String)
    private data class Job(val source: String, val finalized: Boolean, val revision: Long)
    private val rows = mutableListOf<Row>()
    private val documents = mutableListOf<MarkitDocument>()
    private val blockAdapter = Blocks()
    private val lock = Any(); private val processing = Any()
    private var pending: Job? = null; private var running = false
    @Volatile private var closed = false
    private var revision = 0L; private var lastRequested: Pair<String,Boolean>? = null
    private val parsedDocuments = mutableListOf<MarkitDocument>()
    private var previous = ""; private var stream: MarkitStream? = null
    private var dirty = Int.MAX_VALUE
    companion object { private val pool = Executors.newFixedThreadPool(2) }
    init { layoutManager = LinearLayoutManager(context); adapter = blockAdapter; itemAnimator = null }

    fun render(source: String, finalized: Boolean = true) {
        check(android.os.Looper.myLooper() == android.os.Looper.getMainLooper()) { "Call render on the UI thread" }
        check(!closed) { "Renderer closed" }
        if (lastRequested == Pair(source,finalized)) return
        if (lastRequested == null) dirty = 0
        lastRequested = Pair(source,finalized); revision++
        synchronized(lock) {
            pending = Job(source,finalized,revision)
            if (!running) { running = true; pool.execute { drain() } }
        }
    }
    private fun drain() {
        while (true) {
            val job = synchronized(lock) { val next = pending; pending = null; if (next == null) running = false; next } ?: return
            try {
                val patch = synchronized(processing) {
                    if (closed) return
                    if (stream == null || stream!!.flavor != flavor || !job.source.startsWith(previous)) { stream?.close(); stream = MarkitStream(flavor); previous = ""; parsedDocuments.clear() }
                    val result = stream!!.append(job.source.substring(previous.length),job.finalized)
                    parsedDocuments.subList(result.replaceFrom,parsedDocuments.size).clear(); parsedDocuments.addAll(result.documents)
                    previous = job.source; Pair(result,parsedDocuments.toList())
                }
                post {
                    if (closed || lastRequested == null) return@post
                    documents.clear(); documents.addAll(patch.second)
                    dirty = minOf(dirty,patch.first.replaceFrom)
                    if (job.revision == revision) { display(dirty); dirty = Int.MAX_VALUE }
                }
            } catch (error: Throwable) { post { if (!closed && job.revision == revision) onError(error) } }
        }
    }
    fun setDocument(document: MarkitDocument) {
        check(android.os.Looper.myLooper() == android.os.Looper.getMainLooper()) { "Call setDocument on the UI thread" }
        revision++; lastRequested = null; documents.clear(); documents.add(document); dirty = Int.MAX_VALUE; display(0)
    }
    private fun display(changedFrom: Int) {
        val first = rows.indexOfFirst { it.docIndex >= changedFrom }.let { if (it < 0) rows.size else it }
        val removed = rows.size - first
        rows.subList(first,rows.size).clear()
        for (index in changedFrom until documents.size) {
            val document = documents[index]
            for (block in document.blocks) {
                var start = block.start
                if (block.kind != "text" || block.end - start <= 3000) { rows.add(Row(index,document,start,block.end,block.kind)); continue }
                while (start < block.end) {
                    var end = minOf(block.end,start+2500)
                    if (end < block.end) {
                        var split = end
                        while (split > start+1500 && !document.text[split-1].isWhitespace()) split--
                        if (split > start+1500) end = split
                        if (document.text[end-1].isHighSurrogate()) end--
                    }
                    rows.add(Row(index,document,start,end,"text")); start = end
                }
            }
        }
        if (removed > 0) blockAdapter.notifyItemRangeRemoved(first,removed)
        if (rows.size > first) blockAdapter.notifyItemRangeInserted(first,rows.size-first)
    }
    private inner class Blocks : Adapter<Holder>() {
        override fun getItemCount() = rows.size
        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int) = Holder(LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL; setPadding(context.dp(12),context.dp(8),context.dp(12),context.dp(8))
            layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT)
        })
        override fun onBindViewHolder(holder: Holder, position: Int) {
            val root = holder.root; root.removeAllViews(); val row = rows[position]
            val inset = if (row.kind == "code" || row.kind == "table") row.document.spans.filter { (it.kind == "list_item" || it.kind == "quote") && it.start <= row.start && it.end >= row.end }.maxOfOrNull { it.number("depth") } ?: 0 else 0
            root.setPadding(context.dp(12 + inset*16),context.dp(8),context.dp(12),context.dp(8))
            if (row.kind == "code" || row.kind == "table") {
                root.addView(rowsPane(row)); return
            }
            val text = TextView(context); text.setTextColor(context.ink()); text.textSize = 16f
            text.text = styled(row.document,row.start,row.end,context,onLink,onReference)
            enableSourceCopy(text); text.movementMethod = LinkMovementMethod.getInstance()
            root.addView(text)
            for (span in row.document.spans.filter { it.kind == "image" && it.start < row.end && it.end > row.start }) {
                resolveImage?.invoke(span.string("url"))?.let { bitmap -> root.addView(ImageView(context).apply {
                    setImageBitmap(bitmap); adjustViewBounds = true; maxHeight = context.dp(320); scaleType = ImageView.ScaleType.FIT_CENTER
                    contentDescription = row.document.text.substring(span.start,span.end)
                }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,context.dp(240))) }
            }
        }
    }
    private class Holder(val root: LinearLayout) : ViewHolder(root)
    private fun rowsPane(row: Row): LinearLayout {
        val pane = LinearLayout(context); pane.orientation = LinearLayout.VERTICAL
        val table = row.kind == "table"
        val header = LinearLayout(context); header.gravity = Gravity.CENTER_VERTICAL
        val meta = row.document.spans.firstOrNull { it.kind == "code_metadata" && it.start == row.start }
        val label = TextView(context); label.textSize = 12f; label.setTextColor(context.ink()); label.maxLines = 1; label.ellipsize = TextUtils.TruncateAt.START
        label.text = if (table) "Table" else meta?.string("file")?.ifEmpty { null } ?: row.document.spans.firstOrNull { it.kind == "code_block" && it.start == row.start }?.string("language") ?: "Code"
        header.addView(label, LinearLayout.LayoutParams(0,context.dp(40),1f))
        val copy = Button(context); copy.text = "Copy"; copy.contentDescription = if (table) "Copy table" else "Copy code"
        copy.setOnClickListener { (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("Code",row.document.source(row.start,row.end))); copy.text = "Copied" }
        header.addView(copy); pane.addView(header)
        val cells = row.document.spans.filter { it.kind == "table_cell" && it.start >= row.start && it.end <= row.end }.groupBy { it.number("row") }.toSortedMap().values.map { it.sortedBy { cell -> cell.number("column") } }
        val lines = mutableListOf<Pair<Int,Int>>()
        if (!table) {
            var at = row.start
            while (at < row.end) { val newline = row.document.text.indexOf('\n',at).let { if (it < 0 || it >= row.end) row.end else it }; lines.add(Pair(at,newline)); at = newline + 1 }
        }
        val count = if (table) cells.size else lines.size
        val rowHeight = context.dp(if (table) 48 else (22 * resources.configuration.fontScale).toInt())
        val width = maxOf(this@MarkitView.width - context.dp(24), if (table) (cells.firstOrNull()?.size ?: 1) * context.dp(160) else context.dp(minOf(12000,(lines.maxOfOrNull { it.second - it.first } ?: 0)*8+64)))
        val inner = RecyclerView(context); inner.layoutManager = LinearLayoutManager(context); inner.itemAnimator = null
        inner.contentDescription = if (table) "Markdown table" else "Source code"
        inner.adapter = object : Adapter<Holder>() {
            override fun getItemCount() = count
            override fun onCreateViewHolder(parent: ViewGroup, viewType: Int) = Holder(LinearLayout(context).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL; layoutParams = LayoutParams(width,rowHeight) })
            override fun onBindViewHolder(holder: Holder, position: Int) {
                holder.root.removeAllViews()
                if (table) {
                    for (cell in cells[position]) {
                        val text = TextView(context); text.setTextColor(context.ink()); text.textSize = 13f; text.maxLines = 2; text.setPadding(context.dp(8),0,context.dp(8),0)
                        text.text = styled(row.document,cell.start,cell.end,context,onLink,onReference)
                        text.gravity = when(cell.string("alignment")) { "right" -> Gravity.END; "center" -> Gravity.CENTER; else -> Gravity.START }
                        holder.root.addView(text,LinearLayout.LayoutParams(context.dp(160),rowHeight))
                    }
                } else {
                    val number = TextView(context); number.text = (position+1).toString(); number.textSize = 11f; number.gravity = Gravity.END; number.setTextColor(context.ink())
                    holder.root.addView(number,LinearLayout.LayoutParams(context.dp(40),rowHeight))
                    val text = TextView(context); text.setTextColor(context.ink()); text.setSingleLine(true); text.setPadding(context.dp(12),0,0,0)
                    text.text = styled(row.document,lines[position].first,lines[position].second,context,onLink,onReference)
                    holder.root.addView(text,LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT,rowHeight))
                }
            }
        }
        val scroll = HorizontalScrollView(context); scroll.addView(inner,ViewGroup.LayoutParams(width,minOf(context.dp(320),maxOf(rowHeight,count*rowHeight))))
        pane.addView(scroll); return pane
    }
    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig); blockAdapter.notifyItemRangeChanged(0,rows.size)
    }
    override fun close() {
        synchronized(processing) { closed = true; stream?.close(); stream = null }
        synchronized(lock) { pending = null }
    }
}
