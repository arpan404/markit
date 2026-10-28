//! Lists the STIX Two Math glyphs that the web core math font keeps.
//!
//! Lays out a corpus of common formulas with the full font and collects every
//! glyph the layouts place. Other glyphs keep exact metrics in the core font,
//! so layouts never change; drawing one loads the full font first. `scripts/math-font-core.py` turns the list into the core font. See
//! `docs/performance.md`, "Web download size".
//!
//! cargo run -p markit --example math_font_subset > crates/markit/assets/math-core-glyphs.txt
//! scripts/math-font-core.py <latex-rust>/fonts/stix-two-math/STIXTwoMath-Regular.otf \
//!     crates/markit/assets/math-core-glyphs.txt crates/markit/assets/math-core.otf
use latex_rust::{BoxContent, MathBox, MathFont, MathStyle};
use std::collections::BTreeSet;

fn collect(node: &MathBox, out: &mut BTreeSet<u16>) {
    match &node.content {
        BoxContent::Glyph { glyph_id, .. } => {
            out.insert(*glyph_id);
        }
        BoxContent::HList(children)
        | BoxContent::VList(children)
        | BoxContent::Overlap(children) => children.iter().for_each(|child| collect(child, out)),
        BoxContent::Color(_, inner)
        | BoxContent::BackColor(_, inner)
        | BoxContent::Frame { inner, .. } => collect(inner, out),
        _ => {}
    }
}

