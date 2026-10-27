package dev.markit

import org.json.JSONObject
import java.io.Closeable

data class MarkitSpan(val start: Int, val end: Int, val kind: String, val data: JSONObject) {
    fun string(key: String): String = if (data.isNull(key)) "" else data.optString(key, "")
    fun number(key: String): Int = data.optInt(key, 0)
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
                    MarkitSpan(start, end, span.getString("kind"), span)
                }, List(blocks.length()) { i -> val block = blocks.getJSONObject(i)
                    val start = block.getInt("start"); val end = block.getInt("end")
                    require(start >= 0 && end >= start && end <= text.length)
                    MarkitBlock(start, end, block.getString("kind"))
                })
        }
    }
}
data class MarkitReference(val kind: String, val target: String)
data class MarkitPatch(val replaceFrom: Int, val documents: List<MarkitDocument>, val finalized: Boolean)

object Markit {
    init { System.loadLibrary("markit_ffi") }
    @JvmStatic private external fun nativeRender(source: String, format: Int): String
    @JvmStatic external fun nativeCreateStream(): Long
    @JvmStatic external fun nativeAppend(handle: Long, delta: String, finalized: Boolean): String
    @JvmStatic external fun nativeFreeStream(handle: Long)
    fun parse(source: String): MarkitDocument = MarkitDocument.decode(JSONObject(nativeRender(source, 0)))
    fun highlight(source: String, language: String, fullGrammar: Boolean = false): MarkitDocument = MarkitDocument.decode(JSONObject(nativeRender(JSONObject().put("source", source).put("language", language).toString(), if (fullGrammar) 3 else 2)))
}
class MarkitStream : Closeable {
    private var handle = Markit.nativeCreateStream()
    @Synchronized fun append(delta: String, finalized: Boolean = false): MarkitPatch {
        check(handle != 0L) { "Stream closed" }
        val patch = JSONObject(Markit.nativeAppend(handle, delta, finalized))
        val blocks = patch.getJSONArray("blocks")
        return MarkitPatch(patch.getInt("replace_from"), List(blocks.length()) { MarkitDocument.decode(blocks.getJSONObject(it).getJSONObject("document")) }, patch.getBoolean("finalized"))
    }
    @Synchronized override fun close() { if (handle != 0L) { Markit.nativeFreeStream(handle); handle = 0 } }
}
