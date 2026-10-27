use std::time::{Duration, Instant};
use syntect::{
    highlighting::{HighlightIterator, HighlightState, Highlighter},
    parsing::{ParseState, ScopeStack},
    util::LinesWithEndings,
};
fn main() {
    let code = if let Some(path) = std::env::args().nth(1) {
        std::fs::read_to_string(path).expect("read profiling input")
    } else {
        "export function greet(name: string): string {\n  // Preserve Unicode and nested syntax.\n  const value = `Hello ${name} 🦀`;\n  return value + \"!\";\n}\n\n".repeat(28)
    };
    let syntaxes = two_face::syntax::extra_newlines();
    let syntax = syntaxes.find_syntax_by_token("typescript").unwrap();
    let themes = two_face::theme::extra();
    let light = Highlighter::new(themes.get(two_face::theme::EmbeddedThemeName::InspiredGithub));
    let dark = Highlighter::new(themes.get(two_face::theme::EmbeddedThemeName::Dracula));
    for run in 0..3 {
        let mut parser = ParseState::new(syntax);
        let mut l = HighlightState::new(&light, ScopeStack::new());
        let mut d = HighlightState::new(&dark, ScopeStack::new());
        let (mut parse, mut colors) = (Duration::ZERO, Duration::ZERO);
        let mut count = 0;
        for line in LinesWithEndings::from(code.as_str()) {
            let start = Instant::now();
            let ops = parser.parse_line(line, &syntaxes).unwrap();
            parse += start.elapsed();
            let start = Instant::now();
            count += HighlightIterator::new(&mut l, &ops, line, &light).count();
            count += HighlightIterator::new(&mut d, &ops, line, &dark).count();
            colors += start.elapsed();
        }
        println!(
            "run {run}: parse {:.3} ms, both themes {:.3} ms, pieces {count}",
            parse.as_secs_f64() * 1000.,
            colors.as_secs_f64() * 1000.
        );
    }
}
