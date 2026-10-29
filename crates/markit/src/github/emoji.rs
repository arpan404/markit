//! GitHub emoji shortcodes (`:tada:`) from GitHub's gemoji list, as a
//! compact sorted table. Shortcodes GitHub draws as its own images
//! (`:octocat:`) are not in the list and stay text.
use super::emoji_table::{DATA, STARTS};

/// The `code emoji` line starting at byte `start`, without its newline.
fn line(start: u32) -> (&'static str, &'static str) {
    let rest = &DATA[start as usize..];
    let line = &rest[..rest.find('\n').unwrap_or(rest.len())];
    line.split_once(' ').unwrap_or((line, ""))
}

/// The emoji a shortcode names, without its colons.
pub(crate) fn lookup(name: &str) -> Option<&'static str> {
    let at = STARTS
        .binary_search_by(|&start| line(start).0.cmp(name))
        .ok()?;
    Some(line(STARTS[at]).1)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_table_matches_every_gemoji_shortcode() {
        let mut count = 0;
        for emoji in emojis::iter() {
            for code in emoji.shortcodes() {
                let expected = emojis::get_by_shortcode(code).map(emojis::Emoji::as_str);
                assert_eq!(super::lookup(code), expected, "{code}");
                count += 1;
            }
        }
        assert_eq!(
            count,
            super::STARTS.len(),
            "regenerate with the emoji_table example"
        );
        assert_eq!(super::lookup("not_an_emoji"), None);
    }
}
