use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reference {
    pub kind: String,
    pub target: String,
}

/// App references are actions passed to the host, never browser navigation.
pub fn reference_target(target: &str) -> Option<Reference> {
    if target.chars().any(char::is_control) {
        return None;
    }
    let kind = if target.starts_with("ace://thread/") || target.starts_with("thread://") {
        "thread"
    } else if target.starts_with("file://") {
        "file"
    } else if target.starts_with("https://github.com/") && target.contains("/pull/") {
        "pull_request"
    } else {
        return None;
    };
    Some(Reference {
        kind: kind.into(),
        target: target.into(),
    })
}

pub(crate) fn inline_file(text: &str) -> Option<Reference> {
    if text.is_empty()
        || text.contains("://")
        || text
            .chars()
            .any(|c| c.is_whitespace() || "<>\"'".contains(c))
    {
        return None;
    }
    let path = text.split(':').next()?;
    let (_, extension) = path.rsplit_once('.')?;
    if extension.is_empty() || extension.len() > 10 || !extension.chars().all(char::is_alphanumeric)
    {
        return None;
    }
    Some(Reference {
        kind: "file".into(),
        target: text.into(),
    })
}
