package dev.markit

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

/**
 * M2 frame-rate harness: JNI render+decode time distributions at three
 * document sizes through the real [Markit.parse] path (Rust FFI + JSONObject
 * decode, the same work every frame's data source performs).
 *
 * Author-only on this Mac: instrumentation tests require a device and none
 * is attached. This file adds no new dependencies (manual timing with
 * [System.nanoTime]; no androidx.benchmark) and changes no library behavior.
 *
 * Run (needs JDK 17, Android SDK 36, NDK; first build the JNI libraries):
 *   ./scripts/build-android.sh
 *   platforms/android/gradlew -p platforms/android \
 *     :markit:connectedDebugAndroidTest --tests "dev.markit.RenderBenchTest"
 * Pin a shared device first with ANDROID_SERIAL (see docs/validation.md).
 * Read the `MARKIT_PERF` lines in logcat (tag `MarkitBench`).
 *
 * Timing is printed, never asserted, so this harness cannot fail CI on a
 * slow device. Correctness assertions (schema version, expected span kinds)
 * run once per size.
 */
@RunWith(AndroidJUnit4::class)
class RenderBenchTest {
    companion object {
        private const val TAG = "MarkitBench"

        /** GFM section mirroring benchmarks/fixtures.mjs for cross-platform comparison. */
        private const val SECTION = "# Renderer benchmark\n" +
            "\n" +
            "A paragraph with **strong text**, *emphasis*, `inline code`, and a [documentation link](https://example.com/docs). Unicode stays intact: café \uD83E\uDD80.\n" +
            "\n" +
            "- First item with **bold**\n" +
            "- Second item with *emphasis*\n" +
            "\n" +
            "> A quoted paragraph with a [link](https://example.com).\n" +
            "\n" +
            "| Platform | Output |\n" +
            "| :--- | ---: |\n" +
            "| iOS | Native text |\n" +
            "| Android | Native text |\n" +
            "\n" +
            "```kotlin\n" +
            "fun greet(name: String): String {\n" +
            "    return \"Hello \$name \uD83E\uDD80\"\n" +
            "}\n" +
            "```\n" +
            "\n" +
            "Math \\(x^2\\) inline.\n" +
            "\n" +
            "---\n"

        /** Whole-section repeats reaching at least [target] UTF-8 bytes, keeping fences balanced. */
        private fun fixture(target: Int): String {
            val sectionBytes = SECTION.toByteArray().size
            val count = maxOf(1, (target + sectionBytes - 1) / sectionBytes)
            return SECTION.repeat(count)
        }

        private data class Stats(val medianMs: Double, val p95Ms: Double, val maxMs: Double)

        private fun percentile(values: List<Double>, p: Double): Double {
            val sorted = values.sorted()
            return sorted[minOf(sorted.size - 1, Math.ceil(sorted.size * p).toInt() - 1)]
        }

        private var sink = 0

        private fun timeIt(warmup: Int, iterations: Int, operation: () -> Int): Stats {
            repeat(warmup) { sink = sink xor operation() }
            val samples = mutableListOf<Double>()
            repeat(iterations) {
                val start = System.nanoTime()
                sink = sink xor operation()
                samples.add((System.nanoTime() - start) / 1_000_000.0)
            }
            return Stats(percentile(samples, 0.5), percentile(samples, 0.95), samples.max())
        }

        private fun checkRenderDecode(name: String, target: Int, warmup: Int, iterations: Int) {
            val source = fixture(target)
            var checked = false
            val stats = timeIt(warmup, iterations) {
                val document = Markit.parse(source)
                if (!checked) {
                    checked = true
                    assertEquals(2, document.version)
                    assertTrue(document.blocks.isNotEmpty())
                    assertTrue(document.spans.any { it.kind == "strong" })
                    assertTrue(document.spans.any { it.kind == "link" })
                }
                document.text.length + document.spans.size + document.blocks.size
            }
            val line = "MARKIT_PERF phase=render_decode fixture=$name bytes=${source.toByteArray().size} " +
                "iterations=$iterations median_ms=%.3f p95_ms=%.3f max_ms=%.3f sink=$sink".format(stats.medianMs, stats.p95Ms, stats.maxMs)
            Log.i(TAG, line)
            println(line)
        }
    }

    @Test fun renderDecodeSmall() = checkRenderDecode("small", 4_096, 5, 25)
    @Test fun renderDecodeMedium() = checkRenderDecode("medium", 32_768, 3, 9)
    @Test fun renderDecodeLarge() = checkRenderDecode("large", 131_072, 2, 5)
}
