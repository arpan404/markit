# Binary render plan version 1

The additive Rust `render_document_binary(source)` and
`encode_document_binary(&Document)` APIs produce the same document as JSON.
Existing JSON, HTML, stream, Wasm and JNI APIs are unchanged. The C exports
`markit_render_binary(source, len)` and `markit_code_binary(&request)` return
one owned `MarkitBuffer`. Check status before reading; free exactly once with
`markit_buffer_free`, including error results. No input pointer is retained.
Status values and pointer validity requirements match `markit_render`.
Overflow beyond u32 transport limits or invalid math base64 returns status 2.

`MarkitCodeRequest` is a C-layout struct in this order:
`const uint8_t *source; size_t source_len; const uint8_t *language;
size_t language_len; uint32_t mode;`. Input strings are UTF-8. Mode 0 is lexical,
mode 1 full grammar, and other modes return status 2. Missing grammar support
also returns status 2. This struct uses the platform C ABI and is not serialized
into the output. No request JSON is constructed.

## Byte layout

All wire integers are unsigned little-endian u32, including counts, offsets and
UTF-16 range endpoints. Sections have no padding or alignment requirements.
Readers must use byte loads or unaligned loads. Ranges remain half-open UTF-16
positions into rendered text, including surrogate pairs and object replacements.

| Byte offset | Header field |
| --- | --- |
| 0 | Four literal bytes `MKBP` |
| 4 | Transport version, 1 |
| 8 | Document schema version, currently 2 |
| 12 | Text length in UTF-8 bytes |
| 16 | Span count |
| 20 | Block count |
| 24 | String/blob table entry count |
| 28 | Blob area length in bytes |

The 32-byte header is followed by UTF-8 rendered text, then spans, blocks,
table entries, and the blob area. No terminators appear. Reject unsupported
transport versions. Readers should validate section arithmetic and all table
references before use. The total packet length must equal the sum of its sections.
Unknown tags can be skipped because span records have a fixed width.

A span is 40 bytes: start, end, tag, then seven payload words `a` through `g`.
Unused payload words are zero. A block is 12 bytes: start, end, kind string ID.
A table entry is 8 bytes: offset into the blob area, byte length. IDs are zero
based and shared by strings and raw byte blobs. Repeated strings and repeated
math PNGs share entries. An empty string is a valid zero-length entry. All
string references point to UTF-8 bytes. PNG references point to raw PNG bytes,
with no base64 or data URL prefix. Only the absent optional metadata file uses
the sentinel `0xffffffff`; the empty file string differs from absence.

| Tag | JSON kind | Payload |
| --- | --- | --- |
| 1 | heading | a: level |
| 2 | paragraph | none |
| 3 | strong | none |
| 4 | emphasis | none |
| 5 | strikethrough | none |
| 6 | code | none |
| 7 | code_block | a: language string ID |
| 8 | link | a: URL string ID |
| 9 | image | a: URL string ID |
| 10 | quote | a: depth |
| 11 | list_item | a: depth |
| 12 | table | none |
| 13 | table_header | none |
| 14 | rule | none |
| 15 | reference | a: reference_kind string ID, b: target string ID |
| 16 | code_metadata | a: optional file string ID, b: diff boolean |
| 17 | syntax | a: lexical palette ID |
| 18 | diff | a: change string ID |
| 19 | math | a: tex ID, b: raw ID, c: display boolean, d: PNG blob ID, e: width, f: height |
| 20 | table_cell | a: row, b: column, c: header boolean, d: alignment string ID |
| 21 | syntax | a: light RGB, b: dark RGB, each 0x00RRGGBB |
| 22 | syntax | a: light string ID, b: dark string ID |

Booleans are 0 or 1. Tag 21 is used for non-palette canonical lowercase RGB
colors, including grammar colors. Tag 22 preserves unusual public Rust Document
color strings exactly, including uppercase hex. Tag 17 is used whenever both
colors exactly match a palette pair, including grammar results that happen to
match that pair. Wire version 1 fixes the following palette order. The encoder
and lexical highlighter share the palette in Rust.

| ID | Light | Dark |
| --- | --- | --- |
| 0 | #24292f | #c9d1d9 |
| 1 | #6e7781 | #8b949e |
| 2 | #0a3069 | #a5d6ff |
| 3 | #0550ae | #79c0ff |
| 4 | #cf222e | #ff7b72 |
| 5 | #8250df | #d2a8ff |
| 6 | #953800 | #ffa657 |
| 7 | #116329 | #7ee787 |

## Swift reader

`MarkitBinaryPlan.parse` and `.highlight` are opt-in and retain the Rust buffer
until deinit. They read records and copy requested string/blob entries directly.
`text` is decoded once. Math bytes are available through `blob(record.arguments[3])`.
No JSONDecoder, request JSON or base64 decode is needed. The existing
MarkitDocument and native UI continue using JSON. Integrating the direct records
into UIKit is a separate change; no UI performance improvement is claimed here.
The reader validates packet shape and blob bounds, and exposes tags and payloads
for the host to interpret with this spec. It assumes packets originate from the
Rust encoder; it is not an untrusted-network semantic validator.

The existing Rust Document still stores math PNGs as base64. Encoding converts
those bytes back to raw PNG, and deduplicates the output. Avoiding the internal
base64 allocation would require changing the math/document representation.

A standalone macOS bridge check avoids UIKit and simulator requirements:

```sh
cargo build -p markit-ffi
mkdir -p target/binary-swift/CMarkit
cat > target/binary-swift/CMarkit/module.modulemap <<'MAP'
module CMarkit {
  header "../../../platforms/ios/Sources/CMarkit/include/markit.h"
  export *
}
MAP
swiftc -I target/binary-swift/CMarkit -L target/debug -lmarkit_ffi \
  -Xlinker -rpath -Xlinker "$PWD/target/debug" \
  platforms/ios/Sources/Markit/Markit.swift \
  platforms/ios/Sources/Markit/BinaryPlan.swift benchmarks/binary-reader.swift \
  -o target/binary-swift/check
target/binary-swift/check
```

Rust parity checks compare an independent binary decoder's output against
`render_document_json` field for field for six fence languages, math, a table,
and empty input. Separate tests cover direct code, non-palette RGB, unusual
color strings, optional full grammars, and FFI ownership/error handling.

## Benchmark

Run `cargo run --release -p markit --example binary_bench`. The harness uses
4 KiB, 64 KiB and 512 KiB composed documents containing prose, TS, a table and
repeated math. It reports encode-only medians for the same pre-parsed Document
and end-to-end render medians. Each round interleaves and rotates four paths,
discards four warmup passes and takes 31 measured samples, with 30/4/1 calls
per sample by size. It runs three rounds. Repeat the process for load stability.
These are warmed caches, not cold font initialization or first-use grammar costs.
