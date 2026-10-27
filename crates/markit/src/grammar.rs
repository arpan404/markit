use std::{
    cell::RefCell,
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
            let mut parser = ParseState::new(syntax);
            let mut light_state = HighlightState::new(&light, ScopeStack::new());
            let mut dark_state = HighlightState::new(&dark, ScopeStack::new());
            let mut tokens: Vec<CodeToken> = Vec::new();
            let mut failed = false;
            for line in LinesWithEndings::from(code) {
                let Ok(operations) = parser.parse_line(line, syntaxes) else {
                    failed = true;
                    break;
                };
                let lights = HighlightIterator::new(&mut light_state, &operations, line, &light);
                let darks: Vec<_> =
                    HighlightIterator::new(&mut dark_state, &operations, line, &dark).collect();
                let mut dark_index = 0;
                let mut dark_offset = 0;
                for (style, mut text) in lights {
                    while !text.is_empty() {
                        let Some((dark_style, dark_text)) = darks.get(dark_index) else {
                            failed = true;
                            break;
                        };
                        let take = text.len().min(dark_text.len() - dark_offset);
                        if take == 0 {
                            dark_index += 1;
                            dark_offset = 0;
                            continue;
                        }
                        let light_color = color(style.foreground);
                        let dark_color = color(dark_style.foreground);
                        let part = &text[..take];
                        if let Some(last) = tokens
                            .last_mut()
                            .filter(|last| last.light == light_color && last.dark == dark_color)
                        {
                            last.text.push_str(part);
                        } else {
                            tokens.push(CodeToken {
                                text: part.into(),
                                light: light_color,
                                dark: dark_color,
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
                if failed || tokens.len() > 8192 {
                    failed = true;
                    break;
                }
            }
            if failed {
                plain()
            } else {
                HighlightedCode {
                    language: language.clone(),
                    plain: false,
                    tokens,
                }
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
