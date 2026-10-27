use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FenceInfo {
    pub language: String,
    pub file: Option<String>,
    pub diff: bool,
}

pub fn fence_info(info: &str) -> FenceInfo {
    // Quotes group filename attributes; they do not introduce HTML or commands.
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    for c in info.chars() {
        if quote == Some(c) {
            quote = None;
        } else if quote.is_none() && (c == '\'' || c == '"') {
            quote = Some(c);
        } else if c.is_whitespace() && quote.is_none() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    let mut file = None;
    words.retain(|word| {
        if let Some((key, value)) = word.split_once('=') {
            if ["title", "file", "filename", "name"].contains(&key) {
                file = Some(value.to_owned());
                return false;
            }
        }
        true
    });
    let mut language = words.first().cloned().unwrap_or_default();
    if let Some((lang, path)) = language.split_once(':') {
        if !path.is_empty() && file.is_none() {
            file = Some(path.into());
            language = lang.into();
        }
    }
    let path_like = |word: &str| {
        word.contains('/')
            || word.contains('\\')
            || word.rsplit_once('.').is_some_and(|(stem, ext)| {
                !stem.is_empty() && !ext.is_empty() && ext.chars().all(char::is_alphanumeric)
            })
    };
    if file.is_none() {
        if path_like(&language) {
            file = Some(std::mem::take(&mut language));
        } else if let Some(path) = words.get(1).filter(|path| path_like(path)) {
            file = Some(path.clone());
        }
    }
    if language.is_empty() {
        language = file
            .as_deref()
            .and_then(|file| file.rsplit_once('.').map(|(_, ext)| ext))
            .unwrap_or("")
            .into();
    }
    language = crate::highlight::language_alias(&language);
    let diff = language == "diff"
        || file
            .as_deref()
            .is_some_and(|file| file.ends_with(".diff") || file.ends_with(".patch"));
    FenceInfo {
        language,
        file,
        diff,
    }
}
