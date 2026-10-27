package dev.markit

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class NativeBridgeTest {
    @Test fun unicodeHighlightAndMathCrossTheRealJniBridge() {
        val document = Markit.parse("# Hello 🦀\n\n**bold 😀** and \\(x^2\\)")
        val bold = document.spans.first { it.kind == "strong" }
        assertEquals("bold 😀",document.text.substring(bold.start,bold.end))
        assertTrue(document.spans.any { it.kind == "math" && it.string("png").startsWith("iVBOR") })
        val code = Markit.highlight("val greeting = \"😀\";\n","kotlin")
        assertEquals("val greeting = \"😀\";\n",code.text)
        assertTrue(code.spans.any { it.kind == "syntax" })
    }
    @Test fun streamsResolveLateReferencesAndCloseIdempotently() {
        val stream = MarkitStream()
        stream.append("[docs][ref]\n\nOpen tail")
        val patch = stream.append(" continues\n\n[ref]: https://example.com\n",true)
        assertEquals(0,patch.replaceFrom)
        assertTrue(patch.documents[0].spans.any { it.string("url") == "https://example.com" })
        stream.close(); stream.close()
        try { stream.append("x"); fail("closed stream accepted input") } catch (_: IllegalStateException) {}
    }
    @Test fun invalidNativeHandlesThrowRatherThanDereferenceForgedPointers() {
        try { Markit.nativeAppend(Long.MAX_VALUE,"x",false); fail("invalid handle accepted") } catch (_: IllegalStateException) {}
    }
}
