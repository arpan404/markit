//! Keyword tables for the lexical-only languages file viewers commonly open.
//! They colour reserved words and literals; everything else follows the
//! shared scanner rules (comments, strings, numbers, calls, types).

pub(super) fn toml(word: &str) -> bool {
    TOML_WORDS.contains(&word)
}

#[rustfmt::skip]
const TOML_WORDS: &[&str] = &[
    "true", "false", "inf", "nan",
];

pub(super) fn ini(word: &str) -> bool {
    INI_WORDS.contains(&word)
}

#[rustfmt::skip]
const INI_WORDS: &[&str] = &[
    "true", "false", "yes", "no", "on", "off", "TRUE", "FALSE",
];

pub(super) fn ruby(word: &str) -> bool {
    RUBY_WORDS.contains(&word)
}

#[rustfmt::skip]
const RUBY_WORDS: &[&str] = &[
    "BEGIN", "END", "alias", "and", "begin", "break", "case", "class", "def", "defined", "do",
    "else", "elsif", "end", "ensure", "false", "for", "if", "in", "module", "next", "nil",
    "not", "or", "redo", "rescue", "retry", "return", "self", "super", "then", "true", "undef",
    "unless", "until", "when", "while", "yield", "require", "attr_reader", "attr_writer",
    "attr_accessor", "private", "protected", "public",
];

/// PHP keywords are case insensitive; source almost always writes them
/// in lower case, so only that spelling is matched.
pub(super) fn php(word: &str) -> bool {
    PHP_WORDS.contains(&word)
}

#[rustfmt::skip]
const PHP_WORDS: &[&str] = &[
    "abstract", "and", "array", "as", "break", "case", "catch", "class", "clone", "const",
    "continue", "declare", "default", "do", "echo", "else", "elseif", "enum", "extends",
    "false", "final", "finally", "fn", "for", "foreach", "function", "global", "if",
    "implements", "include", "include_once", "instanceof", "interface", "isset", "match",
    "namespace", "new", "null", "or", "print", "private", "protected", "public", "readonly",
    "require", "require_once", "return", "static", "switch", "throw", "trait", "true", "try",
    "unset", "use", "var", "while", "yield", "$this",
];

pub(super) fn lua(word: &str) -> bool {
    LUA_WORDS.contains(&word)
}

#[rustfmt::skip]
const LUA_WORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if",
    "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
    "self",
];

/// Dockerfile instructions are conventionally upper case; matching only that
/// spelling keeps shell words such as `add` or `user` in `RUN` lines plain.
pub(super) fn dockerfile(word: &str) -> bool {
    DOCKERFILE_WORDS.contains(&word)
}

#[rustfmt::skip]
const DOCKERFILE_WORDS: &[&str] = &[
    "FROM", "AS", "RUN", "CMD", "LABEL", "MAINTAINER", "EXPOSE", "ENV", "ADD", "COPY",
    "ENTRYPOINT", "VOLUME", "USER", "WORKDIR", "ARG", "ONBUILD", "STOPSIGNAL", "HEALTHCHECK",
    "SHELL",
];

pub(super) fn makefile(word: &str) -> bool {
    MAKEFILE_WORDS.contains(&word)
}

#[rustfmt::skip]
const MAKEFILE_WORDS: &[&str] = &[
    "ifeq", "ifneq", "ifdef", "ifndef", "else", "endif", "include", "sinclude", "define",
    "endef", "export", "unexport", "override", "private", "vpath",
];

pub(super) fn graphql(word: &str) -> bool {
    GRAPHQL_WORDS.contains(&word)
}

#[rustfmt::skip]
const GRAPHQL_WORDS: &[&str] = &[
    "query", "mutation", "subscription", "fragment", "on", "type", "interface", "union", "enum",
    "input", "scalar", "schema", "extend", "implements", "directive", "repeatable", "true",
    "false", "null",
];

pub(super) fn scala(word: &str) -> bool {
    SCALA_WORDS.contains(&word)
}

#[rustfmt::skip]
const SCALA_WORDS: &[&str] = &[
    "abstract", "case", "catch", "class", "def", "do", "else", "enum", "extends", "false",
    "final", "finally", "for", "given", "if", "implicit", "import", "lazy", "match", "new",
    "null", "object", "override", "package", "private", "protected", "return", "sealed",
    "super", "then", "this", "throw", "trait", "true", "try", "type", "using", "val", "var",
    "while", "with", "yield",
];

pub(super) fn dart(word: &str) -> bool {
    DART_WORDS.contains(&word)
}

#[rustfmt::skip]
const DART_WORDS: &[&str] = &[
    "abstract", "as", "assert", "async", "await", "break", "case", "catch", "class", "const",
    "continue", "default", "do", "dynamic", "else", "enum", "export", "extends", "extension",
    "external", "factory", "false", "final", "finally", "for", "get", "if", "implements",
    "import", "in", "is", "late", "library", "mixin", "new", "null", "on", "operator", "part",
    "required", "rethrow", "return", "sealed", "set", "static", "super", "switch", "this",
    "throw", "true", "try", "typedef", "var", "void", "while", "with", "yield",
];

pub(super) fn elixir(word: &str) -> bool {
    ELIXIR_WORDS.contains(&word)
}

#[rustfmt::skip]
const ELIXIR_WORDS: &[&str] = &[
    "after", "alias", "and", "case", "catch", "cond", "def", "defdelegate", "defexception",
    "defguard", "defimpl", "defmacro", "defmacrop", "defmodule", "defp", "defprotocol",
    "defstruct", "do", "else", "end", "false", "fn", "for", "if", "import", "in", "nil", "not",
    "or", "quote", "raise", "receive", "require", "rescue", "true", "try", "unless", "unquote",
    "use", "when", "with",
];

pub(super) fn r(word: &str) -> bool {
    R_WORDS.contains(&word)
}

#[rustfmt::skip]
const R_WORDS: &[&str] = &[
    "if", "else", "repeat", "while", "function", "for", "in", "next", "break", "return", "TRUE",
    "FALSE", "NULL", "NA", "Inf", "NaN", "library", "require",
];

/// Words most languages reserve, for sources whose language markit does not
/// know. Deliberately small: a wrong keyword colour is worse than none.
pub(super) fn generic(word: &str) -> bool {
    GENERIC_WORDS.contains(&word)
}

#[rustfmt::skip]
const GENERIC_WORDS: &[&str] = &[
    "if", "else", "elif", "for", "while", "do", "return", "function", "func", "fn", "def",
    "class", "struct", "enum", "import", "export", "from", "const", "let", "var", "true",
    "false", "null", "nil", "None", "True", "False", "end", "then", "public", "private",
    "static",
];
