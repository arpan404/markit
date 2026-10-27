package dev.markit.demo

import android.app.Activity
import android.os.Bundle
import dev.markit.MarkitView

class MainActivity : Activity() {
    private lateinit var markdown: MarkitView
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        markdown = MarkitView(this); setContentView(markdown)
        markdown.render("""
            # Native Markdown

            **Rust** parses this document. Android draws *native text*.

            - [x] Code and math
            - [x] Virtualized rows

            ```kotlin title="Hello.kt"
            // Shared Rust highlighting.
            val message = "Hello, world"
            println(message)
            ```

            | Target | Engine |
            | :--- | ---: |
            | Android | Native views |
            | iOS | UIKit |

            A fraction \(\frac{x^2}{2}\).

            ```diff
            -old value
            +new value
            ```
        """.trimIndent())
    }
    override fun onDestroy() { markdown.close(); super.onDestroy() }
}
