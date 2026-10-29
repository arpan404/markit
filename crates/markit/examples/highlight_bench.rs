//! Whole-file highlighting timings at file-viewer sizes.
//!
//! `cargo run --release -p markit --example highlight_bench [file.ts]`
//!
//! Builds ~2 KB, ~60 KB, ~490 KB and ~4 MB TypeScript inputs by repeating
//! the given file (or a built-in sample) and times each explicit highlight
//! output cold: every iteration prepends a distinct comment line, so neither
//! the result cache nor the append-only rescan can answer it. The `stream`
//! section appends a fence line by line in each stream mode, which is what
//! the Markdown fence budget protects.
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

const SAMPLE: &str = r#"import { createServer, type Server } from "node:http";
// Fake daemon: answers JSON-RPC requests with canned events.
export interface Session { id: string; title?: string; createdAt: number }
const sessions = new Map<string, Session>();
/** Create a session and return its identifier. */
export async function createSession(title: string): Promise<string> {
  const id = `s-${sessions.size + 1}`;
  sessions.set(id, { id, title, createdAt: Date.now() });
  return id;
}
export class Daemon {
  #server: Server | undefined;
  constructor(private readonly port = 4123) {}
  start(): void {
    this.#server = createServer((req, res) => {
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: true, sessions: [...sessions.values()] }));
    });
    this.#server.listen(this.port);
  }
}
"#;

fn sized(seed: &str, target: usize) -> String {
    let mut out = String::with_capacity(target + seed.len());
    while out.len() < target {
        out.push_str(seed);
    }
    let cut = out[..target.min(out.len())]
        .rfind('\n')
        .map_or(out.len(), |at| at + 1);
    out.truncate(cut);
    out
}

/// Median of cold runs, in milliseconds.
fn time(code: &str, mut run: impl FnMut(&str)) -> f64 {
    let budget = Duration::from_millis(if code.len() > 1 << 20 { 3000 } else { 600 });
    let started = Instant::now();
    let mut samples = Vec::new();
    let mut iteration = 0u64;
    while samples.len() < 5 || (started.elapsed() < budget && samples.len() < 200) {
        let source = format!("// run {iteration}\n{code}");
        iteration += 1;
        let start = Instant::now();
        run(black_box(&source));
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

type Append = fn(&mut markit::StreamRenderer, &str, bool);

fn stream(code: &str, make: fn() -> markit::StreamRenderer, append: Append) -> f64 {
    let mut renderer = make();
    let start = Instant::now();
    append(&mut renderer, "```ts\n", false);
    for line in code.split_inclusive('\n') {
        append(&mut renderer, line, false);
    }
    append(&mut renderer, "```\n", true);
    start.elapsed().as_secs_f64() * 1000.0
}

fn patch(renderer: &mut markit::StreamRenderer, delta: &str, finalized: bool) {
    black_box(renderer.append(delta, finalized));
}

fn native_patch(renderer: &mut markit::StreamRenderer, delta: &str, finalized: bool) {
    black_box(renderer.append_native_patch(delta, finalized).unwrap());
}

fn web_patch(renderer: &mut markit::StreamRenderer, delta: &str, finalized: bool) {
    black_box(markit::encode_web_patch(&renderer.append(delta, finalized)));
}

fn main() {
    let seed = std::env::args()
        .nth(1)
        .map(|path| std::fs::read_to_string(path).expect("read benchmark input"))
        .unwrap_or_else(|| SAMPLE.to_owned());
    println!(
        "{:>9} {:>9} {:>10} {:>10} {:>11} {:>11} {:>10}",
        "bytes", "runs", "packed ms", "tokens ms", "doc json ms", "doc bin ms", "MiB/s"
    );
    for target in [2 * 1024, 60 * 1024, 490 * 1024, 4 * 1024 * 1024] {
        let code = sized(&seed, target);
        let runs = (markit::highlight_packed(&code, "ts").len() - 13) / 3;
        let packed = time(&code, |s| {
            black_box(markit::highlight_packed(s, "ts"));
        });
        let tokens = time(&code, |s| {
            black_box(markit::highlight(s, "ts"));
        });
        let json = time(&code, |s| {
            black_box(markit::code_document_json(s, "ts"));
        });
        let binary = time(&code, |s| {
            black_box(markit::encode_document_binary(&markit::code_document(s, "ts")).unwrap());
        });
        println!(
            "{:>9} {:>9} {:>10.3} {:>10.3} {:>11.3} {:>11.3} {:>10.1}",
            code.len(),
            runs,
            packed,
            tokens,
            json,
            binary,
            code.len() as f64 / 1048576.0 / (packed / 1000.0)
        );
    }
    println!("\nstream a fence line by line (total ms)");
    println!(
        "{:>9} {:>9} {:>9} {:>12} {:>12} {:>9}",
        "bytes", "html", "json", "native json", "native mknp", "web mkwp"
    );
    for target in [60 * 1024, 490 * 1024] {
        let code = sized(&seed, target);
        println!(
            "{:>9} {:>9.1} {:>9.1} {:>12.1} {:>12.1} {:>9.1}",
            code.len(),
            stream(&code, markit::StreamRenderer::new_html, patch),
            stream(&code, markit::StreamRenderer::new, patch),
            stream(&code, markit::StreamRenderer::new_native, patch),
            stream(&code, markit::StreamRenderer::new_native, native_patch),
            stream(&code, markit::StreamRenderer::new_web, web_patch),
        );
    }
}
