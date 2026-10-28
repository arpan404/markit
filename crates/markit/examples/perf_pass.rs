//! Highlighting, math and native-transport workloads for the third perf pass.
//! Uses only public APIs so the same file runs against older commits.
//! `cargo run --release -p markit --example perf_pass [--features grammars]`
use std::{hint::black_box, time::Instant};

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn time_ms(run: impl FnOnce()) -> f64 {
    let start = Instant::now();
    run();
    start.elapsed().as_secs_f64() * 1e3
}

fn rust_lines(lines: usize, salt: usize) -> String {
    (0..lines)
        .map(|i| {
            format!(
                "fn item_{i}_{salt}(value: i64) -> i64 {{ /* line {i} */ value.wrapping_add({i}) }}"
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tsx(lines: usize, salt: usize) -> String {
    (0..lines)
        .map(|i| match i % 4 {
            0 => format!("export function Row{i}_{salt}({{ value }}: Props) {{"),
            1 => format!("  const label = `row ${{value}} {i}`; // {salt}"),
            2 => format!("  return <div className=\"row\" data-i={{{i}}}>{{label}}</div>;"),
            _ => "}".to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

const FORMULAS: [(&str, bool); 6] = [
    ("x^2+y^2=z^2", false),
    (r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}", true),
    (r"\int_0^\infty e^{-x^2}\,dx=\frac{\sqrt{\pi}}{2}", true),
    (r"\alpha_{k+1} = \alpha_k - \eta \nabla f(\alpha_k)", false),
    (r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}", true),
    (r"\lim_{x \to 0} \frac{\sin x}{x} = 1", false),
];

fn math_message(salt: usize) -> String {
    let mut out = String::from("Some math follows.\n\n");
    for (i, (tex, display)) in FORMULAS.iter().enumerate() {
        if *display {
            out.push_str(&format!("Display {i}:\n\n$$ {tex} + {salt} $$\n\n"));
        } else {
            out.push_str(&format!("Inline {i} is ${tex} + {salt}$ in prose.\n\n"));
        }
    }
    out
}

fn chunks(source: &str, size: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < source.len() {
        let mut end = (at + size).min(source.len());
        while !source.is_char_boundary(end) {
            end += 1;
        }
        out.push(source[at..end].to_owned());
        at = end;
    }
    out
}

fn fence_lines(lines: usize) -> Vec<String> {
    let mut chunks = vec!["```rust\n".to_owned()];
    for line in rust_lines(lines, 0).lines() {
        chunks.push(format!("{line}\n"));
    }
    chunks
}

fn main() {
    // Math: unique formulas each sample, so every case is a cache miss.
    let mut salt = 1000;
    for (name, output) in [("svg_html", 0u8), ("png_native", 1), ("both_api", 2)] {
        let mut samples = Vec::new();
        for _ in 0..9 {
            salt += 1;
            let s = salt;
            samples.push(time_ms(|| {
                for (tex, display) in FORMULAS {
                    let tex = format!("{tex} + {s}");
                    match output {
                        0 => {
                            black_box(markit::render_html(&format!("${tex}$")));
                        }
                        1 => {
                            black_box(markit::parse(&format!("${tex}$")));
                        }
                        _ => {
                            black_box(markit::render_math(&tex, display));
                        }
                    }
                }
            }));
        }
        println!(
            "math6_miss {name} median_ms={:.3} per_formula_ms={:.4}",
            median(samples.clone()),
            median(samples) / 6.0
        );
    }
    let mut warm = Vec::new();
    for _ in 0..9 {
        warm.push(time_ms(|| {
            for (tex, display) in FORMULAS {
                black_box(markit::render_math(tex, display));
            }
        }));
    }
    println!("math6_hit both_api median_ms={:.4}", median(warm));

    for (name, mode) in [("html", 1u8), ("native", 2u8), ("both", 0u8)] {
        let mut samples = Vec::new();
        for _ in 0..7 {
            salt += 1;
            let parts = chunks(&math_message(salt), 7);
            samples.push(time_ms(|| {
                let mut stream = match mode {
                    1 => markit::StreamRenderer::new_html(),
                    2 => markit::StreamRenderer::new_native(),
                    _ => markit::StreamRenderer::new(),
                };
                for (i, part) in parts.iter().enumerate() {
                    black_box(stream.append(part, i + 1 == parts.len()));
                }
            }));
        }
        println!("math_stream mode={name} median_ms={:.3}", median(samples));
    }

    // Native transport: every patch encoded as the mobile bridges would.
    for lines in [400usize, 2000] {
        let parts = fence_lines(lines);
        let mut json = Vec::new();
        let mut binary = Vec::new();
        let mut tail = Vec::new();
        let mut last_json = 0;
        let mut last_binary = 0;
        let mut patch = Vec::new();
        let mut patch_tail = Vec::new();
        let mut open_patch = 0;
        for _ in 0..5 {
            json.push(time_ms(|| {
                let mut stream = markit::StreamRenderer::new_native();
                for (i, part) in parts.iter().enumerate() {
                    let patch = stream.append(part, i + 1 == parts.len());
                    last_json = serde_json::to_vec(&patch).unwrap().len();
                }
            }));
            let mut stream = markit::StreamRenderer::new_native();
            let mut tail_ms = 0.0;
            binary.push(time_ms(|| {
                for (i, part) in parts.iter().enumerate() {
                    let start = Instant::now();
                    let patch = stream.append(part, i + 1 == parts.len());
                    let bytes = markit::encode_native_patch(&patch).unwrap();
                    last_binary = bytes.len();
                    black_box(bytes);
                    if i + 50 >= parts.len() {
                        tail_ms += start.elapsed().as_secs_f64() * 1e3;
                    }
                }
            }));
            tail.push(tail_ms / 50.0);
            let mut stream = markit::StreamRenderer::new_native();
            let mut tail_ms = 0.0;
            patch.push(time_ms(|| {
                for (i, part) in parts.iter().enumerate() {
                    let start = Instant::now();
                    let bytes = stream
                        .append_native_patch(part, i + 1 == parts.len())
                        .unwrap();
                    if i + 2 == parts.len() {
                        open_patch = bytes.len();
                    }
                    black_box(bytes);
                    if i + 50 >= parts.len() {
                        tail_ms += start.elapsed().as_secs_f64() * 1e3;
                    }
                }
            }));
            patch_tail.push(tail_ms / 50.0);
        }
        println!(
            "native_fence_{lines} json median_ms={:.3} last_patch_bytes={last_json}",
            median(json)
        );
        println!(
            "native_fence_{lines} mknp median_ms={:.3} last_patch_bytes={last_binary} last50_per_append_ms={:.4}",
            median(binary),
            median(tail)
        );
        println!(
            "native_fence_{lines} mknp_v2 median_ms={:.3} open_patch_bytes={open_patch} last50_per_append_ms={:.4}",
            median(patch),
            median(patch_tail)
        );
    }

    // Lexical highlighting.
    for (name, lang, source) in [
        ("rust_200", "rust", rust_lines(200, 0)),
        ("tsx_400", "tsx", tsx(400, 0)),
    ] {
        let mut miss = Vec::new();
        let mut hit = Vec::new();
        let mut doc = Vec::new();
        for i in 0..9 {
            let unique = format!("{source}\n// {i} {salt}\n");
            miss.push(time_ms(|| {
                black_box(markit::highlight_packed(&unique, lang));
            }));
            hit.push(time_ms(|| {
                black_box(markit::highlight_packed(&unique, lang));
            }));
            let unique = format!("{source}\n// doc {i} {salt}\n");
            doc.push(time_ms(|| {
                black_box(markit::code_document(&unique, lang));
            }));
        }
        println!(
            "lexical {name} bytes={} packed_miss_ms={:.4} packed_hit_ms={:.5} code_document_miss_ms={:.4}",
            source.len(),
            median(miss),
            median(hit),
            median(doc)
        );
    }

    if markit::highlight_grammar("x", "rust").is_none() {
        println!("grammars feature off");
        return;
    }
    // Full grammars.
    let first = time_ms(|| {
        black_box(markit::highlight_grammar("const a = 1;", "typescript"));
    });
    println!("grammar first_use_ts ms={first:.3}");
    for (name, lang, lines) in [("rust", "rust", 200usize), ("tsx", "tsx", 200)] {
        let mut miss = Vec::new();
        let mut hit = Vec::new();
        for i in 0..7 {
            let source = if lang == "rust" {
                rust_lines(lines, i + salt)
            } else {
                tsx(lines, i + salt)
            };
            miss.push(time_ms(|| {
                black_box(markit::highlight_grammar(&source, lang));
            }));
            hit.push(time_ms(|| {
                black_box(markit::highlight_grammar(&source, lang));
            }));
        }
        let source = if lang == "rust" {
            rust_lines(lines, 0)
        } else {
            tsx(lines, 0)
        };
        println!(
            "grammar {name}_{lines} bytes={} miss_ms={:.3} hit_ms={:.4}",
            source.len(),
            median(miss),
            median(hit)
        );
        // An edit to one line in the middle of an already highlighted file.
        let mut edit = Vec::new();
        for i in 0..7 {
            let base = if lang == "rust" {
                rust_lines(lines, 77 + salt + i)
            } else {
                tsx(lines, 77 + salt + i)
            };
            black_box(markit::highlight_grammar(&base, lang));
            let at = base.len() / 2;
            let at = base[at..].find('\n').map_or(at, |n| at + n);
            let edited = format!("{} // edited\n{}", &base[..at], &base[at + 1..]);
            edit.push(time_ms(|| {
                black_box(markit::highlight_grammar(&edited, lang));
            }));
        }
        println!("grammar {name}_{lines} mid_edit_ms={:.3}", median(edit));
        // Streaming growth line by line.
        let mut built = String::new();
        let grow = time_ms(|| {
            for line in source.lines() {
                built.push_str(line);
                built.push('\n');
                black_box(markit::highlight_grammar(&built, lang));
            }
        });
        println!("grammar {name}_{lines} grow_by_line_total_ms={grow:.3}");
    }
}
