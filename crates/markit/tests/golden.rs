//! Output fingerprints for math and full grammars, recorded before the
//! math raster, lexer and grammar performance pass. `examples/hash_outputs.rs`
//! keeps the original lexical and Markdown fingerprint.

fn mix(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes.iter().chain(b"\n") {
        *hash = (*hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
}

const FORMULAS: &[(&str, bool)] = &[
    ("x^2", false),
    ("\\frac{a+b}{c-d}", true),
    ("\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}", true),
    (
        "\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}",
        true,
    ),
    ("\\sqrt[3]{x} + \\sqrt{y}", false),
    ("\\alpha\\beta\\gamma \\Delta \\Omega", false),
    ("\\left( \\frac{1}{2} \\right)^{n}", true),
    ("\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}", true),
    ("\\mathbb{R} \\to \\mathcal{L}", false),
    ("f'(x) = \\lim_{h \\to 0} \\frac{f(x+h)-f(x)}{h}", true),
    ("\\unknowncommand{x}", false),
    ("{", false),
];

#[test]
fn math_svg_png_and_streams_match_recorded_output() {
    let mut hash = 0xcbf29ce484222325u64;
    for &(tex, display) in FORMULAS {
        match markit::render_math(tex, display) {
            Some(visual) => {
                mix(&mut hash, visual.svg.as_bytes());
                mix(&mut hash, visual.png.as_bytes());
                mix(&mut hash, &visual.width.to_le_bytes());
                mix(&mut hash, &visual.height.to_le_bytes());
            }
            None => mix(&mut hash, b"none"),
        }
    }
    let mut markdown = String::from("Math streams.\n\n");
    for &(tex, display) in FORMULAS {
        if display {
            markdown.push_str(&format!("$$\n{tex}\n$$\n\n"));
        } else {
            markdown.push_str(&format!("Inline ${tex}$ text.\n\n"));
        }
    }
    mix(&mut hash, markit::render_html(&markdown).as_bytes());
    mix(
        &mut hash,
        markit::render_document_json(&markdown).as_bytes(),
    );
    mix(
        &mut hash,
        &markit::render_document_binary(&markdown).unwrap(),
    );
    for mode in 0..3 {
        let mut stream = match mode {
            0 => markit::StreamRenderer::new(),
            1 => markit::StreamRenderer::new_html(),
            _ => markit::StreamRenderer::new_native(),
        };
        let chunks: Vec<&[u8]> = markdown.as_bytes().chunks(11).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            let patch = stream.append(std::str::from_utf8(chunk).unwrap(), i + 1 == chunks.len());
            mix(&mut hash, &serde_json::to_vec(&patch).unwrap());
        }
    }
    assert_eq!(format!("{hash:016x}"), "1c14432ffdfbb735");
}

#[cfg(feature = "grammars")]
#[test]
fn full_grammars_match_recorded_output() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        language: String,
        source: String,
    }
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/grammar.json")).unwrap();
    let mut hash = 0xcbf29ce484222325u64;
    let extra = [
        (
            "go",
            "package main\n\nfunc main() {\n\tfmt.Println(\"hi 🦀\") // c\n}\n",
        ),
        ("java", "class A { /* x */ String s = \"é\"; }\n"),
        ("yaml", "a: 1\nb: [x, 'y']\n# c\n"),
        ("bash", "for i in $(seq 3); do echo \"$i\"; done\n"),
        ("css", "a:hover { color: #fff; }\n"),
        ("no-such-language", "plain text\n"),
    ];
    let mut cases: Vec<(String, String)> = fixtures
        .into_iter()
        .map(|fixture| (fixture.language, fixture.source))
        .collect();
    cases.extend(extra.iter().map(|(l, s)| (l.to_string(), s.to_string())));
    for (language, source) in &cases {
        match markit::highlight_grammar(source, language) {
            Some(code) => mix(&mut hash, serde_json::to_string(&*code).unwrap().as_bytes()),
            None => mix(&mut hash, b"none"),
        }
        match markit::code_document_grammar(source, language) {
            Some(document) => mix(
                &mut hash,
                serde_json::to_string(&document).unwrap().as_bytes(),
            ),
            None => mix(&mut hash, b"none"),
        }
        // Grow by lines, as a streamed fence does.
        let mut built = String::new();
        for line in source.split_inclusive('\n') {
            built.push_str(line);
            if let Some(code) = markit::highlight_grammar(&built, language) {
                mix(&mut hash, serde_json::to_string(&*code).unwrap().as_bytes());
            }
        }
    }
    assert_eq!(format!("{hash:016x}"), "86113db1668408bd");
}

/// Web patch bytes for math documents, pinned so `web/tests/math-font.spec.js`
/// can check that Wasm, which draws with the core math font and loads the full
/// font on demand, produces exactly what native code produces with the full font.
pub const WEB_MATH_DOCUMENTS: [&str; 2] = [
    "Core: $x^2$, $\\alpha_i \\to \\beta^{n+1}$, $\\mathbb{R}^n$ and\n\n$$\n\\sum_{i=1}^{n} \\frac{1}{i^2} = \\frac{\\pi^2}{6} \\quad \\left( \\begin{matrix} a & b \\\\ c & d \\end{matrix} \\right)\n$$\n\nDone.\n",
    "Rare: $\\mathfrak{g} \\oplus \\mathscr{H}$, $\\text{café ☃}$, $\\phantom{\\mathfrak{X}}x$ and\n\n$$\n\\left\\langle \\begin{matrix} 1\\\\2\\\\3\\\\4\\\\5\\\\6\\\\7\\\\8\\\\9 \\end{matrix} \\right\\rangle \\oint_C \\omega\n$$\n",
];

#[test]
fn web_math_patches_match_recorded_output() {
    let hashes: Vec<String> = WEB_MATH_DOCUMENTS
        .iter()
        .map(|document| {
            let mut stream = markit::StreamRenderer::new_web();
            let mut hash = 0xcbf29ce484222325u64;
            mix(
                &mut hash,
                &markit::encode_web_patch(&stream.append(document, true)),
            );
            format!("{hash:016x}")
        })
        .collect();
    assert_eq!(hashes, ["9f34057f1d5643fb", "b6d1d30230ed5bb2"]);
}
