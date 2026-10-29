//! MKNP v2 patches applied by an independent host model must reproduce the
//! documents of the JSON stream API after every append.
#[path = "fixtures/mkbp.rs"]
mod mkbp;
#[path = "fixtures/stream_sequences.rs"]
mod sequences;

use serde_json::Value;

fn word(bytes: &[u8], at: &mut usize) -> usize {
    let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap()) as usize;
    *at += 4;
    value
}

/// Applies one patch the way the Swift and Kotlin readers do. Returns the
/// number of extension records.
fn apply(host: &mut Vec<Value>, bytes: &[u8]) -> usize {
    assert_eq!(&bytes[..4], b"MKNP");
    let mut at = 4;
    assert_eq!(word(bytes, &mut at), 2);
    let replace_from = word(bytes, &mut at);
    let _finalized = word(bytes, &mut at);
    let _source = word(bytes, &mut at);
    let _parsed = word(bytes, &mut at);
    let count = word(bytes, &mut at);
    let mut previous = host.split_off(replace_from.min(host.len()));
    let mut extensions = 0;
    for i in 0..count {
        let kind = word(bytes, &mut at);
        let header: Vec<usize> = if kind == 1 {
            (0..4).map(|_| word(bytes, &mut at)).collect()
        } else {
            assert_eq!(kind, 0);
            Vec::new()
        };
        let len = word(bytes, &mut at);
        let mut document = mkbp::decode(&bytes[at..at + len]);
        at += len;
        if kind == 1 {
            extensions += 1;
            let old = previous
                .get_mut(i)
                .expect("extension of a block the host holds");
            let (keep_text, head, keep_from, keep_to) =
                (header[0], header[1], header[2], header[3]);
            let old_text: Vec<u16> = old["text"].as_str().unwrap().encode_utf16().collect();
            let mut text = String::from_utf16(&old_text[..keep_text]).unwrap();
            text.push_str(document["text"].as_str().unwrap());
            let fresh = document["spans"].as_array().unwrap().clone();
            let old_spans = old["spans"].as_array().unwrap();
            let mut spans = fresh[..head].to_vec();
            spans.extend_from_slice(&old_spans[keep_from..keep_to]);
            spans.extend_from_slice(&fresh[head..]);
            document["text"] = Value::String(text);
            document["spans"] = Value::Array(spans);
        }
        host.push(document);
    }
    assert_eq!(at, bytes.len());
    drop(previous.drain(..));
    extensions
}

fn check(parts: &[String], label: &str) -> usize {
    check_flavor(parts, label, markit::Flavor::Default)
}

fn check_flavor(parts: &[String], label: &str, flavor: markit::Flavor) -> usize {
    let mut lazy = markit::StreamRenderer::new_native().with_flavor(flavor);
    let mut eager = markit::StreamRenderer::new_native().with_flavor(flavor);
    let mut host = Vec::new();
    let mut expected: Vec<Value> = Vec::new();
    let mut extensions = 0;
    for (i, part) in parts.iter().enumerate() {
        let finalized = i + 1 == parts.len();
        let bytes = lazy.append_native_patch(part, finalized).unwrap();
        extensions += apply(&mut host, &bytes);
        let patch = eager.append(part, finalized);
        expected.truncate(patch.replace_from);
        for block in &patch.blocks {
            expected.push(serde_json::to_value(block.document.as_ref().unwrap()).unwrap());
        }
        assert_eq!(host, expected, "{label}: after part {i} {part:?}");
        // The old v1 encoder on the eager patch still decodes to the same documents.
        markit::encode_native_patch(&patch).unwrap();
    }
    extensions
}

fn lines(source: &str) -> Vec<String> {
    source.split_inclusive('\n').map(String::from).collect()
}

fn chunks(source: &str, size: usize) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    chars.chunks(size).map(|c| c.iter().collect()).collect()
}

#[test]
fn v2_patches_rebuild_json_documents_for_stream_fixtures() {
    for (i, parts) in sequences::sequences().into_iter().enumerate() {
        check(&parts, &format!("sequence {i}"));
    }
}

#[test]
fn github_flavour_patches_rebuild_json_documents_with_their_new_spans() {
    let source = concat!(
        "> [!TIP]\n> Use @octocat's fix a1b2c3d :tada:\n\n",
        "- [ ] one[^1]\n- [x] two\n\n",
        "<details>\n<summary>Logs</summary>\n\n```rust\nfn main() {}\n```\n\n</details>\n\n",
        "```mermaid\ngraph TD; A-->B\n```\n\n",
        "```rust\nlet open = 1;\nlet more = 2;\nlet again = 3;\nlet last = 4;\n",
    );
    let with_note = format!("{source}```\n\n[^1]: The note.\n");
    for text in [source, with_note.as_str()] {
        check_flavor(&lines(text), "github by line", markit::Flavor::Github);
        check_flavor(
            &chunks(text, 3),
            "github by 3 chars",
            markit::Flavor::Github,
        );
    }
    // An open fence outside footnotes still extends instead of resending.
    assert!(check_flavor(&lines(source), "github fence", markit::Flavor::Github) > 0);
}

