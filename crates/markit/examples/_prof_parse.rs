use std::hint::black_box;
fn main() {
    let section = "# Markit 🦀\n\nA paragraph with **strong**, *emphasis*, `inline code` and a [link](https://example.com).\n\n- [x] Native output\n- [ ] Highlighting\n\n| Target | Bridge |\n| --- | --- |\n| iOS | C ABI |\n| Android | JNI |\n\n```rust\nfn main() { println!(\"Hello\"); }\n```\n\n";
    let n: usize = std::env::args().nth(1).unwrap().parse().unwrap();
    let source = section.repeat(n);
    let iters = 400_000 / n;
    let t = std::time::Instant::now();
    for _ in 0..iters { black_box(markit::parse(&source)); }
    let e = t.elapsed().as_secs_f64();
    println!("{n}: {:.2} ns/byte", e * 1e9 / (iters * source.len()) as f64);
}
