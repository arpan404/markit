//! Which Markdown dialect a render uses. The default is agent prose: raw HTML
//! is shown as text. The GitHub flavour is for PR, issue and comment bodies.
use pulldown_cmark::Options;

/// Rendering dialect, chosen per render or per stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Flavor {
    /// Agent prose. Raw HTML is escaped; no footnotes, alerts or forge links.
    #[default]
    Default,
    /// GitHub-flavoured Markdown as GitHub draws it: footnotes, `> [!NOTE]`
    /// alerts, an allowlisted HTML subset, task source offsets, `@mentions`,
    /// `#123`, commit SHAs, emoji shortcodes and ```` ```mermaid ```` diagrams.
    Github,
}

impl Flavor {
    /// `"default"` or `"github"`, as the web, C and JNI bridges name them.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "" | "default" => Some(Self::Default),
            "github" | "gfm" => Some(Self::Github),
            _ => None,
        }
    }

    /// `0` default, `1` GitHub, as the C and JNI bridges number them.
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Default),
            1 => Some(Self::Github),
            _ => None,
        }
    }

    pub(crate) fn is_github(self) -> bool {
        self == Self::Github
    }

    pub(crate) fn options(self) -> Options {
        let base =
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
        match self {
            Self::Default => base,
            Self::Github => base | Options::ENABLE_FOOTNOTES | Options::ENABLE_GFM,
        }
    }
}
