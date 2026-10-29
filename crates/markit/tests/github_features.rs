//! GitHub flavour Markdown features, through the public API. Cases follow
//! ace's forge prose, source-mark and text-segment tests.
#[path = "fixtures/mkbp.rs"]
mod mkbp;

use markit::{parse_with, render_html, render_html_with, with_task, Document, Flavor, Style};

fn github(source: &str) -> String {
    render_html_with(source, Flavor::Github)
}

fn spans<'a>(document: &'a Document, kind: &str) -> Vec<&'a Style> {
    let json = |style: &Style| serde_json::to_value(style).unwrap()["kind"].clone();
    document
        .spans
        .iter()
        .map(|span| &span.style)
        .filter(|style| json(style) == kind)
        .collect()
}

fn text_of(document: &Document, style: &Style) -> String {
    let span = document
        .spans
        .iter()
        .find(|span| &span.style == style)
        .unwrap();
    let units: Vec<u16> = document.text.encode_utf16().collect();
    String::from_utf16(&units[span.start..span.end]).unwrap()
}

#[test]
fn alerts_draw_a_titled_block_without_their_marker() {
    for (marker, class, title) in [
        ("NOTE", "note", "Note"),
        ("TIP", "tip", "Tip"),
        ("IMPORTANT", "important", "Important"),
        ("WARNING", "warning", "Warning"),
        ("CAUTION", "caution", "Caution"),
    ] {
        let source = format!("> [!{marker}]\n> Careful *now*\n");
        let html = github(&source);
        assert!(
            html.starts_with(&format!(
                "<div class=\"mk-alert mk-alert-{class}\" data-markit-alert=\"{class}\"><p class=\"mk-alert-title\">{title}</p>\n<p>Careful <em>now</em></p>\n</div>"
            )),
            "{html}"
        );
        assert!(!html.contains("[!"), "{html}");
        let document = parse_with(&source, Flavor::Github);
        assert_eq!(document.text, format!("{title}\nCareful now\n"));
        let alert = Style::Alert {
            alert: class.into(),
        };
        assert_eq!(text_of(&document, &alert), document.text);
        assert!(
            spans(&document, "quote").len() == 1,
            "old hosts still see a quote"
        );
    }
    // The default flavour keeps the marker as text in an ordinary quote.
    assert!(render_html("> [!NOTE]\n> x\n").contains("[!NOTE]"));
}

#[test]
fn footnotes_are_numbered_by_first_reference_and_drawn_last_with_back_links() {
    let source = "Shipped[^later] thanks[^first] again[^later]\n\n[^first]: One.\n[^later]: Since **2.0**.\n\n[^unused]: Never drawn.\n\nAfter.\n";
    let html = github(source);
    let numbers: Vec<_> = html
        .match_indices("data-markit-footnote=\"")
        .map(|(at, m)| &html[at + m.len()..at + m.len() + 1])
        .collect();
    assert_eq!(numbers, ["1", "2", "1"]);
    let section = &html[html
        .find("<section class=\"mk-footnotes\"")
        .expect("section")..];
    assert!(html.find("<p>After.</p>").unwrap() < html.find("<section").unwrap());
    assert!(section.contains("<li data-markit-footnote-def=\"1\">\n<p>Since <strong>2.0</strong>. <a class=\"mk-footnote-back\""), "{section}");
    assert!(section.find("Since").unwrap() < section.find("One.").unwrap());
    assert!(!html.contains("Never drawn"));
    assert!(!html.contains("[^"));
    let document = parse_with(source, Flavor::Github);
    assert!(
        document.text.starts_with("Shipped[1] thanks[2] again[1]\n"),
        "{:?}",
        document.text
    );
    assert!(
        document.text.ends_with("1. Since 2.0.\n2. One.\n"),
        "{:?}",
        document.text
    );
    assert_eq!(spans(&document, "footnote_reference").len(), 3);
    assert_eq!(
        spans(&document, "footnote_definition"),
        [
            &Style::FootnoteDefinition {
                label: "later".into(),
                number: 1
            },
            &Style::FootnoteDefinition {
                label: "first".into(),
                number: 2
            },
        ]
    );
}

#[test]
fn undefined_footnotes_and_the_default_flavour_keep_the_marker_as_text() {
    assert!(github("A[^missing] note\n").contains("A[^missing] note"));
    assert!(!render_html("A[^1]\n\n[^1]: x\n").contains("mk-footnote"));
}

