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
}
