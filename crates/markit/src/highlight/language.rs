//! Language names, aliases, and the lexical profile each language uses.
//! The scanner is table driven: a language is a set of comment, string and
//! keyword rules, so adding one is a table entry rather than scanner code.
use super::{keywords, keywords_extra};
use std::borrow::Cow;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LineMarker {
    Never,
    Always,
    /// Only at the start of the source or after whitespace, so `a#b`,
    /// `x--` and `don't` style text in unknown languages stays plain.
    AfterSpace,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockComment {
    Never,
    Flat,
    Nested,
}

pub(super) struct Profile {
    /// `#` line comments.
    pub hash: LineMarker,
    /// `//` line comments.
    pub slash: bool,
    /// `/* */` block comments.
    pub block: BlockComment,
    /// `--` line comments. `AfterSpace` also needs whitespace after them.
    pub dash: LineMarker,
    /// `;` line comments (INI).
    pub semicolon: LineMarker,
    /// Lua long brackets: `--[[ comment ]]` and `[[ string ]]`.
    pub long_brackets: bool,
    /// HTML/JSX comments and tag/attribute tones.
    pub markup: bool,
    /// `"""`/`'''` strings.
    pub triple_quotes: bool,
    /// Rust raw strings and lifetimes.
    pub rust: bool,
    /// Quoted strings may span lines. Otherwise an unterminated quote ends
    /// at the line break instead of colouring the rest of the file.
    pub multiline_strings: bool,
    /// An apostrophe right after a word character is text, not a quote.
    pub apostrophe_text: bool,
    /// A word before `=` is a property (`key = value` files).
    pub assign_property: bool,
    pub keywords: fn(&str) -> bool,
}

const C_LIKE: Profile = Profile {
    hash: LineMarker::Never,
    slash: true,
    block: BlockComment::Flat,
    dash: LineMarker::Never,
    semicolon: LineMarker::Never,
    long_brackets: false,
    markup: false,
    triple_quotes: false,
    rust: false,
    multiline_strings: true,
    apostrophe_text: false,
    assign_property: false,
    keywords: keywords::c_like,
};

/// `#` comments only, as in scripting and configuration languages.
const HASH: Profile = Profile {
    hash: LineMarker::Always,
    slash: false,
    block: BlockComment::Never,
    ..C_LIKE
};

const NO_COMMENTS: Profile = Profile {
    slash: false,
    block: BlockComment::Never,
    ..C_LIKE
};

/// Line-oriented text formats: strings end at the line, apostrophes in
/// prose are not quotes.
const TEXTUAL: Profile = Profile {
    multiline_strings: false,
    apostrophe_text: true,
    ..HASH
};

static PROFILES: &[(&str, Profile)] = &[
    ("typescript", C_LIKE),
    ("javascript", C_LIKE),
    (
        "tsx",
        Profile {
            markup: true,
            ..C_LIKE
        },
    ),
    (
        "jsx",
        Profile {
            markup: true,
            ..C_LIKE
        },
    ),
    (
        "python",
        Profile {
            triple_quotes: true,
            keywords: keywords::python,
            ..HASH
        },
    ),
    (
        "bash",
        Profile {
            keywords: keywords::bash,
            ..HASH
        },
    ),
    (
        "rust",
        Profile {
            block: BlockComment::Nested,
            rust: true,
            keywords: keywords::rust,
            ..C_LIKE
        },
    ),
    ("go", C_LIKE),
    ("java", C_LIKE),
    (
        "kotlin",
        Profile {
            triple_quotes: true,
            ..C_LIKE
        },
    ),
    (
        "swift",
        Profile {
            triple_quotes: true,
            keywords: keywords::swift,
            ..C_LIKE
        },
    ),
    ("c", C_LIKE),
    ("cpp", C_LIKE),
    ("csharp", C_LIKE),
    (
        "json",
        Profile {
            keywords: keywords::json,
            ..C_LIKE
        },
    ),
    (
        "json5",
        Profile {
            keywords: keywords::json,
            ..C_LIKE
        },
    ),
    (
        "yaml",
        Profile {
            keywords: keywords::yaml,
            ..HASH
        },
    ),
    ("markdown", NO_COMMENTS),
    (
        "sql",
        Profile {
            dash: LineMarker::Always,
            keywords: keywords::sql,
            ..C_LIKE
        },
    ),
    ("css", C_LIKE),
    (
        "html",
        Profile {
            markup: true,
            ..NO_COMMENTS
        },
    ),
    ("diff", C_LIKE),
    ("clike", C_LIKE),
    (
        "toml",
        Profile {
            triple_quotes: true,
            multiline_strings: false,
            assign_property: true,
            keywords: keywords_extra::toml,
            ..HASH
        },
    ),
    (
        "ini",
        Profile {
            hash: LineMarker::AfterSpace,
            semicolon: LineMarker::AfterSpace,
            assign_property: true,
            keywords: keywords_extra::ini,
            ..TEXTUAL
        },
    ),
    (
        "ruby",
        Profile {
            keywords: keywords_extra::ruby,
            ..HASH
        },
    ),
    (
        "php",
        Profile {
            hash: LineMarker::Always,
            keywords: keywords_extra::php,
            ..C_LIKE
        },
    ),
    (
        "lua",
        Profile {
            dash: LineMarker::Always,
            long_brackets: true,
            keywords: keywords_extra::lua,
            ..NO_COMMENTS
        },
    ),
    (
        "dockerfile",
        Profile {
            hash: LineMarker::AfterSpace,
            keywords: keywords_extra::dockerfile,
            ..TEXTUAL
        },
    ),
    (
        "makefile",
        Profile {
            assign_property: true,
            keywords: keywords_extra::makefile,
            ..TEXTUAL
        },
    ),
    (
        "graphql",
        Profile {
            triple_quotes: true,
            keywords: keywords_extra::graphql,
            ..HASH
        },
    ),
    (
        "scala",
        Profile {
            block: BlockComment::Nested,
            triple_quotes: true,
            keywords: keywords_extra::scala,
            ..C_LIKE
        },
    ),
    (
        "dart",
        Profile {
            block: BlockComment::Nested,
            triple_quotes: true,
            keywords: keywords_extra::dart,
            ..C_LIKE
        },
    ),
    (
        "elixir",
        Profile {
            triple_quotes: true,
            keywords: keywords_extra::elixir,
            ..HASH
        },
    ),
    (
        "r",
        Profile {
            keywords: keywords_extra::r,
            ..HASH
        },
    ),
];

/// Unknown languages: the comment and string forms most languages share,
/// guarded so prose-like text does not turn into one long string or comment.
static GENERIC: Profile = Profile {
    hash: LineMarker::AfterSpace,
    dash: LineMarker::AfterSpace,
    keywords: keywords_extra::generic,
    ..TEXTUAL_C
};

const TEXTUAL_C: Profile = Profile {
    multiline_strings: false,
    apostrophe_text: true,
    ..C_LIKE
};

/// How a canonical language is lexed. `None` keeps the source plain: no
/// language, or one that explicitly asks for plain text.
pub(super) fn profile(language: &str) -> Option<&'static Profile> {
    if matches!(language, "" | "text" | "txt" | "plain" | "plaintext") {
        return None;
    }
    Some(
        PROFILES
            .iter()
            .find(|(name, _)| *name == language)
            .map_or(&GENERIC, |(_, profile)| profile),
    )
}

