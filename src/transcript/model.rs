use crate::error::ToolError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub start_ms: u64,
    pub duration_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Srt,
    Vtt,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Srt => "srt",
            Self::Vtt => "vtt",
        }
    }
}

pub fn parse_format(value: Option<&str>) -> Result<Format, ToolError> {
    match value.unwrap_or("text") {
        "text" => Ok(Format::Text),
        "srt" => Ok(Format::Srt),
        "vtt" => Ok(Format::Vtt),
        other => Err(ToolError::InvalidFormat(other.to_string())),
    }
}

/// Language is lowercased so `EN` and `en` share one cached row.
pub fn cache_key(video_id: &str, lang: Option<&str>) -> String {
    let lang = lang
        .map(|l| l.to_ascii_lowercase())
        .unwrap_or_else(|| "default".to_string());
    format!("youtube.transcript\n{video_id}\n{lang}")
}

pub fn validate_video_id(id: &str) -> Result<(), ToolError> {
    let ok = id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(ToolError::InvalidVideoId)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ToolError;

    #[test]
    fn default_format_is_text_and_unknown_format_is_an_error() {
        assert!(matches!(parse_format(None).unwrap(), Format::Text));
        assert!(matches!(parse_format(Some("srt")).unwrap(), Format::Srt));
        assert!(matches!(parse_format(Some("vtt")).unwrap(), Format::Vtt));
        assert!(matches!(
            parse_format(Some("sbv")),
            Err(ToolError::InvalidFormat(value)) if value == "sbv"
        ));
    }

    #[test]
    fn cache_key_ignores_format_and_uses_default_lang_sentinel() {
        assert_eq!(
            cache_key("abcdefghijk", None),
            "youtube.transcript\nabcdefghijk\ndefault"
        );
        assert_eq!(
            cache_key("abcdefghijk", Some("en")),
            "youtube.transcript\nabcdefghijk\nen"
        );
        assert_eq!(
            cache_key("abcdefghijk", Some("EN")),
            cache_key("abcdefghijk", Some("en"))
        );
    }

    #[test]
    fn video_id_must_be_eleven_url_safe_characters() {
        assert!(validate_video_id("abcdefghijk").is_ok());
        assert!(matches!(
            validate_video_id("short"),
            Err(ToolError::InvalidVideoId)
        ));
        assert!(matches!(
            validate_video_id("has space!!"),
            Err(ToolError::InvalidVideoId)
        ));
    }
}
