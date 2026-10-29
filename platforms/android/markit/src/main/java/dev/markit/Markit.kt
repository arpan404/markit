package dev.markit

import org.json.JSONObject
import java.io.Closeable

/** Span payload fields use the JSON export's names (`url`, `level`, `png`, ...). */
data class MarkitSpan(val start: Int, val end: Int, val kind: String, val fields: Map<String, Any> = emptyMap()) {
    fun string(key: String): String = fields[key]?.toString() ?: ""
    fun number(key: String): Int = when (val value = fields[key]) { is Number -> value.toInt(); is String -> value.toIntOrNull() ?: 0; else -> 0 }
    fun bool(key: String): Boolean = fields[key] == true
}
data class MarkitBlock(val start: Int, val end: Int, val kind: String)
data class MarkitDocument(val version: Int, val text: String, val spans: List<MarkitSpan>, val blocks: List<MarkitBlock>) {
    fun source(start: Int = 0, end: Int = text.length): String {
        require(start >= 0 && end >= start && end <= text.length)
        val output = StringBuilder(); var at = start
        for (span in spans.filter { it.kind == "math" && it.start >= start && it.end <= end }) {
            output.append(text,at,span.start); output.append(span.string("raw")); at = span.end
        }
        output.append(text,at,end); return output.toString()
    }
    companion object {
        internal fun decode(json: JSONObject): MarkitDocument {
            require(json.getInt("version") == 2) { "Unsupported document schema" }
            val text = json.getString("text")
            val spans = json.getJSONArray("spans")
            val blocks = json.getJSONArray("blocks")
            return MarkitDocument(2, text,
                List(spans.length()) { i -> val span = spans.getJSONObject(i)
                    val start = span.getInt("start"); val end = span.getInt("end")
                    require(start >= 0 && end >= start && end <= text.length)
                    val fields = HashMap<String, Any>()
                    for (key in span.keys()) if (key != "start" && key != "end" && key != "kind" && !span.isNull(key)) fields[key] = span.get(key)
                    MarkitSpan(start, end, span.getString("kind"), fields)
                }, List(blocks.length()) { i -> val block = blocks.getJSONObject(i)
                    val start = block.getInt("start"); val end = block.getInt("end")
                    require(start >= 0 && end >= start && end <= text.length)
                    MarkitBlock(start, end, block.getString("kind"))
                })
        }
    }
}
data class MarkitReference(val kind: String, val target: String)
/**
 * Which dialect a render uses. [DEFAULT] is agent prose (raw HTML shows as text);
 * [GITHUB] is for PR, issue and comment bodies: footnotes, alerts, an allowlisted
 * HTML subset, task spans with source offsets, forge references, emoji and diagrams.
 */
enum class MarkitFlavor(val code: Int) { DEFAULT(0), GITHUB(1) }
data class MarkitPatch(val replaceFrom: Int, val documents: List<MarkitDocument>, val finalized: Boolean)

object Markit {
    init { System.loadLibrary("markit_ffi") }
    @JvmStatic private external fun nativeRender(source: String, format: Int): String
    @JvmStatic private external fun nativeRenderFlavor(source: String, format: Int, flavor: Int): String
    @JvmStatic external fun nativeCreateStream(): Long
    @JvmStatic external fun nativeCreateStreamFlavor(flavor: Int): Long
    @JvmStatic external fun nativeAppend(handle: Long, delta: String, finalized: Boolean): String
    /** MKNP v2 patch bytes. See docs/binary-transport.md. */
    @JvmStatic external fun nativeAppendPatch(handle: Long, delta: String, finalized: Boolean): ByteArray
    @JvmStatic external fun nativeFreeStream(handle: Long)
    fun parse(source: String, flavor: MarkitFlavor = MarkitFlavor.DEFAULT): MarkitDocument =
        MarkitDocument.decode(JSONObject(if (flavor == MarkitFlavor.DEFAULT) nativeRender(source, 0) else nativeRenderFlavor(source, 0, flavor.code)))
    fun highlight(source: String, language: String, fullGrammar: Boolean = false): MarkitDocument = MarkitDocument.decode(JSONObject(nativeRender(JSONObject().put("source", source).put("language", language).toString(), if (fullGrammar) 3 else 2)))
}
/**
 * Patches cross JNI as MKNP v2 bytes instead of JSON. An open code fence sends
 * only its appended text and changed spans, applied to the block this stream
 * already holds, so a long live fence stays proportional to each append.
 */
class MarkitStream(val flavor: MarkitFlavor = MarkitFlavor.DEFAULT) : Closeable {
    private var handle = if (flavor == MarkitFlavor.DEFAULT) Markit.nativeCreateStream() else Markit.nativeCreateStreamFlavor(flavor.code)
    private val documents = ArrayList<MarkitDocument>()
    private var failed = false
    @Synchronized fun append(delta: String, finalized: Boolean = false): MarkitPatch {
        check(handle != 0L) { "Stream closed" }
        check(!failed) { "Stream failed; create a new stream" }
        val bytes = Markit.nativeAppendPatch(handle, delta, finalized)
        try {
            return NativePatchReader.apply(bytes, documents)
        } catch (error: RuntimeException) {
            failed = true
            throw error
        }
    }
    @Synchronized override fun close() { if (handle != 0L) { Markit.nativeFreeStream(handle); handle = 0 } }
}
