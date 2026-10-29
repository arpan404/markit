package dev.markit.demo

import android.app.Activity
import android.os.Build
import android.util.Log
import android.view.View
import android.view.ViewGroup
import androidx.recyclerview.widget.RecyclerView
import dev.markit.Markit
import dev.markit.MarkitView
import org.json.JSONArray
import org.json.JSONObject

/** Opt-in app benchmark. Synchronous native viewport work, not GPU presentation. */
object MobileBenchmark {
    fun run(activity: Activity) {
        val sourceFile = if (activity.intent.getBooleanExtra("privateBenchmark",false)) java.io.File(activity.getExternalFilesDir(null), "benchmark-fixtures.json").inputStream() else activity.assets.open("fixtures.json")
        val fixtures = JSONObject(sourceFile.bufferedReader().use { it.readText() })
        val cases = JSONObject()
        val names = fixtures.keys().asSequence().sorted().toList()
        val width = activity.window.decorView.width
        val height = activity.window.decorView.height
        fun next(index: Int) {
            if (index == names.size) {
                val result = JSONObject().put("platform", "Android emulator").put("os", Build.VERSION.RELEASE)
                    .put("viewportWidth", width).put("viewportHeight", height).put("cases", cases)
                activity.filesDir.resolve("benchmark.json").writeText(result.toString(2))
                // Each fixture is emitted independently to stay below logcat's line limit.
                for (name in names) Log.i("MARKIT_BENCH", JSONObject().put("case", name).put("result", cases.get(name)).toString())
                Log.i("MARKIT_BENCH", "DONE")
                return
            }
            val name = names[index]; val source = fixtures.getString(name)
            val bridge = JSONArray(); val layout = JSONArray(); val scroll = JSONArray(); val codeScroll = JSONArray(); val tableScroll = JSONArray()
            repeat(10) {
                val start = System.nanoTime(); val document = Markit.parse(source)
                bridge.put((System.nanoTime() - start) / 1e6)
                val view = MarkitView(activity); activity.setContentView(view)
                val before = System.nanoTime()
                view.setDocument(document)
                view.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY))
                view.layout(0, 0, width, height)
                layout.put((System.nanoTime() - before) / 1e6)
                val scrolled = System.nanoTime()
                view.scrollToPosition((view.adapter?.itemCount ?: 1) - 1)
                view.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY))
                view.layout(0, 0, width, height)
                scroll.put((System.nanoTime() - scrolled) / 1e6)
                if (name.startsWith("code") || name.startsWith("table")) {
                    fun codePane(node: View): RecyclerView? {
                        if (node is RecyclerView && node !== view) return node
                        if (node is ViewGroup) for (i in 0 until node.childCount) codePane(node.getChildAt(i))?.let { return it }
                        return null
                    }
                    val code = checkNotNull(codePane(view)) { "Missing code viewport" }
                    val started = System.nanoTime()
                    repeat(8) {
                        code.scrollToPosition(code.adapter!!.itemCount - 1)
                        code.measure(View.MeasureSpec.makeMeasureSpec(code.width, View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(code.height, View.MeasureSpec.EXACTLY)); code.layout(code.left, code.top, code.right, code.bottom)
                        code.scrollToPosition(0)
                        code.measure(View.MeasureSpec.makeMeasureSpec(code.width, View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(code.height, View.MeasureSpec.EXACTLY)); code.layout(code.left, code.top, code.right, code.bottom)
                    }
                    val elapsed = (System.nanoTime() - started) / 1e6
                    if (name.startsWith("code")) codeScroll.put(elapsed) else tableScroll.put(elapsed)
                }
                view.close()
            }
            cases.put(name, JSONObject().put("sourceBytes", source.toByteArray().size).put("bridgeMs", bridge).put("viewportLayoutMs", layout).put("scrollToEndMs", scroll).put("codeScrollRoundTripsMs", codeScroll).put("tableScrollRoundTripsMs",tableScroll))
            activity.window.decorView.post { next(index + 1) }
        }
        next(0)
    }
}
