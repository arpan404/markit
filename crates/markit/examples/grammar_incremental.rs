use std::{hint::black_box, time::Instant};
fn main() {
    let section = "export function greet(name: string): string {\n  // Preserve Unicode and nested syntax.\n  const value = `Hello ${name} 🦀`;\n  return value + \"!\";\n}\n\n";
    for bytes in [4096usize, 16384] {
        let source = section.repeat(bytes.div_ceil(section.len()));
        let mid = source
            .match_indices("  //")
            .nth(source.lines().count() / 14)
            .unwrap()
            .0;
        let mut edited = source.clone();
        edited.insert_str(mid, "  const inserted = 42;\n");
        for seed in [false, true] {
            let mut times = Vec::new();
            for _ in 0..31 {
                let (source, edited) = (source.clone(), edited.clone());
                times.push(
                    std::thread::spawn(move || {
                        if seed {
                            black_box(markit::highlight_grammar(&source, "ts"));
                        }
                        let start = Instant::now();
                        black_box(markit::highlight_grammar(&edited, "ts"));
                        start.elapsed().as_secs_f64() * 1000.
                    })
                    .join()
                    .unwrap(),
                );
            }
            let cold = times[0];
            times.sort_by(f64::total_cmp);
            println!(
                "bytes={} seeded={} median_ms={:.4} first_ms={:.4}",
                source.len(),
                seed,
                times[15],
                cold
            );
        }
    }
}
