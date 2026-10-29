//! The agent flavour: GitHub's Markdown for coding agents' answers. It draws
//! everything the GitHub flavour draws except forge references, which have
//! no repository to point at in chat prose.
use markit::{parse_with, render_html, render_html_with, Document, Flavor, Style};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn references(document: &Document) -> Vec<(&str, &str)> {
    document
        .spans
        .iter()
        .filter_map(|span| match &span.style {
            Style::Reference {
                reference_kind,
                target,
            } => Some((reference_kind.as_str(), target.as_str())),
            _ => None,
        })
        .collect()
}

#[test]
fn agent_flavour_draws_footnotes_alerts_details_diagrams_and_tasks_exactly_like_github() {
    let source = "> [!NOTE]\n> Read this first.\n\n- [ ] one\n- [x] two\n\nShipped[^1] :tada:\n\n<details>\n<summary>Logs</summary>\n\nok <kbd>Ctrl</kbd>\n\n</details>\n\n```mermaid\ngraph TD; A-->B\n```\n\n<img src=\"https://x.test/a.png\">\n\n[^1]: Since **2.0**.\n";
    let html = render_html_with(source, Flavor::Agent);
    for drawn in [
        "data-markit-alert=\"note\"",
        "data-markit-task-offset=\"",
        "data-markit-footnote=\"1\"",
        "<section class=\"mk-footnotes\"",
        "<details>\n<summary>Logs</summary>",
        "<kbd>Ctrl</kbd>",
        "data-markit-diagram=\"mermaid\"",
        "aria-label=\"tada\">🎉",
    ] {
        assert!(html.contains(drawn), "{drawn}: {html}");
    }
    assert!(!html.contains("&lt;details"), "{html}");
    assert_eq!(html, render_html_with(source, Flavor::Github));
    assert_eq!(
        parse_with(source, Flavor::Agent),
        parse_with(source, Flavor::Github)
    );
}

#[test]
fn mentions_issue_numbers_and_commit_shas_stay_plain_text_in_agent_prose() {
    let source = format!(
        "Step #12 by @octocat for @github/docs fixes owner/repo#3 at {SHA}, owner/repo@a1b2c3d and a1b2c3d.\n\nNote[^n]\n\n[^n]: See #4 and @octocat.\n"
    );
    let github = render_html_with(&source, Flavor::Github);
    for kind in ["mention", "issue", "commit"] {
        assert!(
            github.contains(&format!("data-reference-kind=\"{kind}\"")),
            "{kind}: {github}"
        );
    }
    assert_eq!(references(&parse_with(&source, Flavor::Github)).len(), 9);

    let agent = render_html_with(&source, Flavor::Agent);
    assert!(!agent.contains("data-markit-reference"), "{agent}");
    assert!(
        agent.contains(&format!("<p>Step #12 by @octocat for @github/docs fixes owner/repo#3 at {SHA}, owner/repo@a1b2c3d and a1b2c3d.</p>")),
        "{agent}"
    );
    assert!(
        agent.contains("<p>See #4 and @octocat. <a class=\"mk-footnote-back\""),
        "{agent}"
    );
    let document = parse_with(&source, Flavor::Agent);
    assert_eq!(references(&document), []);
    assert!(document.text.contains(&format!("owner/repo#3 at {SHA}")));
}

#[test]
fn agent_prose_still_links_bare_urls_and_draws_emoji() {
    let html = render_html_with("Open https://x.dev/#12 :rocket: @octocat\n", Flavor::Agent);
    assert!(html.contains("<a href=\"https://x.dev/#12\">"), "{html}");
    assert!(html.contains("aria-label=\"rocket\">🚀"), "{html}");
    assert!(html.contains("@octocat"));
    assert!(!html.contains("mk-mention"));
}

#[test]
fn agent_flavour_is_selected_by_its_bridge_name_and_code() {
    let source = "Note[^1] for @octocat\n\n[^1]: Yes.\n";
    let expected = render_html_with(source, Flavor::Agent);
    for flavor in [Flavor::from_name("agent"), Flavor::from_code(2)] {
        let html = render_html_with(source, flavor.expect("known flavour"));
        assert_eq!(html, expected);
        assert!(html.contains("mk-footnotes") && !html.contains("mk-mention"));
    }
    // The default flavour still draws none of it.
    assert!(!render_html(source).contains("mk-footnotes"));
}