/// Tick the `index`th box drawn, as a host would with the reported offset.
fn tick(source: &str, index: usize, checked: bool) -> Option<String> {
    let document = parse_with(source, Flavor::Github);
    let offset = document.spans.iter().find_map(|span| match span.style {
        Style::Task {
            index: at, offset, ..
        } if at == index => Some(offset),
        _ => None,
    })?;
    let html = github(source);
    assert!(
        html.contains(&format!(
            "data-markit-task=\"{index}\" data-markit-task-offset=\"{offset}\""
        )),
        "{html}"
    );
    with_task(source, offset, checked)
}

fn boxes(source: &str) -> Vec<bool> {
    parse_with(source, Flavor::Github)
        .spans
        .iter()
        .filter_map(|span| match span.style {
            Style::Task { checked, .. } => Some(checked),
            _ => None,
        })
        .collect()
}

#[test]
fn a_tick_changes_only_its_own_box_in_quotes_numbered_lists_and_crlf_text() {
    let text = "> - [ ] quoted\r\n\r\n1. [x] numbered\r\n2. [ ] last\r\n";
    assert_eq!(
        tick(text, 0, true).unwrap(),
        "> - [x] quoted\r\n\r\n1. [x] numbered\r\n2. [ ] last\r\n"
    );
    assert_eq!(
        tick(text, 1, false).unwrap(),
        "> - [ ] quoted\r\n\r\n1. [ ] numbered\r\n2. [ ] last\r\n"
    );
    assert_eq!(
        tick(text, 2, true).unwrap(),
        "> - [ ] quoted\r\n\r\n1. [x] numbered\r\n2. [x] last\r\n"
    );
}

#[test]
fn the_box_ticked_is_the_box_drawn_even_with_code_and_nested_lists() {
    assert_eq!(
        tick("    - [ ] b\n\n1. - [ ] a", 0, true).unwrap(),
        "    - [ ] b\n\n1. - [x] a"
    );
    let mixed = "```\n- [ ] code\n```\n\n<!-- - [ ] note -->\n\n- [ ] one\n  - [ ] two";
    assert_eq!(boxes(mixed).len(), 2);
    assert_eq!(
        tick(mixed, 1, true).unwrap(),
        "```\n- [ ] code\n```\n\n<!-- - [ ] note -->\n\n- [ ] one\n  - [x] two"
    );
    let utf16 = "😀 **é**\n\n- [ ] 🦀 one\n- [ ] two\n";
    assert_eq!(
        tick(utf16, 1, true).unwrap(),
        "😀 **é**\n\n- [ ] 🦀 one\n- [x] two\n"
    );
}

#[test]
fn boxes_in_loose_lists_tick_and_stale_offsets_are_refused() {
    assert_eq!(
        tick("- [ ] one\n- [ ] two\n\n- [ ] three\n", 2, true).unwrap(),
        "- [ ] one\n- [ ] two\n\n- [x] three\n"
    );
    assert_eq!(
        tick("- [ ] one\n\n  more\n- [ ] two\n", 1, true).unwrap(),
        "- [ ] one\n\n  more\n- [x] two\n"
    );
    assert_eq!(with_task("- [ ] a", 1, true), None);
    assert_eq!(with_task("- [ ] a", 99, true), None);
}

#[test]
fn across_mixed_fragments_a_tick_flips_exactly_the_box_drawn() {
    let fragments = [
        "- [ ] a",
        "- [x] a",
        "```\n- [ ] a\n```",
        "    - [ ] a",
        "> - [ ] a",
        "> ```\n> - [ ] a\n> ```",
        "1. - [ ] a",
        "- [ ] a\n\n  more",
        "- a\n  - [ ] a",
        "<!-- - [ ] a -->",
        "1. step\n   ```\n   - [ ] a\n   ```\n   - [ ] a",
    ];
    let mut placed = 0;
    for one in fragments {
        for two in fragments {
            for newline in ["\n", "\r\n"] {
                let text = format!("{one}\n\n{two}\n").replace('\n', newline);
                let before = boxes(&text);
                for index in 0..before.len() {
                    let next =
                        tick(&text, index, !before[index]).expect("every drawn box is placed");
                    placed += 1;
                    let mut expected = before.clone();
                    expected[index] = !expected[index];
                    assert_eq!(boxes(&next), expected, "{text:?}");
                }
            }
        }
    }
    assert!(placed > 150, "{placed}");
}

