package dev.markit

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.res.Configuration
import android.graphics.BitmapFactory
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.BitmapDrawable
import android.text.Spannable
import android.text.SpannableString
import android.text.Spanned
import android.text.style.*
import android.util.Base64
import android.view.ActionMode
import android.view.Menu
import android.view.MenuItem
import android.view.View
import android.widget.TextView

internal class MathSpan(drawable: android.graphics.drawable.Drawable, val raw: String) : ImageSpan(drawable, ALIGN_BASELINE)
internal fun Context.dp(value: Int): Int = (resources.displayMetrics.density * value).toInt()
internal fun Context.ink(): Int = if (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES) Color.rgb(230,237,243) else Color.rgb(36,41,47)
internal fun styled(document: MarkitDocument, start: Int, end: Int, context: Context, onLink: (String) -> Unit, onReference: (MarkitReference) -> Unit): SpannableString {
    val output = SpannableString(document.text.substring(start, end))
    val dark = context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
    for (span in document.spans) {
        val from = maxOf(start, span.start) - start; val to = minOf(end, span.end) - start
        if (from >= to) continue
        fun apply(value: Any) { output.setSpan(value, from, to, Spannable.SPAN_EXCLUSIVE_EXCLUSIVE) }
        when (span.kind) {
            "heading" -> { apply(AbsoluteSizeSpan(intArrayOf(30,25,21,19,17,16)[(span.number("level") - 1).coerceIn(0,5)], true)); apply(StyleSpan(Typeface.BOLD)) }
            "strong", "table_header" -> apply(StyleSpan(Typeface.BOLD))
            "emphasis" -> apply(StyleSpan(Typeface.ITALIC))
            "strikethrough" -> apply(StrikethroughSpan())
            "code", "code_block", "table" -> { apply(TypefaceSpan("monospace")); apply(AbsoluteSizeSpan(13,true)) }
            "syntax" -> apply(ForegroundColorSpan(Color.parseColor(span.string(if (dark) "dark" else "light"))))
            "quote" -> { apply(QuoteSpan(context.ink())); apply(LeadingMarginSpan.Standard(context.dp(12))) }
            "list_item" -> apply(LeadingMarginSpan.Standard(context.dp(12)))
            "diff" -> when(span.string("change")) {
                "addition" -> apply(BackgroundColorSpan(0x222ea043)); "deletion" -> apply(BackgroundColorSpan(0x22f85149))
            }
            "link" -> apply(object : ClickableSpan() { override fun onClick(widget: View) { onLink(span.string("url")) } })
            "reference" -> apply(object : ClickableSpan() { override fun onClick(widget: View) { onReference(MarkitReference(span.string("reference_kind"), span.string("target"))) } })
            "math" -> {
                val bytes = Base64.decode(span.string("png"), Base64.DEFAULT)
                val bitmap = BitmapFactory.decodeByteArray(bytes,0,bytes.size) ?: continue
                val drawable = BitmapDrawable(context.resources,bitmap)
                drawable.setTint(context.ink())
                val width = maxOf(1, span.number("width") / 2); val height = maxOf(1, span.number("height") / 2)
                val scale = minOf(1f, 320f / width)
                drawable.setBounds(0,0,context.dp((width*scale).toInt()),context.dp((height*scale).toInt()))
                apply(MathSpan(drawable, span.string("raw")))
            }
        }
    }
    return output
}
internal fun sourceText(text: Spanned, start: Int, end: Int): String {
    val output = StringBuilder(); var at = start
    for (span in text.getSpans(start,end,MathSpan::class.java).sortedBy { text.getSpanStart(it) }) {
        val from = maxOf(at,text.getSpanStart(span)); output.append(text.subSequence(at,from)); output.append(span.raw); at = minOf(end,text.getSpanEnd(span))
    }
    output.append(text.subSequence(at,end)); return output.toString()
}
internal fun enableSourceCopy(view: TextView) {
    view.setTextIsSelectable(true)
    view.customSelectionActionModeCallback = object : ActionMode.Callback {
        override fun onCreateActionMode(mode: ActionMode, menu: Menu) = true
        override fun onPrepareActionMode(mode: ActionMode, menu: Menu) = false
        override fun onDestroyActionMode(mode: ActionMode) {}
        override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
            if (item.itemId != android.R.id.copy) return false
            val text = view.text as? Spanned ?: return false
            val start = minOf(view.selectionStart, view.selectionEnd).coerceAtLeast(0)
            val end = maxOf(view.selectionStart, view.selectionEnd).coerceAtMost(text.length)
            (view.context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("Markdown", sourceText(text,start,end)))
            mode.finish(); return true
        }
    }
}
