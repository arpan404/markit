use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn measure(name: &str, source: &str, mut run: impl FnMut(&str)) {
    for _ in 0..10 {
        run(black_box(source));
    }
    let start = Instant::now();
    let mut iterations = 0u64;
    while start.elapsed() < Duration::from_millis(300) {
        run(black_box(source));
        iterations += 1;
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "{name:20} {:9.1} µs/document {:8.1} MiB/s ({iterations} iterations, {} bytes)",
        elapsed * 1e6 / iterations as f64,
        source.len() as f64 * iterations as f64 / elapsed / 1048576.0,
        source.len()
    );
}

fn main() {
    let section = "# Markit 🦀\n\nA paragraph with **strong**, *emphasis*, `inline code` and a [link](https://example.com).\n\n- [x] Native output\n- [ ] Highlighting\n\n| Target | Bridge |\n| --- | --- |\n| iOS | C ABI |\n| Android | JNI |\n\n```rust\nfn main() { println!(\"Hello\"); }\n```\n\n";
    for repeat in [1, 50, 400] {
        let source = section.repeat(repeat);
        println!("\nFixture: {repeat} sections");
        measure("native parse", &source, |s| {
            black_box(markit::parse(s));
        });
        measure("native parse + JSON", &source, |s| {
            black_box(markit::render_document_json(s));
        });
        measure("HTML", &source, |s| {
            black_box(markit::render_html(s));
        });
        let mut output = String::new();
        measure("HTML reused buffer", &source, |s| {
            markit::render_html_into(s, &mut output);
            black_box(&output);
        });
    }
    let prefix = format!("{}\n\nOpen tail", section.repeat(400));
    let mut elapsed = Duration::ZERO;
    let mut parsed = 0;
    for _ in 0..100 {
        let mut stream = markit::StreamRenderer::new_html();
        stream.append(&prefix, false);
        let start = Instant::now();
        let patch = stream.append(" continues", false);
        elapsed += start.elapsed();
        parsed = patch.parsed_bytes;
        black_box(patch);
    }
    println!(
        "\nStreaming append: {:.1} µs/update after {} bytes; {} source bytes reconsidered",
        elapsed.as_secs_f64() * 1e6 / 100.0,
        prefix.len(),
        parsed
    );
    math();
    #[cfg(feature = "grammars")]
    grammars();
}

/// Uncached math: every formula is new, so parse, layout, SVG and PNG all run.
fn math() {
    const FORMULAS: [&str; 6] = [
        "x^2 + y^2 = z^2",
        "\\frac{a+b}{c-d}",
        "\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}",
        "\\int_0^\\infty e^{-x^2}\\,dx",
        "\\sqrt{x} + \\alpha\\beta",
        "\\left( \\frac{1}{2} \\right)^{n}",
    ];
    println!("\nMath (6 new formulas per iteration)");
    let mut n = 0u64;
    measure("math SVG + PNG", "", |_| {
        n += 1;
        for tex in FORMULAS {
            black_box(markit::render_math(&format!("{tex}+{n}"), true));
        }
    });
    let document: String = FORMULAS
        .iter()
        .map(|tex| format!("$${tex}$$\n\n"))
        .collect();
    measure("math HTML, cached", &document, |s| {
        black_box(markit::render_html(s));
    });
}

/// Optional full grammars: a cold document, a mid-document edit and a
/// fence growing one line at a time.
#[cfg(feature = "grammars")]
fn grammars() {
    let source: String = (0..200)
        .map(|i| {
            format!("fn item_{i}(value: i64) -> i64 {{ /* line {i} */ value.wrapping_add({i}) }}\n")
        })
        .collect();
    println!("\nFull grammars (Rust, 200 lines)");
    let mut n = 0u64;
    measure("grammar cold", &source, |s| {
        n += 1;
        // Every line changes, so nothing is reused from the last document.
        black_box(markit::highlight_grammar(
            &s.replace("value", &format!("v{n}")),
            "rust",
        ));
    });
    measure("grammar mid edit", &source, |s| {
        n += 1;
        let at = s.len() / 2;
        let at = s[at..].find('\n').map_or(at, |i| at + i + 1);
        let edited = format!("{}// {n}\n{}", &s[..at], &s[at..]);
        black_box(markit::highlight_grammar(&edited, "rust"));
    });
    measure("grammar grow 200", &source, |s| {
        n += 1;
        let mut built = format!("// {n}\n");
        for line in s.split_inclusive('\n') {
            built.push_str(line);
            black_box(markit::highlight_grammar(&built, "rust"));
        }
    });
}
