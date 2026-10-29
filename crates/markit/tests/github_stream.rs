//! A GitHub flavour stream shows, after every append, exactly what a
//! one-shot render of the same text shows: HTML, native text and spans.
use markit::{parse_with, render_html_with, Document, Flavor, RenderedBlock, Span, StreamRenderer};
use std::sync::Arc;

const SOURCES: &[&str] = &[
    "> [!WARNING]\n> Careful @octocat\n\nShipped :tada: thanks[^1] #12\n\n[^1]: Since **2.0**.\n\nAfter a1b2c3d.\n",
    "- [ ] one\n- [x] two\n\n```\n- [ ] not a task\n```\n\n1. [ ] three\n   - [ ] four\n",
    "<details>\n<summary>Logs</summary>\n\n```rust\nfn main() {}\n```\n\nmore **text**\n\n</details>\n\nafter\n",
    "Intro\n\n<p align=\"center\">\n<img src=\"https://x.test/a.png\" width=\"20\">\n</p>\n\n<b>open bold\n\nnext\n",
    "```mermaid\ngraph TD; A-->B\n```\n\ntext\n\n```mermaid\nstill open",
    "Ref[^a] and[^b].\n\n- [ ] task[^a]\n\n[^b]: Bee with `code`.\n[^a]: Ay\n    continued.\n\n```\n[^z]: not a definition\n```\n",
    "Press <kbd>Ctrl</kbd> and see owner/repo#5 or @org/team.\n\n| a | b |\n| - | - |\n| <sub>x</sub> | :+1: |\n\n<video src=\"https://x.test/v.mp4\"></video>\n",
    "Late[^n] reference.\n\nMiddle\n\n[^n]: Defined last.\n",
    "😀 **é** <ins>new</ins>\r\n\r\n- [ ] 🦀 one\r\n- [ ] two\r\n",
];

fn apply(blocks: &mut Vec<Arc<RenderedBlock>>, patch: markit::RenderPatch) {
    blocks.truncate(patch.replace_from);
    blocks.extend(patch.blocks);
}

/// Concatenate native block documents into one, shifting each block's ranges.
fn joined(blocks: &[Arc<RenderedBlock>]) -> (String, Vec<Span>) {
    let mut text = String::new();
    let mut spans = Vec::new();
    for block in blocks {
        let document = block.document.as_ref().expect("native document");
        let base = text.encode_utf16().count();
        text.push_str(&document.text);
        spans.extend(document.spans.iter().map(|span| Span {
            start: span.start + base,
            end: span.end + base,
            style: span.style.clone(),
        }));
    }
    (text, spans)
}

fn sorted(document: &Document) -> Vec<Span> {
    document.spans.clone()
}

fn check_prefixes(source: &str, cuts: impl Iterator<Item = usize>) {
    let mut html_stream = StreamRenderer::new_html().with_flavor(Flavor::Github);
    let mut both_stream = StreamRenderer::new().with_flavor(Flavor::Github);
    let mut html_blocks = Vec::new();
    let mut both_blocks = Vec::new();
    let mut at = 0;
    for cut in cuts.chain(std::iter::once(source.len())) {
        if cut < at || !source.is_char_boundary(cut) {
            continue;
        }
        let delta = &source[at..cut];
        let finalized = cut == source.len();
        apply(&mut html_blocks, html_stream.append(delta, finalized));
        apply(&mut both_blocks, both_stream.append(delta, finalized));
        at = cut;
        let prefix = &source[..cut];
        let expected = render_html_with(prefix, Flavor::Github);
        let html: String = html_blocks
            .iter()
            .map(|block| block.html.as_str())
            .collect();
        assert_eq!(html, expected, "html after {prefix:?}");
        let html: String = both_blocks
            .iter()
            .map(|block| block.html.as_str())
            .collect();
        assert_eq!(html, expected, "html (both) after {prefix:?}");
        let document = parse_with(prefix, Flavor::Github);
        let (text, spans) = joined(&both_blocks);
        assert_eq!(text, document.text, "text after {prefix:?}");
        assert_eq!(spans, sorted(&document), "spans after {prefix:?}");
    }
}

#[test]
fn every_character_boundary_matches_a_one_shot_render() {
    for source in SOURCES {
        check_prefixes(source, 0..source.len());
    }
}

#[test]
fn line_and_word_sized_appends_match_a_one_shot_render() {
    for source in SOURCES {
        let lines = source.match_indices('\n').map(|(at, _)| at + 1);
        check_prefixes(source, lines);
        let words = source.match_indices(' ').map(|(at, _)| at);
        check_prefixes(source, words);
    }
}

#[test]
fn a_long_stream_after_its_footnotes_stays_correct_and_keeps_settled_blocks_without_them() {
    let mut stream = StreamRenderer::new_html().with_flavor(Flavor::Github);
    let history = "Settled paragraph @octocat.\n\n".repeat(50);
    stream.append(&history, false);
    let update = stream.append("Open tail", false);
    assert!(
        update.parsed_bytes < 100,
        "ordinary GitHub prose still settles"
    );
    let mut blocks = Vec::new();
    let mut source = String::new();
    let mut stream = StreamRenderer::new_html().with_flavor(Flavor::Github);
    for part in [
        history.as_str(),
        "Note[^x] here.\n\n",
        "More\n\n",
        "[^x]: The note.\n",
        "\nTail[^x]\n",
    ] {
        source.push_str(part);
        apply(&mut blocks, stream.append(part, false));
        let html: String = blocks.iter().map(|block| block.html.as_str()).collect();
        assert_eq!(html, render_html_with(&source, Flavor::Github), "{part:?}");
    }
}

#[test]
fn a_reset_keeps_the_flavour_and_switching_flavour_starts_over() {
    let mut stream = StreamRenderer::new_html().with_flavor(Flavor::Github);
    stream.append("@octocat", false);
    stream.reset();
    assert_eq!(stream.flavor(), Flavor::Github);
    let patch = stream.append("@octocat\n", true);
    assert!(patch.blocks[0].html.contains("mk-mention"));
    stream.set_flavor(Flavor::Default);
    assert_eq!(stream.source(), "");
    let patch = stream.append("@octocat\n", true);
    assert!(!patch.blocks[0].html.contains("mk-mention"));
}
