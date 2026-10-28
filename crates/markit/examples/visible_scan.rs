use markit::{parse, SpanIndex};
use std::{hint::black_box, time::Instant};
fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}
fn main() {
    for lines in [5000, 50000] {
        let source: String = (0..lines)
            .map(|i| format!("```rust\nlet value_{i} = {i}; // 😀 comment\n```\n\n"))
            .collect();
        let doc = parse(&source);
        let mut builds = Vec::new();
        for _ in 0..21 {
            let now = Instant::now();
            black_box(SpanIndex::new(black_box(&doc)));
            builds.push(now.elapsed().as_secs_f64() * 1000.);
        }
        let index = SpanIndex::new(&doc);
        let end = doc.text.encode_utf16().count();
        let ranges: Vec<_> = (0..1000)
            .map(|i| {
                let start = i * (end / 1000);
                start..(start + 40).min(end)
            })
            .collect();
        let mut scan = Vec::new();
        let mut indexed = Vec::new();
        for _ in 0..21 {
            let now = Instant::now();
            for r in &ranges {
                black_box(
                    doc.spans
                        .iter()
                        .filter(|s| s.start < r.end && s.end > r.start)
                        .collect::<Vec<_>>(),
                );
            }
            scan.push(now.elapsed().as_secs_f64() * 1000.);
            let now = Instant::now();
            for r in &ranges {
                black_box(index.query(black_box(r.clone())));
            }
            indexed.push(now.elapsed().as_secs_f64() * 1000.);
        }
        println!(
            "lines={lines} spans={} queries=1000 build_ms={:.4} scan_ms={:.4} indexed_ms={:.4}",
            doc.spans.len(),
            median(builds),
            median(scan),
            median(indexed)
        );
        if lines == 5000 {
            std::fs::write(
                "measurements/native-document.json",
                serde_json::to_string(&doc).unwrap(),
            )
            .unwrap();
        }
    }
}