#[test]
fn mentions_issues_commits_and_emoji_become_references_outside_code_and_links() {
    let sha = "4f2a9c1e8b7d6a5f4e3d2c1b0a9f8e7d6c5b4a39";
    let source = format!("See acme/billing-api#74, {sha}, octo/tools@4f2a9c1 and deadbeef by @octocat for @github/docs #12 :tada:\n\n`@octocat #12` [@octocat #12 :tada:](https://x.dev)\n");
    let html = github(&source);
    for (kind, target, shown) in [
        ("issue", "acme/billing-api#74", "acme/billing-api#74"),
        ("commit", sha, "<code>4f2a9c1</code>"),
        (
            "commit",
            "octo/tools@4f2a9c1",
            "<code>octo/tools@4f2a9c1</code>",
        ),
        ("mention", "octocat", "@octocat"),
        ("mention", "github/docs", "@github/docs"),
        ("issue", "#12", "#12"),
    ] {
        assert!(
            html.contains(&format!("class=\"mk-{kind}\" data-markit-reference=\"{target}\" data-reference-kind=\"{kind}\">{shown}</a>")),
            "{kind} {target}: {html}"
        );
    }
    assert!(!html.contains("data-markit-reference=\"deadbeef\""));
    assert!(html.contains("<code>@octocat #12</code>"), "{html}");
    assert!(html.contains("<a href=\"https://x.dev\">@octocat #12 <span role=\"img\" aria-label=\"tada\">🎉</span></a>"), "{html}");
    assert_eq!(html.matches("data-reference-kind=\"mention\"").count(), 2);
    let document = parse_with(&source, Flavor::Github);
    assert!(document.text.contains("🎉"));
    let references: Vec<_> = document
        .spans
        .iter()
        .filter_map(|span| match &span.style {
            Style::Reference {
                reference_kind,
                target,
            } => Some((reference_kind.as_str(), target.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        references,
        [
            ("issue", "acme/billing-api#74"),
            ("commit", sha),
            ("commit", "octo/tools@4f2a9c1"),
            ("mention", "octocat"),
            ("mention", "github/docs"),
            ("issue", "#12"),
        ]
    );
    // The default flavour links none of it.
    assert!(!render_html(&source).contains("mk-mention"));
}

#[test]
fn urls_and_email_addresses_keep_their_text_and_stay_single_links() {
    let html = github("Mail a@b.com or open https://github.com/o/r/issues/12#issuecomment-1 and https://x.dev/a:b:c\n");
    assert!(
        !html.contains("mk-mention")
            && !html.contains("mk-issue")
            && !html.contains("role=\"img\""),
        "{html}"
    );
    assert_eq!(html.matches("<a href").count(), 3, "{html}");
}

#[test]
fn closed_mermaid_fences_become_diagrams_with_their_code_as_fallback() {
    let source = "```mermaid\ngraph TD; A-->B\n```\n\n```mermaid\nopen";
    let html = github(source);
    assert!(html.starts_with("<div class=\"mk-diagram\" data-markit-diagram=\"mermaid\" data-markit-diagram-source=\"graph TD; A--&gt;B\n\">\n<section class=\"mk-codeblock\">"), "{html}");
    assert_eq!(
        html.matches("mk-diagram").count(),
        1,
        "an unclosed fence stays code: {html}"
    );
    let document = parse_with(source, Flavor::Github);
    let diagram = Style::Diagram {
        language: "mermaid".into(),
        source: "graph TD; A-->B\n".into(),
    };
    assert_eq!(text_of(&document, &diagram), "graph TD; A-->B\n");
    assert_eq!(spans(&document, "diagram").len(), 1);
    assert!(!render_html(source).contains("mk-diagram"));
}

#[test]
fn new_span_kinds_cross_the_binary_transport_unchanged() {
    let source = "> [!TIP]\n> x[^n]\n\n- [x] done @octocat\n\n<kbd>K</kbd> <video src=\"https://x.test/v.mp4\"></video>\n\n<picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"https://x.test/d.png\"><img src=\"https://x.test/l.png\"></picture>\n\n```mermaid\na\n```\n\n[^n]: note\n";
    let document = parse_with(source, Flavor::Github);
    for kind in [
        "alert",
        "footnote_reference",
        "footnote_definition",
        "task",
        "diagram",
        "video",
        "html",
        "dark_image",
        "reference",
    ] {
        assert!(!spans(&document, kind).is_empty(), "{kind}");
    }
    let bytes = markit::render_document_binary_with(source, Flavor::Github).unwrap();
    assert_eq!(
        mkbp::decode(&bytes),
        serde_json::to_value(&document).unwrap()
    );
}
