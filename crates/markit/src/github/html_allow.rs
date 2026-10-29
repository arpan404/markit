//! Which raw HTML in forge text is drawn, and with which attributes. An
//! allowlist of the formatting subset people use on GitHub, matching ace's
//! forge renderer. Scripts, styles, iframes, forms, event handlers, inline
//! styles, classes and ids are never kept: a tag that is not allowed is shown
//! as its source text. Image, picture-source and video URLs never load
//! directly; they become `data-markit-*` attributes the host resolves.

/// Elements drawn with their children. Sorted for binary search.
const CONTAINERS: &[&str] = &[
    "a",
    "abbr",
    "b",
    "bdo",
    "blockquote",
    "caption",
    "center",
    "cite",
    "code",
    "dd",
    "del",
    "details",
    "dfn",
    "div",
    "dl",
    "dt",
    "em",
    "figcaption",
    "figure",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "i",
    "ins",
    "kbd",
    "li",
    "mark",
    "ol",
    "p",
    "picture",
    "pre",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "small",
    "span",
    "strike",
    "strong",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "time",
    "tr",
    "tt",
    "u",
    "ul",
    "var",
    "video",
];
/// Elements that never hold children. Sorted.
const VOIDS: &[&str] = &["br", "hr", "img", "source", "wbr"];
/// Elements that keep `align`.
const ALIGNED: &[&str] = &[
    "div", "h1", "h2", "h3", "h4", "h5", "h6", "img", "p", "td", "th",
];

/// An allowed element with the attributes it keeps, in output order. Values
/// are decoded text; the serializer escapes them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SafeElement {
    pub tag: &'static str,
    pub attrs: Vec<(&'static str, String)>,
    pub void: bool,
}

impl SafeElement {
    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// The interned name of an allowed tag, and whether it is void.
pub(crate) fn allowed(tag: &str) -> Option<(&'static str, bool)> {
    if let Ok(at) = CONTAINERS.binary_search(&tag) {
        return Some((CONTAINERS[at], false));
    }
    VOIDS.binary_search(&tag).ok().map(|at| (VOIDS[at], true))
}

/// A URL that may be followed or resolved: the renderer's link policy applied
/// to the decoded value, so encoded schemes are judged as a browser reads them.
fn safe_url(value: &str) -> Option<String> {
    let value = value.trim();
    crate::is_safe_url(value).then(|| value.to_owned())
}

fn dimension(value: &str) -> bool {
    let digits = value.strip_suffix('%').unwrap_or(value);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, "1"));
    (1..=5).contains(&whole.len())
        && whole.bytes().all(|b| b.is_ascii_digit())
        && !fraction.is_empty()
        && fraction.bytes().all(|b| b.is_ascii_digit())
}

fn count(value: &str) -> bool {
    (1..=4).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_digit())
}

/// A media URL the host may resolve: http(s), or a root-relative path on
/// the forge. Images, picture sources and videos never load directly.
fn safe_media_url(value: &str) -> Option<String> {
    let url = safe_url(value)?;
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    let web = matches!(scheme.as_deref(), Some("http" | "https"));
    let rooted = url.starts_with('/') && !url.starts_with("//");
    (web || rooted).then_some(url)
}

/// Each URL a `srcset` names, in order, without descriptors. As in the
/// HTML parser, a URL runs to whitespace, so it may hold commas.
fn srcset_urls(srcset: &str) -> Vec<&str> {
    let mut urls = Vec::new();
    let mut rest = srcset;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
        if rest.is_empty() {
            return urls;
        }
        let end = rest
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(rest.len());
        let token = &rest[..end];
        urls.push(token.trim_end_matches(','));
        rest = &rest[end..];
        if !token.ends_with(',') {
            // Skip this candidate's descriptors.
            rest = rest.find(',').map_or("", |at| &rest[at + 1..]);
        }
    }
}

fn clipped(value: &str) -> String {
    value.chars().take(1000).collect()
}

