package dev.markit

import android.text.Spanned
import android.text.style.ForegroundColorSpan
import android.view.ContextThemeWrapper
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class CodeViewportTest {
    @Test fun tableRowsRetainLabelsAndReplaceTextAlignmentAndStyles() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        instrumentation.runOnMainSync {
            val context = ContextThemeWrapper(instrumentation.targetContext,android.R.style.Theme_Material_Light_NoActionBar)
            val source = "| Left | Center | Right |\n| :--- | :---: | ---: |\n" + List(150) { "| r$it | **b$it** | [link](https://example.com/$it) |" }.joinToString("\n")
            val view = MarkitView(context)
            var opened = ""
            view.onLink = { opened = it }
            try {
                view.setDocument(Markit.parse(source))
                view.measure(View.MeasureSpec.makeMeasureSpec(1080,View.MeasureSpec.EXACTLY),View.MeasureSpec.makeMeasureSpec(2200,View.MeasureSpec.EXACTLY)); view.layout(0,0,1080,2200)
                fun tableIn(node: View): RecyclerView? {
                    if (node is RecyclerView && node !== view) return node
                    if (node is ViewGroup) for (i in 0 until node.childCount) tableIn(node.getChildAt(i))?.let { return it }
                    return null
                }
                val table = checkNotNull(tableIn(view))
                val holder = table.getChildViewHolder(table.getChildAt(0))
                val root = holder.itemView as ViewGroup
                val labels = List(3) { root.getChildAt(it) as TextView }
                @Suppress("UNCHECKED_CAST")
                val adapter = table.adapter as RecyclerView.Adapter<RecyclerView.ViewHolder>
                for (row in listOf(150,0,75,1)) {
                    adapter.bindViewHolder(holder,row)
                    assertEquals(3,root.childCount)
                    for (column in 0..2) assertSame(labels[column],root.getChildAt(column))
                    assertEquals(if (row == 0) "Left" else "r${row-1}",labels[0].text.toString())
                    assertEquals(if (row == 0) "Center" else "b${row-1}",labels[1].text.toString())
                    assertEquals(android.view.Gravity.START,labels[0].gravity and android.view.Gravity.RELATIVE_HORIZONTAL_GRAVITY_MASK)
                    assertEquals(android.view.Gravity.CENTER_HORIZONTAL,labels[1].gravity and android.view.Gravity.RELATIVE_HORIZONTAL_GRAVITY_MASK)
                    assertEquals(android.view.Gravity.END,labels[2].gravity and android.view.Gravity.RELATIVE_HORIZONTAL_GRAVITY_MASK)
                    val spans = labels[1].text as Spanned
                    assertTrue(spans.getSpans(0,spans.length,android.text.style.StyleSpan::class.java).any { it.style == android.graphics.Typeface.BOLD })
                    if (row > 0) {
                        val links = labels[2].text as Spanned
                        links.getSpans(0,links.length,android.text.style.ClickableSpan::class.java).single().onClick(labels[2])
                        assertEquals("https://example.com/${row-1}",opened)
                    }
                }
            } finally { view.close() }
        }
    }

    @Test fun bucketsPreserveOverlappingSpansAndOrder() {
        val document = Markit.parse("```rust\nlet café = \"😀\";\n/* multi\nline */\n\n```\n")
        val lines = mutableListOf<Pair<Int,Int>>()
        var at = 0
        while (at < document.text.length) {
            val end = document.text.indexOf('\n',at).let { if (it < 0) document.text.length else it }
            lines.add(Pair(at,end)); at = end + 1
        }
        val buckets = spansByLine(document.spans, lines)
        lines.forEachIndexed { index, line ->
            assertEquals(document.spans.filter { maxOf(it.start,line.first) < minOf(it.end,line.second) }, buckets[index])
        }
    }

    @Test fun rebindingCodeRowsRetainsViewsAndMatchesFullScanColors() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        instrumentation.runOnMainSync {
            val context = ContextThemeWrapper(instrumentation.targetContext, android.R.style.Theme_Material_Light_NoActionBar)
            val lines = List(1000) { "let value_$it = \"😀\";" }
            val document = Markit.parse("```rust\n" + lines.joinToString("\n") + "\n```\n")
            val view = MarkitView(context)
            try {
                view.setDocument(document)
                view.measure(View.MeasureSpec.makeMeasureSpec(1080,View.MeasureSpec.EXACTLY),View.MeasureSpec.makeMeasureSpec(2200,View.MeasureSpec.EXACTLY))
                view.layout(0,0,1080,2200)
                fun codePane(node: View): RecyclerView? {
                    if (node is RecyclerView && node !== view) return node
                    if (node is ViewGroup) for (i in 0 until node.childCount) codePane(node.getChildAt(i))?.let { return it }
                    return null
                }
                val code = checkNotNull(codePane(view))
                assertTrue(code.childCount in 1..40)
                val holder = code.getChildViewHolder(code.getChildAt(0))
                val root = holder.itemView as ViewGroup
                val number = root.getChildAt(0) as TextView
                val text = root.getChildAt(1) as TextView
                @Suppress("UNCHECKED_CAST")
                val adapter = code.adapter as RecyclerView.Adapter<RecyclerView.ViewHolder>
                fun colors(value: Spanned) = value.getSpans(0,value.length,ForegroundColorSpan::class.java).map {
                    listOf(value.getSpanStart(it),value.getSpanEnd(it),it.foregroundColor)
                }
                for (position in listOf(999,0,500,1)) {
                    adapter.bindViewHolder(holder,position)
                    assertSame(number,root.getChildAt(0)); assertSame(text,root.getChildAt(1))
                    assertEquals((position+1).toString(),number.text.toString())
                    assertEquals(lines[position],text.text.toString())
                    val start = document.text.indexOf(lines[position])
                    val expected = styled(document,start,start+lines[position].length,context,{}, {})
                    assertEquals(colors(expected),colors(text.text as Spanned))
                }
            } finally { view.close() }
        }
    }

    @Test fun replacementsRebindBlockViewsInPlace() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        instrumentation.runOnMainSync {
            val context = ContextThemeWrapper(instrumentation.targetContext, android.R.style.Theme_Material_Light_NoActionBar)
            val view = MarkitView(context)
            fun layout() { view.measure(View.MeasureSpec.makeMeasureSpec(1080,View.MeasureSpec.EXACTLY),View.MeasureSpec.makeMeasureSpec(2200,View.MeasureSpec.EXACTLY)); view.layout(0,0,1080,2200) }
            fun firstText() = ((view.findViewHolderForAdapterPosition(0)!!.itemView as ViewGroup).getChildAt(0) as TextView)
            try {
                view.setDocument(Markit.parse("Hello\n\n```kotlin\nval a = 1\n```\n")); layout()
                val text = firstText()
                assertEquals("Hello",text.text.toString().trim())
                view.setDocument(Markit.parse("Hello world\n\n```kotlin\nval a = 2\n```\n")); layout()
                assertSame(text,firstText())
                assertEquals("Hello world",firstText().text.toString().trim())
                view.setDocument(Markit.parse("```kotlin\nval b = 3\n```\n\nTail")); layout()
                assertEquals("Tail",((view.findViewHolderForAdapterPosition(1)!!.itemView as ViewGroup).getChildAt(0) as TextView).text.toString().trim())
            } finally { view.close() }
        }
    }
}
