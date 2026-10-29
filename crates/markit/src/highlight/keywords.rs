//! Keyword tables of the languages markit has always lexed. Length dispatch
//! first: most identifiers miss on length alone without comparing bytes.

pub(super) fn python(word: &str) -> bool {
    match word.len() {
        2 => matches!(word, "as" | "if" | "in" | "is" | "or"),
        3 => matches!(word, "and" | "def" | "del" | "for" | "not" | "try"),
        4 => matches!(
            word,
            "None" | "True" | "elif" | "else" | "from" | "pass" | "self" | "with"
        ),
        5 => matches!(
            word,
            "False" | "async" | "await" | "break" | "class" | "raise" | "while" | "yield"
        ),
        6 => matches!(
            word,
            "assert" | "except" | "global" | "import" | "lambda" | "return"
        ),
        7 => matches!(word, "finally"),
        8 => matches!(word, "continue" | "nonlocal"),
        _ => false,
    }
}

pub(super) fn bash(word: &str) -> bool {
    match word.len() {
        2 => matches!(word, "cd" | "do" | "fi" | "if" | "in"),
        3 => matches!(word, "for"),
        4 => matches!(
            word,
            "case" | "done" | "echo" | "elif" | "else" | "esac" | "then"
        ),
        5 => matches!(word, "break" | "local" | "unset" | "until" | "while"),
        6 => matches!(word, "export" | "return" | "select" | "source"),
        7 => matches!(word, "declare"),
        8 => matches!(word, "continue" | "function" | "readonly"),
        _ => false,
    }
}

pub(super) fn sql(word: &str) -> bool {
    match word.len() {
        2 => {
            word.eq_ignore_ascii_case("as")
                || word.eq_ignore_ascii_case("by")
                || word.eq_ignore_ascii_case("on")
                || word.eq_ignore_ascii_case("or")
        }
        3 => {
            word.eq_ignore_ascii_case("all")
                || word.eq_ignore_ascii_case("and")
                || word.eq_ignore_ascii_case("end")
                || word.eq_ignore_ascii_case("key")
                || word.eq_ignore_ascii_case("not")
                || word.eq_ignore_ascii_case("set")
        }
        4 => {
            word.eq_ignore_ascii_case("case")
                || word.eq_ignore_ascii_case("drop")
                || word.eq_ignore_ascii_case("else")
                || word.eq_ignore_ascii_case("from")
                || word.eq_ignore_ascii_case("into")
                || word.eq_ignore_ascii_case("join")
                || word.eq_ignore_ascii_case("left")
                || word.eq_ignore_ascii_case("null")
                || word.eq_ignore_ascii_case("then")
                || word.eq_ignore_ascii_case("true")
                || word.eq_ignore_ascii_case("when")
        }
        5 => {
            word.eq_ignore_ascii_case("alter")
                || word.eq_ignore_ascii_case("false")
                || word.eq_ignore_ascii_case("group")
                || word.eq_ignore_ascii_case("index")
                || word.eq_ignore_ascii_case("inner")
                || word.eq_ignore_ascii_case("limit")
                || word.eq_ignore_ascii_case("order")
                || word.eq_ignore_ascii_case("outer")
                || word.eq_ignore_ascii_case("right")
                || word.eq_ignore_ascii_case("table")
                || word.eq_ignore_ascii_case("union")
                || word.eq_ignore_ascii_case("where")
        }
        6 => {
            word.eq_ignore_ascii_case("create")
                || word.eq_ignore_ascii_case("delete")
                || word.eq_ignore_ascii_case("having")
                || word.eq_ignore_ascii_case("insert")
                || word.eq_ignore_ascii_case("offset")
                || word.eq_ignore_ascii_case("select")
                || word.eq_ignore_ascii_case("update")
                || word.eq_ignore_ascii_case("values")
        }
        7 => word.eq_ignore_ascii_case("default") || word.eq_ignore_ascii_case("primary"),
        8 => word.eq_ignore_ascii_case("distinct"),
        10 => word.eq_ignore_ascii_case("references"),
        _ => false,
    }
}

pub(super) fn json(word: &str) -> bool {
    match word.len() {
        3 => matches!(word, "NaN"),
        4 => matches!(word, "null" | "true"),
        5 => matches!(word, "false"),
        8 => matches!(word, "Infinity"),
        _ => false,
    }
}

pub(super) fn yaml(word: &str) -> bool {
    match word.len() {
        2 => matches!(word, "no" | "on"),
        3 => matches!(word, "off" | "yes"),
        4 => matches!(word, "null" | "true"),
        5 => matches!(word, "false"),
        _ => false,
    }
}

