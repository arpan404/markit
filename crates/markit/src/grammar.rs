use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, OnceLock},
};
use syntect::{
    highlighting::{Color, HighlightIterator, HighlightState, Highlighter, ThemeSet},
    parsing::{ParseState, ScopeStack, SyntaxSet},
    util::LinesWithEndings,
};

use crate::highlight::{language_alias, CodeToken, HighlightedCode};

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static THEMES: OnceLock<ThemeSet> = OnceLock::new();
thread_local! {
    static PREVIOUS: RefCell<Option<ParsedCode>> = const { RefCell::new(None) };
    static CACHE: RefCell<crate::cache::Cache<(String, String), Arc<HighlightedCode>>> =
        RefCell::new(crate::cache::Cache::new(4 * 1024 * 1024));
}

fn color(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

pub fn highlight(code: &str, language: &str) -> Arc<HighlightedCode> {
    let language = language_alias(language);
    let key = (language.clone(), code.to_owned());
    if let Some(cached) = CACHE.with(|cache| cache.borrow_mut().get(&key)) {
        return cached;
    }
    let plain = || HighlightedCode {
        language: language.clone(),
        plain: true,
        tokens: vec![CodeToken {
            text: code.to_owned(),
            light: "#24292f".into(),
            dark: "#c9d1d9".into(),
        }],
    };
    let result = if code.len() > 128 * 1024
        || code.lines().any(|line| line.len() > 2048)
        || language.is_empty()
    {
        plain()
    } else {
        let syntaxes = SYNTAXES.get_or_init(two_face::syntax::extra_newlines);
        let syntax = syntaxes
            .find_syntax_by_token(&language)
            .or_else(|| syntaxes.find_syntax_by_extension(&language))
            .or_else(|| {
                syntaxes.find_syntax_by_name(match language.as_str() {
                    "csharp" => "C#",
                    "jsx" | "json5" => "JavaScript",
                    "tsx" => "TypeScriptReact",
                    "bash" => "Bourne Again Shell (bash)",
                    _ => &language,
                })
            });
        if let Some(syntax) = syntax {
            let themes = THEMES.get_or_init(|| {
                let extras = two_face::theme::extra();
                let mut themes = ThemeSet::new();
                themes.themes.insert(
                    "light".into(),
                    extras
                        .get(two_face::theme::EmbeddedThemeName::InspiredGithub)
                        .clone(),
                );
                themes.themes.insert(
                    "dark".into(),
                    extras
                        .get(two_face::theme::EmbeddedThemeName::Dracula)
                        .clone(),
                );
                themes
            });
            let light = Highlighter::new(&themes.themes["light"]);
            let dark = Highlighter::new(&themes.themes["dark"]);
            let previous = PREVIOUS.with(|cache| cache.borrow_mut().take());
            match parse_code(code, &language, syntax, syntaxes, &light, &dark, previous) {
                Some((tokens, parsed)) => {
                    PREVIOUS.with(|cache| *cache.borrow_mut() = parsed);
                    HighlightedCode {
                        language: language.clone(),
                        plain: false,
                        tokens,
                    }
                }
                None => plain(),
            }
        } else {
            plain()
        }
    };
    let weight = key.0.len()
        + key.1.len()
        + result
            .tokens
            .iter()
            .map(|token| token.text.len() + 96)
            .sum::<usize>();
    let result = Arc::new(result);
    CACHE.with(|cache| cache.borrow_mut().insert(key, result.clone(), weight));
    result
}

// One recent document per thread, with at most 4096 line checkpoints. Exact-result
// caching remains independently byte-budgeted above. Lines share their snapshots
// across edits, so reusing a suffix does not clone parser or theme stacks.
struct ParsedCode {
    language: String,
    lines: Vec<Rc<ParsedLine>>,
}
#[derive(Clone, PartialEq, Eq)]
struct Boundary {
    parser: ParseState,
    light: HighlightState,
    dark: HighlightState,
}
struct ParsedLine {
    text: String,
    before: Boundary,
    after: Boundary,
    runs: Vec<Run>,
}
type Colors = Rc<(String, String)>;
struct Run {
    len: usize,
    colors: Colors,
}
type Palette = HashMap<(Color, Color), Colors>;

fn parse_line(
    line: &str,
    state: &mut Boundary,
    syntaxes: &SyntaxSet,
    light: &Highlighter<'_>,
    dark: &Highlighter<'_>,
    palette: &mut Palette,
) -> Option<Rc<ParsedLine>> {
    let before = state.clone();
    let operations = state.parser.parse_line(line, syntaxes).ok()?;
    let lights = HighlightIterator::new(&mut state.light, &operations, line, light);
    let darks: Vec<_> = HighlightIterator::new(&mut state.dark, &operations, line, dark).collect();
    let (mut dark_index, mut dark_offset) = (0, 0);
    let mut runs: Vec<Run> = Vec::new();
    for (style, mut text) in lights {
        while !text.is_empty() {
            let (dark_style, dark_text) = darks.get(dark_index)?;
            let take = text.len().min(dark_text.len() - dark_offset);
            if take == 0 {
                dark_index += 1;
                dark_offset = 0;
                continue;
            }
            let colors = palette
                .entry((style.foreground, dark_style.foreground))
                .or_insert_with(|| {
                    Rc::new((color(style.foreground), color(dark_style.foreground)))
                });
            if let Some(last) = runs.last_mut().filter(|last| last.colors == *colors) {
                last.len += take;
            } else {
                runs.push(Run {
                    len: take,
                    colors: colors.clone(),
                });
            }
            text = &text[take..];
            dark_offset += take;
            if dark_offset == dark_text.len() {
                dark_index += 1;
                dark_offset = 0;
            }
        }
    }
    Some(Rc::new(ParsedLine {
        text: line.into(),
        before,
        after: state.clone(),
        runs,
    }))
}

fn parse_code(
    code: &str,
    language: &str,
    syntax: &syntect::parsing::SyntaxReference,
    syntaxes: &SyntaxSet,
    light: &Highlighter<'_>,
    dark: &Highlighter<'_>,
    previous: Option<ParsedCode>,
) -> Option<(Vec<CodeToken>, Option<ParsedCode>)> {
    let lines: Vec<_> = LinesWithEndings::from(code).collect();
    let previous = previous.filter(|old| old.language == language);
    let old = previous
        .as_ref()
        .map(|old| old.lines.as_slice())
        .unwrap_or_default();
    let prefix = lines
        .iter()
        .zip(old)
        .take_while(|(line, old)| **line == old.text)
        .count();
    let suffix = lines[prefix..]
        .iter()
        .rev()
        .zip(old[prefix..].iter().rev())
        .take_while(|(line, old)| **line == old.text)
        .count();
    let mut state = if prefix > 0 {
        old[prefix - 1].after.clone()
    } else {
        Boundary {
            parser: ParseState::new(syntax),
            light: HighlightState::new(light, ScopeStack::new()),
            dark: HighlightState::new(dark, ScopeStack::new()),
        }
    };
    let mut parsed = old[..prefix].to_vec();
    let mut palette = Palette::new();
    for index in prefix..lines.len() {
        if index >= lines.len() - suffix {
            let old_index = old.len() - (lines.len() - index);
            if state == old[old_index].before {
                parsed.extend_from_slice(&old[old_index..]);
                break;
            }
        }
        parsed.push(parse_line(
            lines[index],
            &mut state,
            syntaxes,
            light,
            dark,
            &mut palette,
        )?);
    }
    let mut tokens: Vec<CodeToken> = Vec::new();
    for line in &parsed {
        let mut offset = 0;
        for run in &line.runs {
            let part = &line.text[offset..offset + run.len];
            if let Some(last) = tokens
                .last_mut()
                .filter(|last| last.light == run.colors.0 && last.dark == run.colors.1)
            {
                last.text.push_str(part);
            } else {
                tokens.push(CodeToken {
                    text: part.into(),
                    light: run.colors.0.clone(),
                    dark: run.colors.1.clone(),
                });
            }
            offset += run.len;
        }
        if tokens.len() > 8192 {
            return None;
        }
    }
    let cache = (parsed.len() <= 4096).then(|| ParsedCode {
        language: language.into(),
        lines: parsed,
    });
    Some((tokens, cache))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        language: String,
        source: String,
        json: String,
    }
    fn fixtures() -> Vec<Fixture> {
        serde_json::from_str(include_str!("../tests/fixtures/grammar.json")).unwrap()
    }
    fn clear() {
        CACHE.with(|cache| *cache.borrow_mut() = crate::cache::Cache::new(4 * 1024 * 1024));
        PREVIOUS.with(|cache| *cache.borrow_mut() = None);
    }
    fn json(source: &str, language: &str) -> String {
        serde_json::to_string(&*crate::highlight_grammar(source, language).unwrap()).unwrap()
    }
    #[test]
    fn grammar_json_matches_original_on_six_fixtures() {
        for fixture in fixtures() {
            clear();
            assert_eq!(
                json(&fixture.source, &fixture.language),
                fixture.json,
                "{}",
                fixture.language
            );
        }
    }
    #[test]
    fn edits_and_line_insertions_match_full_grammar_json() {
        for fixture in fixtures() {
            clear();
            let mut source = fixture.source;
            json(&source, &fixture.language);
            // Include syntax-state changes that propagate beyond the edited line,
            // then close them again. Insert/delete lines and change the final newline.
            for insertion in [
                "inserted 🦀 line\n",
                "/*\n",
                "*/\n",
                "\"\"\"\n",
                "\"\"\"\n",
                "\r\n",
            ] {
                let mid = source
                    .match_indices('\n')
                    .nth(source.lines().count() / 2)
                    .map(|(i, _)| i + 1)
                    .unwrap_or(0);
                source.insert_str(mid, insertion);
                let incremental = json(&source, &fixture.language);
                clear();
                assert_eq!(
                    incremental,
                    json(&source, &fixture.language),
                    "{}: {insertion:?}",
                    fixture.language
                );
            }
            for _ in 0..3 {
                let end = source.find('\n').map(|i| i + 1).unwrap_or(source.len());
                source.drain(..end);
                let incremental = json(&source, &fixture.language);
                clear();
                assert_eq!(incremental, json(&source, &fixture.language));
            }
            source.push_str("\ntrailing 🦀");
            let incremental = json(&source, &fixture.language);
            clear();
            assert_eq!(incremental, json(&source, &fixture.language));
        }
    }
}
