use std::{hint::black_box, time::Instant};
#[path = "../tests/fixtures/stream_sequences.rs"]
mod fixture;
use fixture::sequences;

fn main() {
    if std::env::args().any(|a| a == "parity") {
        for parts in sequences() {
            for mode in 0..3 {
                let mut stream = match mode {
                    1 => markit::StreamRenderer::new_html(),
                    2 => markit::StreamRenderer::new_native(),
                    _ => markit::StreamRenderer::new(),
                };
                for (i, part) in parts.iter().enumerate() {
                    println!(
                        "{}",
                        serde_json::to_string(&stream.append(part, i + 1 == parts.len())).unwrap()
                    );
                }
            }
        }
        return;
    }
    for mode in 0..3 {
        let mut samples = Vec::new();
        for _ in 0..9 {
            let mut stream = match mode {
                1 => markit::StreamRenderer::new_html(),
                2 => markit::StreamRenderer::new_native(),
                _ => markit::StreamRenderer::new(),
            };
            stream.append(
                &"History paragraph with **bold** text.\n\n".repeat(2800),
                false,
            );
            stream.append(
                &format!(
                    "```rust\n{}",
                    format!("let value = calculate(42); // {}\n", "x".repeat(80)).repeat(400)
                ),
                false,
            );
            let line = "let value = calculate(42); // growing fence\n";
            let mut elapsed = 0u128;
            for _ in 0..180 {
                let start = Instant::now();
                black_box(stream.append(line, false));
                elapsed += start.elapsed().as_nanos();
            }
            samples.push(elapsed as f64 / 180.0 / 1000.0);
        }
        samples.sort_by(f64::total_cmp);
        println!("mode={mode} median_us={:.3}", samples[4]);
    }
}