pub(super) fn rust(word: &str) -> bool {
    match word.len() {
        2 => matches!(word, "as" | "do" | "fn" | "if" | "in"),
        3 => matches!(
            word,
            "box" | "dyn" | "for" | "let" | "mod" | "mut" | "pub" | "ref" | "try" | "use"
        ),
        4 => matches!(
            word,
            "Self" | "else" | "enum" | "impl" | "loop" | "move" | "priv" | "self" | "true" | "type"
        ),
        5 => matches!(
            word,
            "async"
                | "await"
                | "break"
                | "const"
                | "crate"
                | "false"
                | "final"
                | "macro"
                | "match"
                | "super"
                | "trait"
                | "union"
                | "where"
                | "while"
                | "yield"
        ),
        6 => matches!(
            word,
            "become" | "extern" | "return" | "static" | "struct" | "typeof" | "unsafe"
        ),
        7 => matches!(word, "unsized" | "virtual"),
        8 => matches!(word, "abstract" | "continue" | "override"),
        _ => false,
    }
}

pub(super) fn swift(word: &str) -> bool {
    match word.len() {
        2 => matches!(word, "as" | "do" | "if" | "in" | "is"),
        3 => matches!(
            word,
            "any" | "for" | "get" | "let" | "nil" | "set" | "try" | "var"
        ),
        4 => matches!(
            word,
            "Self"
                | "case"
                | "else"
                | "enum"
                | "func"
                | "init"
                | "lazy"
                | "open"
                | "self"
                | "some"
                | "true"
                | "weak"
        ),
        5 => matches!(
            word,
            "actor"
                | "async"
                | "await"
                | "break"
                | "catch"
                | "class"
                | "defer"
                | "false"
                | "final"
                | "guard"
                | "inout"
                | "super"
                | "throw"
                | "where"
                | "while"
        ),
        6 => matches!(
            word,
            "deinit"
                | "didSet"
                | "import"
                | "public"
                | "repeat"
                | "return"
                | "static"
                | "struct"
                | "switch"
                | "throws"
        ),
        7 => matches!(
            word,
            "default" | "dynamic" | "package" | "private" | "unowned" | "willSet"
        ),
        8 => matches!(
            word,
            "continue"
                | "indirect"
                | "internal"
                | "isolated"
                | "mutating"
                | "operator"
                | "optional"
                | "override"
                | "protocol"
                | "required"
                | "rethrows"
        ),
        9 => matches!(
            word,
            "borrowing" | "consuming" | "extension" | "subscript" | "typealias"
        ),
        11 => matches!(
            word,
            "convenience" | "fallthrough" | "fileprivate" | "nonisolated" | "nonmutating"
        ),
        14 => matches!(word, "associatedtype"),
        _ => false,
    }
}

pub(super) fn c_like(word: &str) -> bool {
    match word.len() {
        2 => matches!(word, "as" | "do" | "go" | "if" | "in" | "is" | "of"),
        3 => matches!(
            word,
            "for" | "int" | "let" | "map" | "new" | "out" | "ref" | "try" | "use" | "val" | "var"
        ),
        4 => matches!(
            word,
            "case"
                | "char"
                | "else"
                | "enum"
                | "from"
                | "func"
                | "long"
                | "null"
                | "self"
                | "this"
                | "true"
                | "type"
                | "void"
                | "when"
                | "with"
        ),
        5 => matches!(
            word,
            "async"
                | "await"
                | "break"
                | "catch"
                | "class"
                | "const"
                | "defer"
                | "false"
                | "final"
                | "float"
                | "match"
                | "short"
                | "super"
                | "throw"
                | "trait"
                | "using"
                | "where"
                | "while"
                | "yield"
        ),
        6 => matches!(
            word,
            "delete"
                | "double"
                | "export"
                | "import"
                | "native"
                | "object"
                | "public"
                | "record"
                | "return"
                | "sealed"
                | "static"
                | "string"
                | "struct"
                | "switch"
                | "throws"
                | "typeof"
        ),
        7 => matches!(
            word,
            "boolean" | "default" | "extends" | "finally" | "package" | "private" | "virtual"
        ),
        8 => matches!(
            word,
            "abstract"
                | "continue"
                | "debugger"
                | "function"
                | "internal"
                | "override"
                | "readonly"
                | "unsigned"
                | "volatile"
        ),
        9 => matches!(
            word,
            "interface" | "namespace" | "protected" | "satisfies" | "undefined"
        ),
        10 => matches!(word, "implements" | "instanceof"),
        12 => matches!(word, "synchronized"),
        _ => false,
    }
}
