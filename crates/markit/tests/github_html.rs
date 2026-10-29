//! GitHub flavour raw HTML: the allowlisted subset is drawn, everything else
//! is inert text. Cases follow ace's forge sanitizer and prose tests.
use markit::{parse_with, render_html, render_html_with, Flavor, Style};

fn github(source: &str) -> String {
    render_html_with(source, Flavor::Github)
}

/// Every attribute name in `html` (crude but enough: our output always
/// quotes values with `"` and escapes `"` inside them).
fn attribute_names(html: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        rest = &rest[at + 1..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        let mut inside = false;
        let mut word = String::new();
        for c in tag.chars() {
            match c {
                '"' => inside = !inside,
                '=' if !inside => {
                    names.push(word.trim().rsplit(' ').next().unwrap_or("").to_string());
                    word.clear();
                }
                c if !inside => word.push(c),
                _ => {}
            }
        }
        rest = &rest[end..];
    }
    names
}

/// Every `href`, `src`, `srcset` and `data-markit-src*` value in `html`.
fn urls(html: &str) -> Vec<String> {
    let mut found = Vec::new();
    for name in [
        " href=\"",
        " src=\"",
        " srcset=\"",
        " data-markit-src=\"",
        " data-markit-srcset=\"",
    ] {
        let mut rest = html;
        while let Some(at) = rest.find(name) {
            rest = &rest[at + name.len()..];
            let end = rest.find('"').unwrap();
            found.push(rest[..end].to_string());
            rest = &rest[end..];
        }
    }
    found
}

#[test]
fn default_flavour_still_shows_raw_html_as_text() {
    let source = "<details><summary>x</summary></details>\n\n<b>bold</b>\n";
    let html = render_html(source);
    assert!(!html.contains("<details>"));
    assert!(html.contains("&lt;details&gt;"));
    assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"));
}

#[test]
fn details_hold_the_markdown_blocks_until_they_close() {
    let html =
        github("<details>\n<summary>More</summary>\n\nhidden **bold**\n\n</details>\n\nafter\n");
    assert!(html.starts_with("<details>"), "{html}");
    let inside = &html[..html.find("</details>").unwrap()];
    assert!(inside.contains("<summary>More</summary>"));
    assert!(inside.contains("<strong>bold</strong>"));
    assert!(html[html.find("</details>").unwrap()..].contains("<p>after</p>"));
}

#[test]
fn styles_handlers_and_unknown_attributes_are_dropped_but_alignment_stays() {
    let html = github("<p align=\"center\" style=\"color:red\" onclick=\"steal()\" class=\"x\" id=\"y\">Hi <b>there</b></p>\n");
    assert!(
        html.contains("<p align=\"center\">Hi <b>there</b></p>"),
        "{html}"
    );
    for name in attribute_names(&html) {
        assert!(
            !name.starts_with("on") && name != "style" && name != "class" && name != "id",
            "{name} in {html}"
        );
    }
}

#[test]
fn scripts_frames_forms_styles_svg_and_math_are_shown_as_text() {
    for (source, shown) in [
        (
            "<script>alert(1)</script>",
            "&lt;script&gt;alert(1)&lt;/script&gt;",
        ),
        (
            "<iframe src=\"https://evil.example\"></iframe>",
            "&lt;iframe src=&quot;https://evil.example&quot;&gt;",
        ),
        (
            "<form action=\"https://evil.example\"><input name=\"q\"></form>",
            "&lt;form",
        ),
        ("<style>body{display:none}</style>", "&lt;style&gt;"),
        ("<svg><script>alert(5)</script></svg>", "&lt;svg&gt;"),
        ("<math><mi>x</mi></math>", "&lt;math&gt;"),
        ("<object data=\"x\"></object>", "&lt;object"),
    ] {
        let html = github(&format!("{source}\n"));
        assert!(html.contains(shown), "{source}: {html}");
        for tag in [
            "<script", "<iframe", "<form", "<style", "<svg", "<math", "<input", "<object",
        ] {
            assert!(!html.contains(tag), "{source}: {html}");
        }
    }
}

