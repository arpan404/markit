package dev.markit.demo

import android.app.Activity
import android.os.Build
import android.util.Log
import android.view.View
import dev.markit.Markit
import dev.markit.MarkitView
import org.json.JSONArray
import org.json.JSONObject

/** Opt-in app benchmark. Synchronous native viewport work, not GPU presentation. */
object MobileBenchmark {
    fun run(activity: Activity) {
        val fixtures = JSONObject(activity.assets.open("fixtures.json").bufferedReader().use { it.readText() })
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
            val bridge = JSONArray(); val layout = JSONArray(); val scroll = JSONArray()
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
                view.close()
            }
            cases.put(name, JSONObject().put("sourceBytes", source.toByteArray().size).put("bridgeMs", bridge).put("viewportLayoutMs", layout).put("scrollToEndMs", scroll))
            activity.window.decorView.post { next(index + 1) }
        }
        next(0)
    }
}
