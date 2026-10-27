//! Host-only preparation measurements. These do not measure mobile UI frames.
use serde_json::{json, Value};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn measure(name: &str, inputs: &[String], mut run: impl FnMut(&str)) -> Value {
    let mut index = 0;
    for source in inputs.iter().take(16) {
        run(black_box(source));
    }
    let mut samples = Vec::new();
    for _ in 0..9 {
        let start = Instant::now();
        let mut count = 0;
        while start.elapsed() < Duration::from_millis(40) {
            run(black_box(&inputs[index % inputs.len()]));
            index += 1;
            count += 1;
        }
        samples.push(start.elapsed().as_secs_f64() * 1000.0 / count as f64);
    }
    let mut sorted = samples.clone();
    sorted.sort_by(f64::total_cmp);
    json!({"name":name,"bytes":inputs[0].len(),"median_ms":sorted[4],"samples_ms":samples,"iterations":index})
}
fn main() {
    let code_section = "export function greet(name: string): string {\n  // Preserve Unicode and nested syntax.\n  const value = `Hello ${name} 🦀`;\n  return value + \"!\";\n}\n\n";
    let mut rows = Vec::new();
    for bytes in [4096usize, 16384] {
        let source = code_section.repeat(bytes.div_ceil(code_section.len()));
        // More sources than the 128-entry cache. Cycling cannot return a hit.
        let inputs: Vec<_> = (0..256)
            .map(|i| format!("{source}\n// native-benchmark-{i}"))
            .collect();
        rows.push(measure("lexical tokens, miss", &inputs, |s| {
            black_box(markit::highlight(s, "ts"));
        }));
        rows.push(measure("native lexical spans, miss", &inputs, |s| {
            black_box(markit::code_document(s, "ts"));
        }));
        rows.push(measure(
            "lexical tokens, hit",
            std::slice::from_ref(&source),
            |s| {
                black_box(markit::highlight(s, "ts"));
            },
        ));
        #[cfg(feature = "grammars")]
        rows.push(measure("full grammar tokens, miss", &inputs, |s| {
            black_box(markit::highlight_grammar(s, "ts"));
        }));
    }
    for (name, tex) in [
        ("simple", r"x^2+1"),
        (
            "sum_fraction",
            r"\sum_{i=1}^{n} i^2 = \frac{n(n+1)(2n+1)}{6}",
        ),
    ] {
        let formulas: Vec<_> = (0..256).map(|i| format!("{tex}+{i}")).collect();
        rows.push(measure(
            &format!("native PNG math {name}, miss"),
            &formulas,
            |s| {
                black_box(markit::render_math(s, true));
            },
        ));
        let html: Vec<_> = formulas.iter().map(|s| format!("$${s}$$")).collect();
        rows.push(measure(
            &format!("native document math {name}, miss"),
            &html,
            |s| {
                black_box(markit::parse(s));
            },
        ));
        rows.push(measure(&format!("HTML math {name}, miss"), &html, |s| {
            black_box(markit::render_html(s));
        }));
    }
    let result = json!({"revision":std::env::var("MARKIT_BENCH_REVISION").unwrap_or_default(),"target":std::env::consts::ARCH,"platform":std::env::consts::OS,"scope":"host Rust preparation only, excludes mobile runtime and UI", "results":rows});
    let text = serde_json::to_string_pretty(&result).unwrap();
    if let Some(path) = std::env::args().nth(1).filter(|arg| !arg.starts_with('-')) {
        std::fs::write(path, format!("{text}\n")).unwrap();
    }
    println!("{text}");
}