#[test]
fn long_fences_extend_instead_of_resending() {
    let rust: String = (0..600)
        .map(|i| format!("fn item_{i}(v: i64) -> i64 {{ /* {i} */ v.wrapping_add({i}) }}\n"))
        .collect();
    let python: String = (0..120)
        .map(|i| {
            format!("def f{i}(x):\n    \"\"\"doc 🦀 {i}\n    more\"\"\"\n    return x + {i}  # c\n")
        })
        .collect();
    let diff: String = (0..300)
        .map(|i| match i % 4 {
            0 => format!("@@ -{i} +{i} @@\n"),
            1 => format!("-old {i} café\n"),
            2 => format!("+new {i}\n"),
            _ => format!(" ctx {i}\n"),
        })
        .collect();
    let plain: String = (0..200)
        .map(|i| format!("plain text line {i} ✓\n"))
        .collect();
    // More than 8192 lexical runs: the fence falls back to plain mid-stream.
    let dense: String = (0..3000).map(|i| format!("a{i}(1,\"s\");\n")).collect();
    let cases = [
        (
            "rust",
            format!("Intro **text**.\n\n```rust\n{rust}```\n\nAfter.\n"),
        ),
        ("python", format!("```python\n{python}```\n")),
        ("diff", format!("```diff\n{diff}```\n")),
        ("plain", format!("```\n{plain}```\n")),
        ("text", format!("~~~~text title=\"a b.txt\"\n{plain}~~~~\n")),
        ("dense", format!("```js\n{dense}```\n")),
        (
            "block comment",
            "```rust\nfn a() {}\n/* open\n".to_owned()
                + &"still open\n".repeat(50)
                + "*/ fn x() {}\n```\n",
        ),
    ];
    for (name, source) in cases {
        let by_line = check(&lines(&source), &format!("{name} by line"));
        assert!(by_line > 0, "{name}: no extensions");
        check(
            &chunks(&source, 7)
                .into_iter()
                .take(1200)
                .collect::<Vec<_>>(),
            &format!("{name} by 7 chars"),
        );
        check(
            &chunks(&source, 1)
                .into_iter()
                .take(1500)
                .collect::<Vec<_>>(),
            &format!("{name} by char"),
        );
    }
}

#[test]
fn json_and_patch_appends_can_interleave() {
    let source: String = (0..80).map(|i| format!("let x{i} = \"v{i}\";\n")).collect();
    let source = format!("```rust\n{source}```\n\nTail\n");
    let parts = lines(&source);
    let mut mixed = markit::StreamRenderer::new_native();
    let mut eager = markit::StreamRenderer::new_native();
    let mut host = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        let finalized = i + 1 == parts.len();
        let expected_patch = eager.append(part, finalized);
        if i % 3 == 0 {
            let patch = mixed.append(part, finalized);
            assert_eq!(
                serde_json::to_string(&patch).unwrap(),
                serde_json::to_string(&expected_patch).unwrap()
            );
            host.truncate(patch.replace_from);
            for block in &patch.blocks {
                host.push(serde_json::to_value(block.document.as_ref().unwrap()).unwrap());
            }
        } else {
            apply(
                &mut host,
                &mixed.append_native_patch(part, finalized).unwrap(),
            );
        }
    }
    let mut all = markit::StreamRenderer::new_native();
    let patch = all.append(&source, true);
    let expected: Vec<Value> = patch
        .blocks
        .iter()
        .map(|b| serde_json::to_value(b.document.as_ref().unwrap()).unwrap())
        .collect();
    assert_eq!(host, expected);
}

#[test]
fn extension_patches_stay_small() {
    let mut stream = markit::StreamRenderer::new_native();
    stream.append_native_patch("```rust\n", false).unwrap();
    let mut largest = 0;
    // 900 lines stay under the lexer's run limit, so every line extends.
    for i in 0..900 {
        let bytes = stream
            .append_native_patch(&format!("let value_{i} = compute({i}); // note\n"), false)
            .unwrap();
        if i > 10 {
            largest = largest.max(bytes.len());
        }
    }
    assert!(largest < 1024, "extension patch grew to {largest} bytes");
}
