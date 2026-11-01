use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("{counter} quota exhausted: {remaining} left, this page costs {cost}")]
    QuotaExhausted {
        counter: &'static str,
        remaining: i64,
        cost: i64,
    },
    #[error("refused auth-only parameter: {0}")]
    AuthOnlyParam(String),
    #[error("unknown parameter: {0}")]
    UnknownParam(String),
    #[error("youtube_videos_list does not accept {0}")]
    VideosListPaging(String),
    #[error("missing required selector: {0}")]
    MissingSelector(&'static str),
    #[error("max_pages {got} exceeds hard cap {cap}")]
    MaxPages { got: u32, cap: u32 },
    #[error("max_results {got} exceeds 50")]
    MaxResults { got: u32 },
    #[error("YouTube API error {status}: {reason}")]
    Google { status: u16, reason: String },
    #[error("YouTube HTTP {status}: unreadable error body")]
    GoogleUnreadable { status: u16 },
    #[error("cache: {0}")]
    Store(String),
    #[error("http: {0}")]
    Http(String),
    #[error("invalid video id")]
    InvalidVideoId,
    #[error("invalid format: {0}")]
    InvalidFormat(String),
    #[error("no caption track")]
    NoCaptionTrack,
    #[error("no caption track for language {0}")]
    NoCaptionLanguage(String),
    #[error("caption track requires a browser token")]
    BrowserTokenRequired,
    #[error("transcript empty")]
    EmptyTranscript,
    #[error("video unplayable: {status}: {reason}")]
    Unplayable { status: String, reason: String },
}

impl ToolError {
    /// Pull status and the first Google `reason`. The API key is not part of this body.
    pub fn from_google_body(status: u16, body: &str) -> Self {
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(_) => return Self::GoogleUnreadable { status },
        };
        let reason = parsed
            .pointer("/error/errors/0/reason")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        Self::Google { status, reason }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_quota_exceeded_reason() {
        let body = include_str!("../fixtures/youtube/error.quotaExceeded.json");
        let err = ToolError::from_google_body(403, body);
        let text = err.to_string();
        assert!(text.contains("403"));
        assert!(text.contains("quotaExceeded"));
        assert!(!text.contains("AIza"));
    }
}