/// `tag` with `attrs` as it may be drawn, or `None` when it is not allowed.
pub(crate) fn safe_element(tag: &str, attrs: &[(String, String)]) -> Option<SafeElement> {
    let (tag, void) = allowed(tag)?;
    let get = |name: &str| {
        attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    let mut kept: Vec<(&'static str, String)> = Vec::new();
    match tag {
        "a" => {
            if let Some(href) = get("href").and_then(safe_url) {
                kept.push(("href", href));
            }
            if let Some(title) = get("title") {
                kept.push(("title", clipped(title)));
            }
        }
        "abbr" => {
            if let Some(title) = get("title") {
                kept.push(("title", clipped(title)));
            }
        }
        "bdo" => {
            if let Some(dir) = get("dir").filter(|dir| matches!(*dir, "ltr" | "rtl")) {
                kept.push(("dir", dir.into()));
            }
        }
        "details" => {
            if get("open").is_some() {
                kept.push(("open", String::new()));
            }
        }
        "img" => {
            let src = get("src").and_then(safe_media_url).unwrap_or_default();
            kept.push(("data-markit-src", src));
            kept.push(("alt", get("alt").map(clipped).unwrap_or_default()));
            if let Some(title) = get("title") {
                kept.push(("title", clipped(title)));
            }
            for name in ["width", "height"] {
                if let Some(value) = get(name).filter(|value| dimension(value)) {
                    kept.push((
                        if name == "width" { "width" } else { "height" },
                        value.into(),
                    ));
                }
            }
        }
        "ol" => {
            if let Some(start) = get("start").filter(|value| count(value)) {
                kept.push(("start", start.into()));
            }
            if let Some(kind) =
                get("type").filter(|value| matches!(*value, "1" | "a" | "A" | "i" | "I"))
            {
                kept.push(("type", kind.into()));
            }
        }
        "source" => {
            // A `<picture>` light/dark variant: the first safe candidate only.
            if let Some(url) = get("srcset")
                .into_iter()
                .flat_map(srcset_urls)
                .find_map(safe_media_url)
            {
                kept.push(("data-markit-srcset", url));
            }
            if let Some(media) = get("media") {
                kept.push(("media", clipped(media)));
            }
            if let Some(kind) = get("type") {
                kept.push(("type", clipped(kind)));
            }
        }
        "td" | "th" => {
            for name in ["colspan", "rowspan"] {
                if let Some(value) = get(name).filter(|value| count(value)) {
                    kept.push((
                        if name == "colspan" {
                            "colspan"
                        } else {
                            "rowspan"
                        },
                        value.into(),
                    ));
                }
            }
        }
        "time" => {
            if let Some(datetime) = get("datetime") {
                kept.push(("datetime", clipped(datetime)));
            }
        }
        "video" => {
            let src = get("src").and_then(safe_media_url).unwrap_or_default();
            kept.push(("data-markit-src", src));
        }
        _ => {}
    }
    if ALIGNED.contains(&tag) {
        if let Some(align) = get("align").map(str::to_ascii_lowercase) {
            if matches!(align.as_str(), "left" | "right" | "center" | "justify") {
                kept.push(("align", align));
            }
        }
    }
    Some(SafeElement {
        tag,
        attrs: kept,
        void,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect()
    }

    #[test]
    fn lists_are_sorted_for_lookup() {
        assert!(CONTAINERS.windows(2).all(|w| w[0] < w[1]));
        assert!(VOIDS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn only_allowlisted_attributes_and_safe_values_survive() {
        let element = safe_element(
            "img",
            &attrs(&[
                ("src", "https://ok.example/a.png"),
                ("onerror", "alert(1)"),
                ("style", "x"),
                ("width", "120"),
                ("height", "1e9"),
                ("align", "CENTER"),
            ]),
        )
        .unwrap();
        assert_eq!(
            element.attrs,
            [
                ("data-markit-src", "https://ok.example/a.png".to_string()),
                ("alt", String::new()),
                ("width", "120".into()),
                ("align", "center".into()),
            ]
        );
        assert!(safe_element("script", &[]).is_none());
        assert!(safe_element("iframe", &[]).is_none());
        let link = safe_element("a", &attrs(&[("href", "javascript:alert(1)")])).unwrap();
        assert!(link.attr("href").is_none());
    }
}
