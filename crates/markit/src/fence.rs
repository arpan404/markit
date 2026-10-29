use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FenceInfo {
    pub language: String,
    pub file: Option<String>,
    pub diff: bool,
}

pub fn fence_info(info: &str) -> FenceInfo {
    // Quotes group filename attributes; they do not introduce HTML or commands.
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    for c in info.chars() {
        if quote == Some(c) {
            quote = None;
        } else if quote.is_none() && (c == '\'' || c == '"') {
            quote = Some(c);
        } else if c.is_whitespace() && quote.is_none() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    let mut file = None;
    words.retain(|word| {
        if let Some((key, value)) = word.split_once('=') {
            if ["title", "file", "filename", "name"].contains(&key) {
                file = Some(value.to_owned());
                return false;
            }
        }
        true
    });
    let mut language = words.first().cloned().unwrap_or_default();
    if let Some((lang, path)) = language.split_once(':') {
        if !path.is_empty() && file.is_none() {
            file = Some(path.into());
            language = lang.into();
        }
    }
    let path_like = |word: &str| {
        word.contains('/')
            || word.contains('\\')
            || word.rsplit_once('.').is_some_and(|(stem, ext)| {
                !stem.is_empty() && !ext.is_empty() && ext.chars().all(char::is_alphanumeric)
            })
    };
    if file.is_none() {
        if path_like(&language) {
            file = Some(std::mem::take(&mut language));
        } else if let Some(path) = words.get(1).filter(|path| path_like(path)) {
            file = Some(path.clone());
        }
    }
    if language.is_empty() {
        language = file
            .as_deref()
            .and_then(|file| file.rsplit_once('.').map(|(_, ext)| ext))
            .unwrap_or("")
            .into();
    }
    language = crate::highlight::language_alias(&language);
    let diff = language == "diff"
        || file
            .as_deref()
            .is_some_and(|file| file.ends_with(".diff") || file.ends_with(".patch"));
    FenceInfo {
        language,
        file,
        diff,
    }
}

/// What a line of a diff fence is, for its tint and its `<ins>`/`<del>` markup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Context,
    Addition,
    Deletion,
    Hunk,
    Meta,
}

impl DiffKind {
    /// Class suffix in `mk-diff-*`.
    pub fn class(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Addition => "addition",
            Self::Deletion => "deletion",
            Self::Hunk => "hunk",
            Self::Meta => "meta",
        }
    }
    fn code(self) -> u32 {
        match self {
            Self::Context => 0,
            Self::Addition => 1,
            Self::Deletion => 2,
            Self::Hunk => 3,
            Self::Meta => 4,
        }
    }
}

/// One diff line: its kind and its old/new line numbers when hunk headers give them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffRow {
    pub kind: DiffKind,
    pub old: Option<u32>,
    pub new: Option<u32>,
}

/// Reads a unified diff one line at a time. A line's kind depends only on the
/// lines before it, so a streamed fence never reclassifies a finished line.
/// Inside a hunk the `@@ -a,b +c,d @@` counts decide whether `---` is a removed
/// line; after them (or with no counts) `---`/`+++`/`diff ` start a file header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffCursor {
    old: Option<u32>,
    new: Option<u32>,
    old_left: u32,
    new_left: u32,
    pub additions: u32,
    pub deletions: u32,
}

const DIFF_HEADERS: [&str; 14] = [
    "diff ",
    "index ",
    "---",
    "+++",
    "new file mode",
    "deleted file mode",
    "old mode",
    "new mode",
    "similarity index",
    "dissimilarity index",
    "rename from",
    "rename to",
    "copy from",
    "copy to",
];

/// `@@ -a[,b] +c[,d] @@`: old start, old count, new start, new count.
fn hunk_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    let rest = line.strip_prefix("@@ -")?;
    let (ranges, _) = rest.split_once(" @@")?;
    let (old, new) = ranges.split_once(" +")?;
    let range = |text: &str| -> Option<(u32, u32)> {
        match text.split_once(',') {
            Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
            None => Some((text.parse().ok()?, 1)),
        }
    };
    let (old_start, old_count) = range(old)?;
    let (new_start, new_count) = range(new)?;
    Some((old_start, old_count, new_start, new_count))
}