#[test]
fn links_with_hidden_or_unsafe_schemes_are_never_live() {
    for source in [
        "<a href=\"jav&#x09;ascript:alert(1)\">x</a>",
        "<a href=\" javascript:alert(1)\">x</a>",
        "<a href=\"JaVaScRiPt&colon;alert(1)\">x</a>",
        "<a href=\"javascript&#58;alert(1)\">x</a>",
        "<a href=\"data:text/html,<script>alert(1)</script>\">x</a>",
        "<a href=\"vbscript:msgbox(1)\">x</a>",
        "<a href=\"//evil.example/x\">x</a>",
        "<a href=\"\\\\evil.example\">x</a>",
        "<a href=\"java\nscript:alert(1)\">x</a>",
    ] {
        let html = github(&format!("{source}\n"));
        assert!(!html.contains("href"), "{source}: {html}");
        assert!(html.contains("<a>x</a>"), "{source}: {html}");
    }
    let html = github("<a href=\"https://ok.dev\" target=\"_top\" onclick=\"x()\">ok</a>\n");
    assert!(html.contains("<a href=\"https://ok.dev\">ok</a>"), "{html}");
}

#[test]
fn the_first_of_repeated_attributes_wins() {
    let html = github("<a href=\"https://ok.example\" href=\"javascript:alert(1)\">x</a>\n");
    assert!(html.contains("href=\"https://ok.example\""));
    assert!(!html.contains("javascript"));
}

#[test]
fn quote_joined_and_backtick_attributes_never_produce_handlers() {
    for source in [
        "<img src=\"https://ok.example/a.png\"onerror=alert(1)>",
        "<img src=`x` onerror=alert(1)>",
        "<img src=x onerror=\"alert(1)\"/>",
        "<img/src=x/onerror=alert(1)>",
        "<IMG SRC=x ONERROR=alert(1)>",
    ] {
        let html = github(&format!("{source}\n"));
        assert!(
            !html.contains("<img") || !html.contains(" onerror="),
            "{source}: {html}"
        );
        for name in attribute_names(&html) {
            assert!(!name.starts_with("on"), "{source}: {html}");
        }
    }
}

#[test]
fn images_videos_and_picture_sources_never_load_directly() {
    let html = github(concat!(
        "<picture>\n",
        "<source media=\"(prefers-color-scheme: dark)\" srcset=\"https://x.test/dark.png 2x, https://x.test/other.png\">\n",
        "<img src=\"https://x.test/light.png\" alt=\"Logo\" width=\"120\" height=\"40%\">\n",
        "</picture>\n\n",
        "<video src=\"https://x.test/clip.mp4\"><b>fallback</b></video>\n",
    ));
    assert!(!html.contains(" src=\""), "{html}");
    assert!(!html.contains(" srcset=\""), "{html}");
    assert!(html.contains("<source data-markit-srcset=\"https://x.test/dark.png\" media=\"(prefers-color-scheme: dark)\">"), "{html}");
    assert!(html.contains("<img data-markit-src=\"https://x.test/light.png\" alt=\"Logo\" width=\"120\" height=\"40%\" loading=\"lazy\">"), "{html}");
    assert!(html.contains("<video data-markit-src=\"https://x.test/clip.mp4\" controls=\"\" preload=\"none\"></video>"), "{html}");
    assert!(
        !html.contains("fallback"),
        "video content is dropped: {html}"
    );
}

#[test]
fn unsafe_media_sources_resolve_to_nothing() {
    let html = github(concat!(
        "<picture><source srcset=\"javascript:alert(1)\"><source srcset=\"data:image/svg+xml,<svg onload=alert(1)>\"><img src=\"javascript:x\"></picture>\n\n",
        "<video src=\"data:video/mp4,x\"></video>\n"
    ));
    for url in urls(&html) {
        assert!(
            url.is_empty() || url.starts_with("https:"),
            "{url} in {html}"
        );
    }
    assert!(!html.contains("javascript"));
    assert!(!html.contains("data:"));
}

