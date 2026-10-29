package dev.markit

import android.util.Base64
import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Reader for MKNP v2 stream patches and their MKBP v1 documents (docs/binary-transport.md). */
internal object NativePatchReader {
    private val palette = arrayOf(
        "#24292f" to "#c9d1d9", "#6e7781" to "#8b949e", "#0a3069" to "#a5d6ff", "#0550ae" to "#79c0ff",
        "#cf222e" to "#ff7b72", "#8250df" to "#d2a8ff", "#953800" to "#ffa657", "#116329" to "#7ee787",
    )

    private fun word(buffer: ByteBuffer, at: Int): Int {
        val value = buffer.getInt(at).toLong() and 0xffffffffL
        require(value <= Int.MAX_VALUE) { "Malformed Markit patch" }
        return value.toInt()
    }

    /** Applies one patch to [documents], the stream's current blocks. */
    fun apply(bytes: ByteArray, documents: MutableList<MarkitDocument>): MarkitPatch {
        val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        require(bytes.size >= 28 && bytes[0] == 'M'.code.toByte() && bytes[1] == 'K'.code.toByte() &&
            bytes[2] == 'N'.code.toByte() && bytes[3] == 'P'.code.toByte()) { "Malformed Markit patch" }
        require(word(buffer, 4) == 2) { "Unsupported Markit patch version" }
        val replaceFrom = word(buffer, 8)
        val finalized = word(buffer, 12) != 0
        val count = word(buffer, 24)
        require(replaceFrom <= documents.size) { "Malformed Markit patch" }
        var at = 28
        val blocks = ArrayList<MarkitDocument>(count)
        for (index in 0 until count) {
            val kind = word(buffer, at); at += 4
            require(kind == 0 || kind == 1) { "Malformed Markit patch" }
            val header = if (kind == 1) IntArray(4) { word(buffer, at + it * 4) }.also { at += 16 } else null
            val length = word(buffer, at); at += 4
            require(length <= bytes.size - at) { "Malformed Markit patch" }
            val fresh = document(bytes, at, length)
            at += length
            if (header == null) { blocks.add(fresh); continue }
            val old = documents.getOrNull(replaceFrom + index) ?: throw IllegalArgumentException("Malformed Markit patch")
            val (keepText, head, keepFrom, keepTo) = header.toList()
            require(keepText <= old.text.length && head <= fresh.spans.size && keepFrom <= keepTo && keepTo <= old.spans.size) { "Malformed Markit patch" }
            val text = StringBuilder(keepText + fresh.text.length).append(old.text, 0, keepText).append(fresh.text).toString()
            val spans = ArrayList<MarkitSpan>(fresh.spans.size + keepTo - keepFrom)
            spans.addAll(fresh.spans.subList(0, head))
            spans.addAll(old.spans.subList(keepFrom, keepTo))
            spans.addAll(fresh.spans.subList(head, fresh.spans.size))
            blocks.add(MarkitDocument(fresh.version, text, spans, fresh.blocks))
        }
        require(at == bytes.size) { "Malformed Markit patch" }
        while (documents.size > replaceFrom) documents.removeAt(documents.size - 1)
        documents.addAll(blocks)
        return MarkitPatch(replaceFrom, blocks, finalized)
    }

