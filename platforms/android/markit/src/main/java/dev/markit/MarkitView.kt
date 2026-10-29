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
    var onError: (Throwable) -> Unit = {}
    /** A virtualized block. [spans] holds only the spans intersecting it, bucketed
     * once per document, so binding never scans the whole document. Code and
     * table panes are built once per row and reattached when they scroll back. */
    private class Row(val docIndex: Int, val document: MarkitDocument, val start: Int, val end: Int, val kind: String, val spans: List<MarkitSpan>) {
        val isPane: Boolean get() = kind == "code" || kind == "table"
        var pane: LinearLayout? = null; var paneWidth = -1
    }
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
    companion object {
        private val pool = Executors.newFixedThreadPool(2)
        private const val TEXT = 0; private const val PANE = 1
        private const val CODE_LINE = 2; private const val TABLE_ROW = 3
    }
    /** Code lines and table rows from every pane share one pool of row views. */
    private val paneRows = RecycledViewPool().apply { setMaxRecycledViews(CODE_LINE,40); setMaxRecycledViews(TABLE_ROW,20) }
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
                    if (stream == null || !job.source.startsWith(previous)) { stream?.close(); stream = MarkitStream(); previous = ""; parsedDocuments.clear() }
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
        val oldCount = rows.size
        rows.subList(first,rows.size).clear()
        for (index in changedFrom until documents.size) rows.addAll(rowsFor(documents[index],index))
        // Rows that keep their position are rebound in place, so a streaming
        // block updates its existing views instead of being removed and recreated.
        val shared = minOf(oldCount,rows.size)
        if (shared > first) blockAdapter.notifyItemRangeChanged(first,shared-first)
        if (oldCount > rows.size) blockAdapter.notifyItemRangeRemoved(rows.size,oldCount-rows.size)
        if (rows.size > oldCount) blockAdapter.notifyItemRangeInserted(oldCount,rows.size-oldCount)
    }
    /** Splits a document's blocks into rows and buckets its spans by row once. */
    private fun rowsFor(document: MarkitDocument, index: Int): List<Row> {
        val ranges = ArrayList<Pair<Int,Int>>(); val kinds = ArrayList<String>()
        for (block in document.blocks) {
            var start = block.start
            if (block.kind != "text" || block.end - start <= 3000) { ranges.add(Pair(start,block.end)); kinds.add(block.kind); continue }
            while (start < block.end) {
                var end = minOf(block.end,start+2500)
                if (end < block.end) {
                    var split = end
                    while (split > start+1500 && !document.text[split-1].isWhitespace()) split--
                    if (split > start+1500) end = split
                    if (document.text[end-1].isHighSurrogate()) end--
                }
                ranges.add(Pair(start,end)); kinds.add("text"); start = end
            }
        }
        val sorted = (1 until ranges.size).all { ranges[it-1].second <= ranges[it].first }
        val buckets = if (sorted) spansByLine(document.spans,ranges) else null
        return ranges.indices.map { i ->
            val (start,end) = ranges[i]
            Row(index,document,start,end,kinds[i],buckets?.get(i) ?: document.spans.filter { maxOf(it.start,start) < minOf(it.end,end) })
        }
    }
    private inner class Blocks : Adapter<Holder>() {
        override fun getItemCount() = rows.size
        override fun getItemViewType(position: Int) = if (rows[position].isPane) PANE else TEXT
        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): Holder {
            val root = LinearLayout(context).apply {
                orientation = LinearLayout.VERTICAL; setPadding(context.dp(12),context.dp(8),context.dp(12),context.dp(8))
                layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT)
            }
            if (viewType == PANE) return Holder(root)
            val text = TextView(context); text.textSize = 16f
            enableSourceCopy(text); text.movementMethod = LinkMovementMethod.getInstance()
            root.addView(text)
            return Holder(root,code = text)
        }
        override fun onBindViewHolder(holder: Holder, position: Int) {
            val root = holder.root; val row = rows[position]
            if (row.isPane) {
                val inset = row.spans.filter { (it.kind == "list_item" || it.kind == "quote") && it.start <= row.start && it.end >= row.end }.maxOfOrNull { it.number("depth") } ?: 0
                root.setPadding(context.dp(12 + inset*16),context.dp(8),context.dp(12),context.dp(8))
                val width = this@MarkitView.width
                val pane = row.pane?.takeIf { row.paneWidth == width } ?: rowsPane(row).also { row.pane = it; row.paneWidth = width }
                if (pane.parent !== root) {
                    root.removeAllViews()
                    (pane.parent as? ViewGroup)?.removeView(pane)
                    root.addView(pane)
                }
                return
            }
            val text = holder.code!!
            while (root.childCount > 1) root.removeViewAt(root.childCount - 1)
            text.setTextColor(context.ink())
            text.text = styled(row.document,row.start,row.end,context,onLink,onReference,row.spans)
            for (span in row.spans) {
                if (span.kind != "image") continue
                resolveImage?.invoke(span.string("url"))?.let { bitmap -> root.addView(ImageView(context).apply {
                    setImageBitmap(bitmap); adjustViewBounds = true; maxHeight = context.dp(320); scaleType = ImageView.ScaleType.FIT_CENTER
                    contentDescription = row.document.text.substring(span.start,span.end)
                }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,context.dp(240))) }
            }
        }
    }
    private class Holder(val root: LinearLayout, val number: TextView? = null, val code: TextView? = null) : ViewHolder(root)
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
        val cells = if (table) row.document.spans.filter { it.kind == "table_cell" && it.start >= row.start && it.end <= row.end }.groupBy { it.number("row") }.toSortedMap().values.map { it.sortedBy { cell -> cell.number("column") } } else emptyList()
        val lines = mutableListOf<Pair<Int,Int>>()
        if (!table) {
            var at = row.start
            while (at < row.end) { val newline = row.document.text.indexOf('\n',at).let { if (it < 0 || it >= row.end) row.end else it }; lines.add(Pair(at,newline)); at = newline + 1 }
        }
        val candidates = row.spans
        val lineSpans = if (!table && lines.size > 50 && candidates.size > lines.size) spansByLine(candidates, lines) else null
        val cellRanges = if (table) cells.flatten().map { Pair(it.start,it.end) } else emptyList()
        val cellSpans = if (cellRanges.size > 8 && (1 until cellRanges.size).all { cellRanges[it-1].second <= cellRanges[it].first }) spansByLine(candidates,cellRanges) else null
        val cellOffsets = IntArray(cells.size + 1)
        for (i in cells.indices) cellOffsets[i+1] = cellOffsets[i] + cells[i].size
        val count = if (table) cells.size else lines.size
        val rowHeight = context.dp(if (table) 48 else (22 * resources.configuration.fontScale).toInt())
        val width = maxOf(this@MarkitView.width - context.dp(24), if (table) (cells.firstOrNull()?.size ?: 1) * context.dp(160) else context.dp(minOf(12000,(lines.maxOfOrNull { it.second - it.first } ?: 0)*8+64)))
        val inner = RecyclerView(context); inner.layoutManager = LinearLayoutManager(context); inner.itemAnimator = null
        inner.setRecycledViewPool(paneRows)
        inner.contentDescription = if (table) "Markdown table" else "Source code"
        inner.adapter = object : Adapter<Holder>() {
            override fun getItemCount() = count
            override fun getItemViewType(position: Int) = if (table) TABLE_ROW else CODE_LINE
            override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): Holder {
                val root = LinearLayout(context).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL; layoutParams = LayoutParams(width,rowHeight) }
                if (viewType == TABLE_ROW) return Holder(root)
                val number = TextView(context); number.textSize = 11f; number.gravity = Gravity.END
                val text = TextView(context); text.setSingleLine(true); text.setPadding(context.dp(12),0,0,0)
                root.addView(number,LinearLayout.LayoutParams(context.dp(40),rowHeight))
                root.addView(text,LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT,rowHeight))
                return Holder(root,number,text)
            }
            override fun onBindViewHolder(holder: Holder, position: Int) {
                // Row views come from the shared pool, so they may have been sized by another pane.
                (holder.root.layoutParams as? LayoutParams)?.let { if (it.width != width || it.height != rowHeight) { it.width = width; it.height = rowHeight; holder.root.layoutParams = it } }
                if (table) {
                    val rowCells = cells[position]
                    while (holder.root.childCount > rowCells.size) holder.root.removeViewAt(holder.root.childCount - 1)
                    while (holder.root.childCount < rowCells.size) {
                        val text = TextView(context); text.textSize = 13f; text.maxLines = 2; text.setPadding(context.dp(8),0,context.dp(8),0)
                        holder.root.addView(text,LinearLayout.LayoutParams(context.dp(160),rowHeight))
                    }
                    for ((column,cell) in rowCells.withIndex()) {
                        val text = holder.root.getChildAt(column) as TextView
                        text.setTextColor(context.ink())
                        text.text = styled(row.document,cell.start,cell.end,context,onLink,onReference,cellSpans?.get(cellOffsets[position]+column) ?: candidates)
                        text.gravity = when(cell.string("alignment")) { "right" -> Gravity.END; "center" -> Gravity.CENTER; else -> Gravity.START }
                    }
                } else {
                    holder.number!!.apply { text = (position+1).toString(); setTextColor(context.ink()) }
                    holder.code!!.apply {
                        setTextColor(context.ink())
                        text = styled(row.document,lines[position].first,lines[position].second,context,onLink,onReference,lineSpans?.get(position) ?: candidates)
                    }
                }
            }
        }
        val scroll = HorizontalScrollView(context); scroll.addView(inner,ViewGroup.LayoutParams(width,minOf(context.dp(320),maxOf(rowHeight,count*rowHeight))))
        pane.addView(scroll); return pane
    }
    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        for (row in rows) row.pane = null
        blockAdapter.notifyItemRangeChanged(0,rows.size)
    }
    override fun close() {
        synchronized(processing) { closed = true; stream?.close(); stream = null }
        synchronized(lock) { pending = null }
    }
}
