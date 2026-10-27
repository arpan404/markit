import SwiftUI
import Markit

@main struct MarkitDemoApp: App {
    var body: some Scene {
        WindowGroup {
            NavigationView {
                MarkitText(sample).navigationTitle("Markit")
            }
        }
    }
}
private let sample = #"""
# Native Markdown

**Rust** parses this text. UIKit displays it with *native text layout*.

- [x] Code and math
- [x] Virtualized rows

```swift title="Hello.swift"
// Shared Rust syntax highlighting.
let message = "Hello, world"
print(message)
```

| Target | Engine |
| :--- | ---: |
| iOS | UIKit |
| Android | Native views |

A fraction \(\frac{x^2}{2}\) and display math:

$$\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$$

```diff
-old value
+new value
```
"""#