#[test]
fn unclosed_tags_close_at_their_container_and_stray_end_tags_vanish() {
    let html = github("before <b>bold <a href=\"https://ok.example\">link\n\nnext</i> para\n");
    assert!(
        html.contains("<p>before <b>bold <a href=\"https://ok.example\">link</a></b></p>"),
        "{html}"
    );
    assert!(html.contains("<p>next para</p>"), "{html}");
}

#[test]
fn entities_in_text_stay_text_and_cut_off_tags_are_escaped() {
    let html = github("<p>&lt;script&gt; &copy; &#x1F389; & 1 < 2</p>\n");
    assert!(
        html.contains("<p>&lt;script&gt; &copy; &#x1F389; &amp; 1 &lt; 2</p>"),
        "{html}"
    );
    assert!(!html.contains("<script"));
    let html = github("a <b\n");
    assert_eq!(html, "<p>a &lt;b</p>\n");
}

#[test]
fn comments_are_hidden_and_never_swallow_following_markdown() {
    let html = github("<!-- a note for reviewers -->\n\nvisible\n");
    assert!(!html.contains("reviewers"), "{html}");
    assert!(html.contains("<p>visible</p>"));
}

#[test]
fn formatting_tags_are_drawn_and_mapped_onto_native_spans() {
    let source = "Press <kbd>Ctrl</kbd>, H<sub>2</sub>O, x<sup>2</sup>, <mark>hi</mark>, <ins>new</ins>, <del>old</del>, <ruby>漢<rt>kan</rt></ruby>\n";
    let html = github(source);
    for tag in [
        "<kbd>Ctrl</kbd>",
        "<sub>2</sub>",
        "<sup>2</sup>",
        "<mark>hi</mark>",
        "<ins>new</ins>",
        "<del>old</del>",
        "<ruby>漢<rt>kan</rt></ruby>",
    ] {
        assert!(html.contains(tag), "{tag}: {html}");
    }
    let document = parse_with(source, Flavor::Github);
    let text = &document.text;
    let tagged = |name: &str| {
        document
            .spans
            .iter()
            .find(|span| matches!(&span.style, Style::Html { tag } if tag == name))
            .map(|span| {
                text.encode_utf16()
                    .skip(span.start)
                    .take(span.end - span.start)
                    .collect::<Vec<_>>()
            })
            .map(|units| String::from_utf16(&units).unwrap())
    };
    assert_eq!(tagged("kbd").as_deref(), Some("Ctrl"));
    assert_eq!(tagged("sub").as_deref(), Some("2"));
    assert_eq!(tagged("mark").as_deref(), Some("hi"));
    assert!(document
        .spans
        .iter()
        .any(|span| span.style == Style::Strikethrough));
    assert!(!text.contains('<'), "{text}");
}

#[test]
fn native_documents_carry_html_images_with_their_dark_variant_and_videos() {
    let document = parse_with(
        "<picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"https://x.test/d.png\"><img src=\"https://x.test/l.png\" alt=\"Logo\"></picture>\n\n<video src=\"https://x.test/v.mp4\"></video>\n",
        Flavor::Github,
    );
    let styles: Vec<_> = document.spans.iter().map(|span| &span.style).collect();
    assert!(styles.contains(&&Style::Image {
        url: "https://x.test/l.png".into()
    }));
    assert!(styles.contains(&&Style::DarkImage {
        url: "https://x.test/d.png".into()
    }));
    assert!(styles.contains(&&Style::Video {
        url: "https://x.test/v.mp4".into()
    }));
    assert!(
        document.text.starts_with("Logo\n[video]"),
        "{:?}",
        document.text
    );
}

#[test]
fn raw_link_and_code_text_is_never_linked_again() {
    let html = github("<a href=\"https://ok.dev\">ping @octocat about #12 https://x.dev</a> and <code>@octocat #12</code>\n");
    let link = &html[html.find("<a href").unwrap()..html.find("</a>").unwrap()];
    assert!(!link[1..].contains("<a"), "{html}");
    assert!(html.contains("<code>@octocat #12</code>"), "{html}");
}