impl DiffCursor {
    /// Classify the next line (with or without its line ending) and advance.
    pub fn row(&mut self, line: &str) -> DiffRow {
        let text = line.trim_end_matches(['\n', '\r']);
        let row = |kind, old, new| DiffRow { kind, old, new };
        if text.starts_with("@@") {
            match hunk_header(text) {
                Some((old, old_left, new, new_left)) => {
                    *self = Self {
                        old: Some(old),
                        new: Some(new),
                        old_left,
                        new_left,
                        ..*self
                    };
                }
                None => {
                    *self = Self {
                        old: None,
                        new: None,
                        old_left: 0,
                        new_left: 0,
                        ..*self
                    };
                }
            }
            return row(DiffKind::Hunk, None, None);
        }
        let in_hunk = self.old_left > 0 || self.new_left > 0;
        if text.starts_with('\\') {
            return row(DiffKind::Meta, None, None);
        }
        if !in_hunk && DIFF_HEADERS.iter().any(|header| text.starts_with(header)) {
            if text.starts_with("diff ") {
                self.old = None;
                self.new = None;
            }
            return row(DiffKind::Meta, None, None);
        }
        match text.as_bytes().first() {
            Some(b'+') => {
                self.additions += 1;
                self.new_left = self.new_left.saturating_sub(1);
                let new = self.new;
                self.new = new.map(|n| n + 1);
                row(DiffKind::Addition, None, new)
            }
            Some(b'-') => {
                self.deletions += 1;
                self.old_left = self.old_left.saturating_sub(1);
                let old = self.old;
                self.old = old.map(|n| n + 1);
                row(DiffKind::Deletion, old, None)
            }
            _ => {
                self.old_left = self.old_left.saturating_sub(1);
                self.new_left = self.new_left.saturating_sub(1);
                let (old, new) = (self.old, self.new);
                self.old = old.map(|n| n + 1);
                self.new = new.map(|n| n + 1);
                row(DiffKind::Context, old, new)
            }
        }
    }
}

/// Packed diff rows for the web runtime: `[additions, deletions]`, then
/// `kind, old + 1, new + 1` per line (0 = no number). A final line with no
/// characters (after the last newline) has no row.
pub fn diff_rows_packed(code: &str) -> Vec<u32> {
    let mut cursor = DiffCursor::default();
    let mut words = vec![0, 0];
    for line in code.split_inclusive('\n') {
        let row = cursor.row(line);
        words.extend([
            row.kind.code(),
            row.old.map_or(0, |n| n + 1),
            row.new.map_or(0, |n| n + 1),
        ]);
    }
    words[0] = cursor.additions;
    words[1] = cursor.deletions;
    words
}

#[cfg(test)]
mod diff_tests {
    use super::*;

    fn rows(code: &str) -> Vec<(DiffKind, Option<u32>, Option<u32>)> {
        let mut cursor = DiffCursor::default();
        code.split_inclusive('\n')
            .map(|line| {
                let row = cursor.row(line);
                (row.kind, row.old, row.new)
            })
            .collect()
    }

    #[test]
    fn hunk_headers_number_old_and_new_lines() {
        use DiffKind::*;
        let diff = "diff --git a/f b/f\nindex 1..2\n--- a/f\n+++ b/f\n@@ -10,3 +10,4 @@ fn x\n ctx\n-old\n+new\n+more\n ctx\n";
        assert_eq!(
            rows(diff),
            vec![
                (Meta, None, None),
                (Meta, None, None),
                (Meta, None, None),
                (Meta, None, None),
                (Hunk, None, None),
                (Context, Some(10), Some(10)),
                (Deletion, Some(11), None),
                (Addition, None, Some(11)),
                (Addition, None, Some(12)),
                (Context, Some(12), Some(13)),
            ]
        );
    }

    #[test]
    fn removed_lines_that_look_like_headers_stay_removals_inside_a_hunk() {
        use DiffKind::*;
        let diff = "@@ -1,2 +1,0 @@\n--- a comment\n-x\n--- a/next\n+++ b/next\n";
        assert_eq!(
            rows(diff),
            vec![
                (Hunk, None, None),
                (Deletion, Some(1), None),
                (Deletion, Some(2), None),
                (Meta, None, None),
                (Meta, None, None),
            ]
        );
    }

    #[test]
    fn diffs_without_hunk_headers_are_tinted_but_unnumbered() {
        use DiffKind::*;
        assert_eq!(
            rows("-old\n+new\n same\n\\ No newline at end of file\n"),
            vec![
                (Deletion, None, None),
                (Addition, None, None),
                (Context, None, None),
                (Meta, None, None),
            ]
        );
    }

    #[test]
    fn packed_rows_count_changes_and_offset_numbers() {
        assert_eq!(
            diff_rows_packed("@@ -0,0 +1,2 @@\n+a\n+b\n"),
            vec![2, 0, 3, 0, 0, 1, 0, 2, 1, 0, 3]
        );
    }

    #[test]
    fn a_line_keeps_its_kind_as_later_lines_arrive() {
        let diff = "--- a/f\n+++ b/f\n@@ -1,2 +1,2 @@\n-a\n+b\n c\n@@ -9 +9 @@\n-x\n+y\n";
        let full = rows(diff);
        for end in 0..diff.len() {
            let prefix = rows(&diff[..end]);
            let complete = diff[..end].matches('\n').count();
            assert_eq!(&prefix[..complete], &full[..complete]);
        }
    }
}
