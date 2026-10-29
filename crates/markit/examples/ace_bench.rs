//! Ace-like workloads. Release medians, cache behavior called out per case.
//! `cargo run --release -p markit --example ace_bench`
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

struct CountingAlloc;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

struct Counters {
    allocs: usize,
    bytes: usize,
}

fn counters() -> Counters {
    Counters {
        allocs: ALLOCS.load(Ordering::Relaxed),
        bytes: BYTES.load(Ordering::Relaxed),
    }
}

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn time_ms(mut run: impl FnMut()) -> f64 {
    let start = Instant::now();
    run();
    start.elapsed().as_secs_f64() * 1e3
}

fn chat_markdown() -> String {
    let code = (0..40)
        .map(|i| format!("pub fn step_{i}(value: i64) -> i64 {{ value.wrapping_mul({i}) + 1 }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,4 @@\n fn render() {\n-    parse_all()\n+    parse_tail()\n }\n";
    format!(
        r#"Here is the streaming path for a long assistant reply.

The settled prefix stays put. Only the open tail is parsed again, which is what a chat client does while tokens arrive. Inline `code`, **strong**, and a [note](https://example.com/markit) sit in the same paragraph so the inline pass cannot be skipped.

```rust file=crates/markit/src/stream.rs
{code}
```

The matching diff is small but still highlighted:

```diff
{diff}
```

The sum is $\sum_{{i=1}}^{{n}} i = n(n+1)/2$ and the display form is

$$\frac{{n(n+1)}}{{2}}$$

- reuse row views when a message grows
- keep code colors on the lexical palette
- [x] preserve source bytes
- [ ] device frame times

| Path | Cost |
| --- | --- |
| parse | tail only |
| highlight | resumed scan |
| bridge | one buffer |
"#
    )
}

fn large_rust(lines: usize) -> String {
    (0..lines)
        .map(|i| {
            format!("fn item_{i}(value: i64) -> i64 {{ /* line {i} */ value.wrapping_add({i}) }}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn large_diff(lines: usize) -> String {
    (0..lines)
        .map(|i| match i % 5 {
            0 => format!("diff --git a/f{i}.rs b/f{i}.rs"),
            1 => format!("--- a/f{i}.rs"),
            2 => format!("+++ b/f{i}.rs"),
            3 => format!("-fn old_{i}() {{ todo!() }}"),
            _ => format!("+fn new_{i}() -> i64 {{ {i} }}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn prose(repeat: usize) -> String {
    let section = "A paragraph with **strong**, *emphasis*, `inline code` and a [link](https://example.com/path).\n\n- [x] Native output\n- [ ] Highlighting\n\n| Target | Bridge |\n| --- | --- |\n| iOS | C ABI |\n| Android | JNI |\n\n";
    section.repeat(repeat)
}

fn main() {
    let chat = chat_markdown();
    println!("fixture chat_bytes={}", chat.len());
    let mut chunks = Vec::new();
    let mut at = 0;
    let bytes = chat.as_bytes();
    while at < bytes.len() {
        let mut end = (at + 24).min(bytes.len());
        while end < bytes.len() && bytes[end].is_ascii_alphanumeric() {
            end += 1;
        }
        if end == at {
            end = (at + 1).min(bytes.len());
        }
        chunks.push(chat[at..end].to_owned());
        at = end;
    }
    println!("fixture chat_chunks={}", chunks.len());

    for (name, mode) in [("html", 1u8), ("native", 2u8), ("web", 3u8), ("both", 0u8)] {
        let mut samples = Vec::new();
        let mut alloc_samples = Vec::new();
        let mut byte_samples = Vec::new();
        for _ in 0..7 {
            let before = counters();
            let ms = time_ms(|| {
                let mut stream = match mode {
                    1 => markit::StreamRenderer::new_html(),
                    2 => markit::StreamRenderer::new_native(),
                    3 => markit::StreamRenderer::new_web(),
                    _ => markit::StreamRenderer::new(),
                };
                for (i, chunk) in chunks.iter().enumerate() {
                    black_box(stream.append(chunk, i + 1 == chunks.len()));
                }
            });
            let after = counters();
            samples.push(ms);
            alloc_samples.push((after.allocs - before.allocs) as f64);
            byte_samples.push((after.bytes - before.bytes) as f64);
        }
        println!(
            "stream_chat mode={name} median_ms={:.3} allocs={:.0} bytes={:.0}",
            median(samples),
            median(alloc_samples),
            median(byte_samples)
        );
    }

    let rust_lines = large_rust(400);
    let fence = format!("```rust\n{rust_lines}\n");
    let mut line_chunks: Vec<String> = vec!["```rust\n".into()];
    for line in rust_lines.lines() {
        line_chunks.push(format!("{line}\n"));
    }
    for (name, mode) in [("html", 1u8), ("native", 2u8), ("web", 3u8), ("both", 0u8)] {
        let mut samples = Vec::new();
        for _ in 0..5 {
            samples.push(time_ms(|| {
                let mut stream = match mode {
                    1 => markit::StreamRenderer::new_html(),
                    2 => markit::StreamRenderer::new_native(),
                    3 => markit::StreamRenderer::new_web(),
                    _ => markit::StreamRenderer::new(),
                };
                for (i, chunk) in line_chunks.iter().enumerate() {
                    black_box(stream.append(chunk, i + 1 == line_chunks.len()));
                }
            }));
        }
        println!(
            "stream_fence_400 mode={name} median_ms={:.3}",
            median(samples)
        );
    }
    let mut json_samples = Vec::new();
    let mut binary_samples = Vec::new();
    let mut json_bytes = 0usize;
    let mut binary_bytes = 0usize;
    let mut patch_samples = Vec::new();
    let mut patch_bytes = 0usize;
    for _ in 0..5 {
        patch_samples.push(time_ms(|| {
            let mut stream = markit::StreamRenderer::new_native();
            for (i, chunk) in line_chunks.iter().enumerate() {
                let encoded = stream
                    .append_native_patch(chunk, i + 1 == line_chunks.len())
                    .unwrap();
                if i + 2 == line_chunks.len() {
                    patch_bytes = encoded.len();
                }
                black_box(encoded);
            }
        }));
        json_samples.push(time_ms(|| {
            let mut stream = markit::StreamRenderer::new_native();
            for (i, chunk) in line_chunks.iter().enumerate() {
                let patch = stream.append(chunk, i + 1 == line_chunks.len());
                json_bytes = serde_json::to_vec(&patch).unwrap().len();
                black_box(json_bytes);
            }
        }));
        binary_samples.push(time_ms(|| {
            let mut stream = markit::StreamRenderer::new_native();
            for (i, chunk) in line_chunks.iter().enumerate() {
                let patch = stream.append(chunk, i + 1 == line_chunks.len());
                let encoded = markit::encode_native_patch(&patch).unwrap();
                binary_bytes = encoded.len();
                black_box(encoded);
            }
        }));
    }
    println!(
        "stream_fence_400 native_json median_ms={:.3} last_patch_bytes={json_bytes}",
        median(json_samples)
    );
    println!(
        "stream_fence_400 native_mknp median_ms={:.3} last_patch_bytes={binary_bytes}",
        median(binary_samples)
    );
    println!(
        "stream_fence_400 native_mknp_v2 median_ms={:.3} open_patch_bytes={patch_bytes}",
        median(patch_samples)
    );
    let _ = fence;

    let html_cases = [
        ("prose_4k", prose(16)),
        ("prose_32k", prose(128)),
        ("code_16k", format!("```rust\n{}\n```\n", large_rust(200))),
        ("diff_16k", format!("```diff\n{}\n```\n", large_diff(250))),
        ("chat", chat.clone()),
    ];
    for (name, source) in html_cases {
        let mut samples = Vec::new();
        let mut alloc_samples = Vec::new();
        for _ in 0..9 {
            let before = counters();
            samples.push(time_ms(|| {
                black_box(markit::render_html(black_box(&source)));
            }));
            let after = counters();
            alloc_samples.push((after.allocs - before.allocs) as f64);
        }
        println!(
            "html {name} bytes={} median_ms={:.3} allocs={:.0}",
            source.len(),
            median(samples),
            median(alloc_samples)
        );
    }

    let mixed = format!("{}\n\n{}", prose(40), chat);
    for (name, source) in [("mixed", mixed.as_str()), ("chat", chat.as_str())] {
        let mut parse_s = Vec::new();
        let mut json_s = Vec::new();
        let mut bin_s = Vec::new();
        for _ in 0..9 {
            parse_s.push(time_ms(|| {
                black_box(markit::parse(black_box(source)));
            }));
            json_s.push(time_ms(|| {
                black_box(markit::render_document_json(black_box(source)));
            }));
            bin_s.push(time_ms(|| {
                black_box(markit::render_document_binary(black_box(source)).unwrap());
            }));
        }
        println!(
            "document {name} bytes={} parse_ms={:.3} json_ms={:.3} binary_ms={:.3}",
            source.len(),
            median(parse_s),
            median(json_s),
            median(bin_s)
        );
    }

    let code = large_rust(200);
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    for i in 0..9 {
        let unique = format!("{code}\n// {i}\n");
        cold.push(time_ms(|| {
            black_box(markit::highlight_packed(black_box(&unique), "rust"));
        }));
        warm.push(time_ms(|| {
            black_box(markit::highlight_packed(black_box(&unique), "rust"));
        }));
    }
    println!(
        "highlight_packed_200lines cold_ms={:.3} warm_ms={:.3}",
        median(cold),
        median(warm)
    );

    let mut grow = Vec::new();
    let mut built = String::new();
    for line in large_rust(120).lines() {
        built.push_str(line);
        built.push('\n');
        let snapshot = built.clone();
        grow.push(time_ms(|| {
            black_box(markit::highlight_packed(black_box(&snapshot), "rust"));
        }));
    }
    grow.sort_by(f64::total_cmp);
    println!(
        "highlight_grow_120 median_line_ms={:.4} p95_ms={:.4} sum_ms={:.3}",
        grow[grow.len() / 2],
        grow[grow.len() * 95 / 100],
        grow.iter().sum::<f64>()
    );

    let messages: Vec<String> = (0..200)
        .map(|i| {
            format!("## Message {i}\n\nReply with **item {i}** and `value_{i}`.\n\n- [x] done\n\n")
        })
        .collect();
    let mut many = Vec::new();
    for _ in 0..5 {
        many.push(time_ms(|| {
            for message in &messages {
                black_box(markit::render_html(message));
                black_box(markit::parse(message));
            }
        }));
    }
    println!("many_messages_200 html+parse median_ms={:.3}", median(many));

    for (name, tex, display) in [
        ("simple", "x^2", false),
        ("sum", r"\sum_{i=1}^{n} i", true),
        ("frac", r"\frac{n(n+1)}{2}", true),
    ] {
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        for i in 0..5 {
            let unique = format!("{tex} + {i}");
            cold.push(time_ms(|| {
                black_box(markit::render_math(black_box(&unique), display));
            }));
            warm.push(time_ms(|| {
                black_box(markit::render_math(black_box(&unique), display));
            }));
        }
        println!(
            "math {name} cold_ms={:.3} warm_ms={:.3}",
            median(cold),
            median(warm)
        );
    }
}
