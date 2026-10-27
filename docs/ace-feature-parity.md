# Ace feature coverage

Markit implements the renderer features identified in the local Ace Markdown modules. It is a separate library; no Ace application source or data has changed.

| Feature | Implementation |
| --- | --- |
| CommonMark and inline styles | Shared parser; native UTF-16 spans and escaped HTML |
| Nested lists, tasks, quotes, rules | Native attributed/spanned text and web semantics |
| Tables | Alignment metadata; horizontally scrolling native row grids and web tables |
| Code blocks | Language/file metadata, syntax colors, line numbers, copy controls |
| Whole source files | Highlighted native documents and web token results with exact source preservation |
| Syntax languages | Ace's language families and aliases on the default linear tokenizer; optional fuller Sublime grammars |
| Diff fences | File labels and separate addition, deletion, context, and metadata styling |
| Links and references | CommonMark definitions, late-reference invalidation, bare links, safe destinations |
| File, thread, PR actions | Host callbacks for code file paths and explicit file/thread/PR links |
| Images | Host-resolved bytes; native images and browser Blob URLs without automatic remote fetching |
| Math | Dollar/backslash delimiters, shared Rust layout, web SVG/native bitmap output, TeX copying |
| Streaming | Append API, stable block patches, unfinished fences/math, replacement edits and coalescing |
| Memory and worker limits | Bounded caches, input/token limits, plain-source fallback, stale-result suppression and disposal |
| Long documents | Recycled native blocks, small prose rows, bounded code/table viewports; web content visibility and virtualized large code fences |
| Appearance and access | Light/dark syntax colors, native font scaling, selectable text, keyboard reference activation and labeled copy buttons |

App-specific thread/file lookup, PR cards, attachment permissions, conversation scroll anchoring, search, and surrounding transcript virtualization remain responsibilities of the host application. The hooks are implemented; wiring them into Ace requires an Ace integration change.

Compatibility choices are explicit. Raw HTML displays literally. The default tokenizer supplies lexical highlighting rather than exact Prism token equivalence. Full grammars are opt-in. LaTeX support follows the embedded renderer; unsupported constructs stay source text. No claim of complete Ace application behavior or measured native UI parity is made.