pub fn language_alias(language: &str) -> String {
    canonical_language(language).into_owned()
}

/// Borrow names that are already canonical (the common case for fences and
/// repeated highlight calls) instead of allocating twice per lookup.
pub(super) fn canonical_language(language: &str) -> Cow<'_, str> {
    let canonical = !language.is_empty()
        && !language
            .bytes()
            .any(|b| b.is_ascii_uppercase() || b.is_ascii_whitespace());
    if canonical && alias_target(language).is_none() {
        return Cow::Borrowed(language);
    }
    let lower = language.trim().to_ascii_lowercase();
    match alias_target(&lower) {
        Some(target) => Cow::Borrowed(target),
        None => Cow::Owned(lower),
    }
}

fn alias_target(language: &str) -> Option<&'static str> {
    Some(match language {
        "ts" | "mts" | "cts" => "typescript",
        "js" | "mjs" | "cjs" => "javascript",
        "py" | "pyi" => "python",
        "rs" => "rust",
        "sh" | "zsh" | "shell" | "shellscript" | "console" => "bash",
        "kt" | "kts" => "kotlin",
        "c++" | "cc" | "cxx" | "hpp" => "cpp",
        "cs" => "csharp",
        "yml" => "yaml",
        "md" => "markdown",
        "xml" | "svg" | "markup" | "htm" => "html",
        "jsonc" => "json",
        "patch" => "diff",
        "rb" | "gemfile" | "rakefile" => "ruby",
        "ex" | "exs" => "elixir",
        "gql" => "graphql",
        "mk" | "make" | "gnumakefile" => "makefile",
        "docker" | "containerfile" => "dockerfile",
        "cfg" | "conf" | "properties" | "editorconfig" | "gitconfig" => "ini",
        "sc" => "scala",
        _ => return None,
    })
}
