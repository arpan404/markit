//! Which Markdown dialect a render uses. The default is the plainest: raw
//! HTML is shown as text. The GitHub flavour is for PR, issue and comment
//! bodies; the agent flavour is GitHub's Markdown for chat answers, where
//! there is no repository to link references into.
use pulldown_cmark::Options;

/// Rendering dialect, chosen per render or per stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Flavor {
    /// CommonMark with tables, strikethrough and task lists. Raw HTML is
    /// escaped; no footnotes, alerts or forge links.
    #[default]
    Default,
    /// GitHub-flavoured Markdown as GitHub draws it: footnotes, `> [!NOTE]`
    /// alerts, an allowlisted HTML subset, task source offsets, `@mentions`,
    /// `#123`, commit SHAs, emoji shortcodes and ```` ```mermaid ```` diagrams.
    Github,
    /// Everything [`Flavor::Github`] draws except forge references:
    /// `@mentions`, `#123`, `owner/repo#123`, commit SHAs and
    /// `owner/repo@sha` stay plain text. For coding agents' answers, where
    /// "step #1" or a hash in a log names no repository.
    Agent,
}

impl Flavor {
    /// `"default"`, `"github"` or `"agent"`, as the web, C and JNI bridges
    /// name them.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "" | "default" => Some(Self::Default),
            "github" | "gfm" => Some(Self::Github),
            "agent" => Some(Self::Agent),
            _ => None,
        }
    }

    /// `0` default, `1` GitHub, `2` agent, as the C and JNI bridges number
    /// them.
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Default),
            1 => Some(Self::Github),
            2 => Some(Self::Agent),
            _ => None,
        }
    }

    /// Drawn by the GitHub pipeline: footnotes, alerts, allowlisted HTML,
    /// task source offsets, emoji shortcodes and diagrams.
    pub(crate) fn github_markdown(self) -> bool {
        matches!(self, Self::Github | Self::Agent)
    }

    /// Links `@mentions`, `#123`, commit SHAs and their cross-repo forms.
    pub(crate) fn forge_references(self) -> bool {
        self == Self::Github
    }

    pub(crate) fn options(self) -> Options {
        let base =
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
        if self.github_markdown() {
            base | Options::ENABLE_FOOTNOTES | Options::ENABLE_GFM
        } else {
            base
        }
    }
}