fn corpus() -> Vec<String> {
    let mut formulas = Vec::new();
    let mut atoms: Vec<String> = Vec::new();
    for ch in (0x21u8..0x7f).map(char::from) {
        atoms.push(match ch {
            '\\' => "\\backslash".into(),
            '{' | '}' | '#' | '$' | '%' | '&' | '_' => format!("\\{ch}"),
            '~' | '^' => continue,
            _ => ch.to_string(),
        });
    }
    for entry in latex_rust::symbols() {
        let latex = entry.latex;
        atoms.push(latex.replace("{}", "{x}"));
    }
    let greek = [
        "alpha",
        "beta",
        "gamma",
        "delta",
        "epsilon",
        "varepsilon",
        "zeta",
        "eta",
        "theta",
        "vartheta",
        "iota",
        "kappa",
        "lambda",
        "mu",
        "nu",
        "xi",
        "pi",
        "varpi",
        "rho",
        "varrho",
        "sigma",
        "varsigma",
        "tau",
        "upsilon",
        "phi",
        "varphi",
        "chi",
        "psi",
        "omega",
        "Gamma",
        "Delta",
        "Theta",
        "Lambda",
        "Xi",
        "Pi",
        "Sigma",
        "Upsilon",
        "Phi",
        "Psi",
        "Omega",
    ];
    atoms.extend(greek.iter().map(|g| format!("\\{g}")));
    // Every atom at text size; only the common ones in scripts.
    formulas.extend(atoms.iter().cloned());
    formulas.extend(atoms.iter().map(|atom| format!("{atom}_{{i=1}}^{{n}} x")));
    let mut scripted: Vec<String> = (0x21u8..0x7f)
        .map(char::from)
        .filter(char::is_ascii_alphanumeric)
        .map(String::from)
        .collect();
    scripted.extend(greek.iter().map(|g| format!("\\{g}")));
    scripted.extend(
        [
            "+",
            "-",
            "=",
            "<",
            ">",
            ",",
            ".",
            "'",
            "*",
            "/",
            "(",
            ")",
            "[",
            "]",
            "|",
            "\\{",
            "\\}",
            "!",
            ":",
            "\\prime",
            "\\infty",
            "\\pm",
            "\\mp",
            "\\cdot",
            "\\times",
            "\\ldots",
            "\\cdots",
            "\\to",
            "\\in",
            "\\leq",
            "\\geq",
            "\\le",
            "\\ge",
            "\\neq",
            "\\sim",
            "\\approx",
            "\\partial",
            "\\nabla",
            "\\top",
            "\\perp",
            "\\ast",
            "\\star",
            "\\dagger",
            "\\circ",
            "\\emptyset",
            "\\mathbb{R}",
            "\\mathbb{N}",
            "\\mathbb{Z}",
            "\\mathbb{C}",
            "\\ell",
            "\\hbar",
            "\\log",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    for atom in &scripted {
        formulas.push(format!("x^{{{atom}}}_{{{atom}}}"));
        formulas.push(format!("x^{{y^{{{atom}}}}}"));
    }
    let letters: String = ('A'..='Z').chain('a'..='z').chain('0'..='9').collect();
    let capitals_and_digits: String = ('A'..='Z').chain('0'..='9').collect();
    for style in [
        "mathbf",
        "mathit",
        "mathrm",
        "mathbb",
        "mathcal",
        "boldsymbol",
        "operatorname",
        "text",
        "textbf",
        "textit",
        "textrm",
        "mathnormal",
    ] {
        // \mathbb and \mathcal are capitals in practice (and \mathbb{1});
        // their rare lowercase forms load the full font.
        let letters = match style {
            "mathbb" => capitals_and_digits.as_str(),
            "mathcal" => &capitals_and_digits[..26],
            _ => letters.as_str(),
        };
        formulas.push(format!("\\{style}{{{letters}}}"));
        if matches!(style, "mathrm" | "text" | "operatorname") {
            formulas.push(format!("x_{{\\{style}{{{letters}}}}}"));
        }
    }
    for g in greek {
        formulas.push(format!("\\boldsymbol{{\\{g}}}"));
    }
    let delimiters = [
        "(",
        ")",
        "[",
        "]",
        "\\{",
        "\\}",
        "|",
        "\\|",
        "\\langle",
        "\\rangle",
        "\\lfloor",
        "\\rfloor",
        "\\lceil",
        "\\rceil",
        "/",
        "\\backslash",
        "\\uparrow",
        "\\downarrow",
        "\\updownarrow",
        "\\Uparrow",
        "\\Downarrow",
        "\\Updownarrow",
        "\\vert",
        "\\Vert",
        "\\lvert",
        "\\rvert",
        "\\lVert",
        "\\rVert",
        "\\lbrack",
        "\\rbrack",
        "\\lbrace",
        "\\rbrace",
        "\\lgroup",
        "\\rgroup",
        "\\lmoustache",
        "\\rmoustache",
    ];
    for rows in 1..=6 {
        let body = (0..rows)
            .map(|r| format!("a_{r}"))
            .collect::<Vec<_>>()
            .join(" \\\\ ");
        for d in delimiters {
            formulas.push(format!(
                "\\left{d} \\begin{{matrix}} {body} \\end{{matrix}} \\right{d}"
            ));
        }
        formulas.push(format!(
            "\\sqrt{{\\begin{{matrix}} {body} \\end{{matrix}}}}"
        ));
        formulas.push(format!(
            "\\sqrt[3]{{\\begin{{matrix}} {body} \\end{{matrix}}}}"
        ));
    }
    for d in delimiters {
        for size in [
            "big", "Big", "bigg", "Bigg", "bigl", "bigr", "Bigl", "Bigr", "biggl", "biggr",
            "Biggl", "Biggr",
        ] {
            formulas.push(format!("\\{size}{d}"));
        }
        formulas.push(format!(
            "\\left{d} \\frac{{a}}{{\\frac{{b}}{{c}}}} \\right."
        ));
    }
    for width in 1..=24 {
        let body = "x".repeat(width);
        for wide in [
            "widehat",
            "widetilde",
            "widecheck",
            "overline",
            "underline",
            "overbrace",
            "underbrace",
            "overrightarrow",
            "overleftarrow",
            "overleftrightarrow",
            "underrightarrow",
            "underleftarrow",
            "underleftrightarrow",
            "xrightarrow",
            "xleftarrow",
            "xRightarrow",
            "xLeftarrow",
            "xleftrightarrow",
            "xLeftrightarrow",
            "xmapsto",
            "xhookrightarrow",
            "xhookleftarrow",
            "overparen",
            "underparen",
            "overbracket",
            "underbracket",
            "hat",
            "tilde",
            "bar",
            "vec",
            "dot",
            "ddot",
            "check",
            "breve",
            "acute",
            "grave",
            "mathring",
            "dddot",
            "Vec",
            "sqrt",
            "boxed",
            "cancel",
            "bcancel",
            "xcancel",
        ] {
            formulas.push(format!("\\{wide}{{{body}}}"));
        }
        formulas.push(format!(
            "\\overbrace{{{body}}}^{{n}} \\underbrace{{{body}}}_{{n}}"
        ));
    }
    for env in [
        "matrix",
        "pmatrix",
        "bmatrix",
        "Bmatrix",
        "vmatrix",
        "Vmatrix",
        "smallmatrix",
        "cases",
        "dcases",
        "rcases",
        "aligned",
        "gathered",
        "split",
        "array",
    ] {
        let open = if env == "array" {
            "\\begin{array}{cc}".to_string()
        } else {
            format!("\\begin{{{env}}}")
        };
        formulas.push(format!("{open} a & b \\\\ c & d \\end{{{env}}}"));
    }
    for display in [
        "\\displaystyle",
        "\\textstyle",
        "\\scriptstyle",
        "\\scriptscriptstyle",
    ] {
        for op in [
            "sum",
            "prod",
            "coprod",
            "int",
            "iint",
            "iiint",
            "oint",
            "oiint",
            "bigcup",
            "bigcap",
            "bigoplus",
            "bigotimes",
            "bigodot",
            "biguplus",
            "bigsqcup",
            "bigvee",
            "bigwedge",
            "lim",
            "max",
            "min",
            "sup",
            "inf",
            "det",
            "log",
            "sin",
            "cos",
        ] {
            formulas.push(format!("{display} \\{op}_{{i=0}}^{{n}} \\frac{{\\partial f}}{{\\partial x}} \\dfrac{{1}}{{2}} \\tfrac{{1}}{{2}} \\binom{{n}}{{k}}"));
        }
    }
    formulas.extend(
        [
            "e^{i\\pi} + 1 = 0",
            "\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}",
            "f'(x) = \\lim_{h\\to 0} \\frac{f(x+h)-f(x)}{h}",
            "a \\pmod{n}",
            "a \\bmod b",
            "\\not= \\not\\in \\not\\subset",
            "x \\equiv y \\pmod{m}",
            "\\{x \\mid x > 0\\}",
            "\\nabla \\cdot \\vec{E} = \\frac{\\rho}{\\varepsilon_0}",
            "\\mathbb{E}[X] = \\sum_x x\\,p(x)",
            "\\|x\\|_2^2",
            "O(n \\log n)",
            "\\hbar \\ell \\wp \\Re \\Im \\aleph \\beth \\gimel \\daleth \\eth \\imath \\jmath",
            "x', x'', x''', x^\\prime",
            "\\cdots \\ldots \\vdots \\ddots \\dots",
            "\\stackrel{?}{=} \\overset{a}{b} \\underset{a}{b}",
            "\\substack{a \\\\ b}",
            "\\color{red}{x} \\colorbox{yellow}{y} \\fcolorbox{red}{blue}{z}",
            "\\phantom{x} \\hphantom{x} \\vphantom{x}",
            "\\quad \\qquad \\, \\: \\; \\! \\enspace \\hspace{1em}",
            "\\mathop{\\mathrm{arg\\,max}}_x f(x)",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    formulas
}

fn main() {
    let font = MathFont::stix_two_math().expect("STIX Two Math");
    let mut glyphs = BTreeSet::new();
    let mut laid_out = 0;
    let formulas = corpus();
    for tex in &formulas {
        let Ok(ast) = latex_rust::parse(tex) else {
            continue;
        };
        for style in [MathStyle::Display, MathStyle::Text] {
            if let Ok(tree) = latex_rust::layout(&ast, &font, style) {
                laid_out += 1;
                collect(&tree, &mut glyphs);
            }
        }
    }
    eprintln!(
        "{} formulas, {laid_out} layouts, {} glyphs",
        formulas.len(),
        glyphs.len()
    );
    for gid in glyphs {
        println!("{gid}");
    }
}
