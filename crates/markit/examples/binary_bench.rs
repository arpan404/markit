use std::{hint::black_box, time::Instant};
fn sample(mut f: impl FnMut(), iterations: usize) -> f64 {
    let at = Instant::now();
    for _ in 0..iterations {
        f();
    }
    at.elapsed().as_secs_f64() * 1000. / iterations as f64
}
fn main() {
    println!("round,source_bytes,json_bytes,binary_bytes,json_encode_ms,binary_encode_ms,json_render_ms,binary_render_ms");
    let unit = "# Example 🦀\n\nSome **bold** and [link](https://example.test).\n\n```ts\nconst message: string = 'hello'; // comment\nconsole.log(message, 42);\n```\n\n| a | b |\n| :--- | ---: |\n| x | y |\n\nInline $x^2$.\n\n";
    for target in [4096, 65536, 524288] {
        let source = unit.repeat(target / unit.len() + 1);
        let doc = markit::parse(&source);
        let json = serde_json::to_vec(&doc).unwrap();
        let binary = markit::encode_document_binary(&doc).unwrap();
        if target == 4096 {
            std::fs::write("/tmp/sol-bin-plan.bin", &binary).unwrap();
        }
        for round in 0..3 {
            let iterations = if target < 10000 {
                30
            } else if target < 100000 {
                4
            } else {
                1
            };
            let mut samples: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::new());
            for pass in 0..35 {
                // Rotate order to distribute thermal/load drift; discard warmup.
                for offset in 0..4 {
                    let mode = (pass + offset) % 4;
                    let ms = sample(
                        || match mode {
                            0 => {
                                black_box(serde_json::to_string(black_box(&doc)).unwrap());
                            }
                            1 => {
                                black_box(markit::encode_document_binary(black_box(&doc)).unwrap());
                            }
                            2 => {
                                black_box(markit::render_document_json(black_box(&source)));
                            }
                            _ => {
                                black_box(
                                    markit::render_document_binary(black_box(&source)).unwrap(),
                                );
                            }
                        },
                        iterations,
                    );
                    if pass >= 4 {
                        samples[mode].push(ms);
                    }
                }
            }
            let median: Vec<_> = samples
                .iter_mut()
                .map(|s| {
                    s.sort_by(f64::total_cmp);
                    s[s.len() / 2]
                })
                .collect();
            println!(
                "{round},{},{},{},{:.6},{:.6},{:.6},{:.6}",
                source.len(),
                json.len(),
                binary.len(),
                median[0],
                median[1],
                median[2],
                median[3]
            );
        }
    }
}