    /** One MKBP v1 document with the JSON export's field values; math PNG bytes are re-encoded as base64. */
    fun document(bytes: ByteArray, offset: Int, length: Int): MarkitDocument {
        val buffer = ByteBuffer.wrap(bytes, offset, length).slice().order(ByteOrder.LITTLE_ENDIAN)
        require(length >= 32 && bytes[offset] == 'M'.code.toByte() && bytes[offset + 1] == 'K'.code.toByte() &&
            bytes[offset + 2] == 'B'.code.toByte() && bytes[offset + 3] == 'P'.code.toByte()) { "Malformed Markit document" }
        require(word(buffer, 4) == 1) { "Unsupported Markit document version" }
        val version = word(buffer, 8); val textLength = word(buffer, 12); val spanCount = word(buffer, 16)
        val blockCount = word(buffer, 20); val entries = word(buffer, 24); val blobLength = word(buffer, 28)
        require(textLength <= length && spanCount <= length / 40 && blockCount <= length / 12 && entries <= length / 8) { "Malformed Markit document" }
        val spansAt = 32 + textLength; val blocksAt = spansAt + spanCount * 40
        val tableAt = blocksAt + blockCount * 12; val blobsAt = tableAt + entries * 8
        require(blobsAt <= length && blobLength == length - blobsAt) { "Malformed Markit document" }
        fun blob(id: Int): Pair<Int, Int> {
            require(id in 0 until entries) { "Malformed Markit document" }
            val start = word(buffer, tableAt + id * 8); val size = word(buffer, tableAt + id * 8 + 4)
            require(start <= blobLength && size <= blobLength - start) { "Malformed Markit document" }
            return offset + blobsAt + start to size
        }
        fun string(id: Int): String = blob(id).let { (start, size) -> String(bytes, start, size, Charsets.UTF_8) }
        val text = String(bytes, offset + 32, textLength, Charsets.UTF_8)
        val spans = ArrayList<MarkitSpan>(spanCount)
        for (i in 0 until spanCount) {
            val at = spansAt + i * 40
            val start = word(buffer, at); val end = word(buffer, at + 4); val tag = word(buffer, at + 8)
            val a = IntArray(7) { buffer.getInt(at + 12 + it * 4) }
            // Extension documents index the full extended text, not just `text`.
            require(start in 0..end) { "Malformed Markit document" }
            var kind: String
            val fields = HashMap<String, Any>(4)
            when (tag) {
                1 -> { kind = "heading"; fields["level"] = a[0] }
                2 -> kind = "paragraph"
                3 -> kind = "strong"
                4 -> kind = "emphasis"
                5 -> kind = "strikethrough"
                6 -> kind = "code"
                7 -> { kind = "code_block"; fields["language"] = string(a[0]) }
                8 -> { kind = "link"; fields["url"] = string(a[0]) }
                9 -> { kind = "image"; fields["url"] = string(a[0]) }
                10 -> { kind = "quote"; fields["depth"] = a[0] }
                11 -> { kind = "list_item"; fields["depth"] = a[0] }
                12 -> kind = "table"
                13 -> kind = "table_header"
                14 -> kind = "rule"
                15 -> { kind = "reference"; fields["reference_kind"] = string(a[0]); fields["target"] = string(a[1]) }
                16 -> { kind = "code_metadata"; if (a[0] != -1) fields["file"] = string(a[0]); fields["diff"] = a[1] != 0 }
                17 -> { kind = "syntax"; val colors = palette.getOrNull(a[0]) ?: throw IllegalArgumentException("Malformed Markit document"); fields["light"] = colors.first; fields["dark"] = colors.second }
                18 -> { kind = "diff"; fields["change"] = string(a[0]) }
                19 -> {
                    kind = "math"; fields["tex"] = string(a[0]); fields["raw"] = string(a[1]); fields["display"] = a[2] != 0
                    val (start, size) = blob(a[3])
                    fields["png"] = Base64.encodeToString(bytes, start, size, Base64.NO_WRAP); fields["width"] = a[4]; fields["height"] = a[5]
                }
                20 -> { kind = "table_cell"; fields["row"] = a[0]; fields["column"] = a[1]; fields["header"] = a[2] != 0; fields["alignment"] = string(a[3]) }
                21 -> { kind = "syntax"; fields["light"] = "#%06x".format(a[0]); fields["dark"] = "#%06x".format(a[1]) }
                22 -> { kind = "syntax"; fields["light"] = string(a[0]); fields["dark"] = string(a[1]) }
                23 -> { kind = "alert"; fields["alert"] = string(a[0]) }
                24 -> { kind = "footnote_reference"; fields["label"] = string(a[0]); fields["number"] = a[1] }
                25 -> { kind = "footnote_definition"; fields["label"] = string(a[0]); fields["number"] = a[1] }
                26 -> { kind = "task"; fields["index"] = a[0]; fields["offset"] = a[1]; fields["checked"] = a[2] != 0 }
                27 -> { kind = "diagram"; fields["language"] = string(a[0]); fields["source"] = string(a[1]) }
                28 -> { kind = "video"; fields["url"] = string(a[0]) }
                29 -> { kind = "html"; fields["tag"] = string(a[0]) }
                30 -> { kind = "dark_image"; fields["url"] = string(a[0]) }
                else -> throw IllegalArgumentException("Malformed Markit document")
            }
            spans.add(MarkitSpan(start, end, kind, fields))
        }
        val blocks = List(blockCount) { i ->
            val at = blocksAt + i * 12
            MarkitBlock(word(buffer, at), word(buffer, at + 4), string(word(buffer, at + 8)))
        }
        return MarkitDocument(version, text, spans, blocks)
    }
}
