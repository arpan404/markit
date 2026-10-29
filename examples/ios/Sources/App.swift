import SwiftUI
import Markit

@main struct MarkitDemoApp: App {
    var body: some Scene {
        WindowGroup {
            NavigationView {
                TranscriptDemo()
                    .navigationTitle("Markit")
                    .toolbar {
                        NavigationLink(destination: MarkitText(DemoText.document).navigationTitle("Document")) { Text("Document") }
                    }
            }
            .navigationViewStyle(.stack)
        }
    }
}

/// A chat-style transcript: each answer is a self-sizing `MarkitInline` row in
/// a LazyVStack, and "Stream" appends an answer in small deltas.
struct TranscriptDemo: View {
    struct Message: Identifiable {
        let id: Int
        let fromUser: Bool
        var text: String
        var done: Bool
    }

    @State private var messages: [Message] = [
        Message(id: 0, fromUser: true, text: "How do I sum the first n integers?", done: true),
        Message(id: 1, fromUser: false, text: DemoText.document, done: true),
    ]
    @State private var streaming = false
    @State private var lastReference = ""

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 20) {
                    ForEach(messages) { message in
                        row(message).id(message.id)
                    }
                    if !lastReference.isEmpty {
                        Text("Opened \(lastReference)").font(.footnote).foregroundColor(.secondary)
                    }
                }
                .padding(16)
            }
            .onChange(of: messages.last?.text.count ?? 0) { _ in
                if let last = messages.last { proxy.scrollTo(last.id, anchor: .bottom) }
            }
        }
        .safeAreaInset(edge: .bottom) {
            Button(streaming ? "Streaming…" : "Stream an answer") { stream() }
                .buttonStyle(.borderedProminent)
                .disabled(streaming)
                .padding(8)
        }
    }

    @ViewBuilder private func row(_ message: Message) -> some View {
        if message.fromUser {
            Text(message.text)
                .padding(12)
                .background(Color.accentColor.opacity(0.15), in: RoundedRectangle(cornerRadius: 16))
                .frame(maxWidth: .infinity, alignment: .trailing)
        } else {
            MarkitInline(markdown: message.text, finalized: message.done, theme: DemoTheme.answer,
                         codeCollapse: MarkitCodeCollapse(),
                         onReference: { lastReference = $0.target })
        }
    }

    private func stream() {
        streaming = true
        let id = messages.count
        messages.append(Message(id: id, fromUser: true, text: "And in Swift?", done: true))
        messages.append(Message(id: id + 1, fromUser: false, text: "", done: false))
        let chunks = DemoText.streamed.chunked(size: 12)
        Task { @MainActor in
            for chunk in chunks {
                try? await Task.sleep(nanoseconds: 30_000_000)
                messages[messages.count - 1].text += chunk
            }
            messages[messages.count - 1].done = true
            streaming = false
        }
    }
}

enum DemoTheme {
    static let answer: MarkitTheme = {
        var theme = MarkitTheme()
        theme.bodyFont = .system(size: 16)
        theme.codeHeaderBackground = .clear
        theme.codeBlockBackground = .secondarySystemBackground
        theme.linkColor = .systemBlue
        theme.blockSpacing = 12
        return theme
    }()
}

extension String {
    func chunked(size: Int) -> [String] {
        var result: [String] = []
        var current = ""
        for character in self {
            current.append(character)
            if current.count == size { result.append(current); current = "" }
        }
        if !current.isEmpty { result.append(current) }
        return result
    }
}

enum DemoText {
    static let document = #"""
    # Native Markdown

    **Rust** parses this text. UIKit displays it with *native text layout*.

    - [x] Code and math
    - [x] Rows that size themselves

    ```swift title="Hello.swift"
    // Shared Rust syntax highlighting.
    let message = "Hello, world"
    print(message)
    ```

    | Target | Engine |
    | :--- | ---: |
    | iOS | UIKit |
    | Android | Native views |

    > Quotes get a bar, and links like [the docs](https://example.com) go to `onLink`.

    A fraction \(\frac{x^2}{2}\) and display math:

    $$\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$$

    ```diff
    -old value
    +new value
    ```
    """#

    static let streamed = #"""
    Use the closed form, which avoids a loop:

    ```swift
    func sum(upTo n: Int) -> Int {
        n * (n + 1) / 2
    }
    ```

    It works because pairing the first and last terms gives \(n + 1\) each time, and there are \(n/2\) pairs.

    | n | sum |
    | ---: | ---: |
    | 10 | 55 |
    | 100 | 5050 |
    """#
}
