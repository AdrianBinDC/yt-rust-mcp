use crate::error::ToolError;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionTrack {
    pub language_code: String,
    pub kind: String,
    pub base_url: String,
}

pub fn select_track(player: &Value, lang: Option<&str>) -> Result<CaptionTrack, ToolError> {
    if let Some(status) = player
        .pointer("/playabilityStatus/status")
        .and_then(Value::as_str)
    {
        if status != "OK" {
            let reason = player
                .pointer("/playabilityStatus/reason")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            return Err(ToolError::Unplayable {
                status: status.to_string(),
                reason,
            });
        }
    }
    let tracks = player
        .pointer("/captions/playerCaptionsTracklistRenderer/captionTracks")
        .and_then(Value::as_array)
        .ok_or(ToolError::NoCaptionTrack)?;
    let parsed: Vec<CaptionTrack> = tracks.iter().filter_map(read_track).collect();
    if parsed.is_empty() {
        return Err(ToolError::NoCaptionTrack);
    }
    let matched: Vec<&CaptionTrack> = match lang {
        Some(code) => parsed
            .iter()
            .filter(|track| track.language_code.eq_ignore_ascii_case(code))
            .collect(),
        None => parsed.iter().collect(),
    };
    if matched.is_empty() {
        return Err(ToolError::NoCaptionLanguage(lang.unwrap_or("").to_string()));
    }
    let chosen = matched
        .iter()
        .find(|track| track.kind == "manual")
        .copied()
        .unwrap_or(matched[0]);
    Ok(chosen.clone())
}

fn read_track(item: &Value) -> Option<CaptionTrack> {
    let base_url = item.get("baseUrl")?.as_str()?.to_string();
    let language_code = item
        .get("languageCode")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let kind = if item.get("kind").and_then(Value::as_str) == Some("asr") {
        "asr"
    } else {
        "manual"
    };
    Some(CaptionTrack {
        language_code,
        kind: kind.to_string(),
        base_url,
    })
}

pub fn timedtext_url(base_url: &str) -> Result<String, ToolError> {
    if base_url.contains("exp=xpe") {
        return Err(ToolError::BrowserTokenRequired);
    }
    let (path, query) = base_url.split_once('?').unwrap_or((base_url, ""));
    let mut pairs: Vec<(String, String)> = query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (name, value) = part.split_once('=').unwrap_or((part, ""));
            (name.to_string(), value.to_string())
        })
        .collect();
    if let Some(fmt) = pairs.iter_mut().find(|(name, _)| name == "fmt") {
        fmt.1 = "json3".to_string();
    } else {
        pairs.push(("fmt".to_string(), "json3".to_string()));
    }
    let query = pairs
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    Ok(format!("{path}?{query}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ToolError;

    fn player_ok() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../fixtures/youtube/transcript.player.ok.json"
        ))
        .unwrap()
    }

    fn player_no_tracks() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../fixtures/youtube/transcript.player.no-tracks.json"
        ))
        .unwrap()
    }

    fn player_token() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../fixtures/youtube/transcript.player.token.json"
        ))
        .unwrap()
    }

    #[test]
    fn prefers_manual_track_and_replaces_existing_fmt() {
        let track = select_track(&player_ok(), None).unwrap();
        assert_eq!(track.language_code, "en");
        assert_eq!(track.kind, "manual");
        assert_eq!(
            timedtext_url(&track.base_url).unwrap(),
            "https://www.youtube.com/api/timedtext?v=abcdefghijk&lang=en&fmt=json3"
        );
    }

    #[test]
    fn requested_language_can_fall_back_to_auto_generated() {
        let track = select_track(&player_ok(), Some("es")).unwrap();
        assert_eq!(track.kind, "asr");
        assert_eq!(track.language_code, "es");
    }

    #[test]
    fn missing_language_names_the_request() {
        let err = select_track(&player_ok(), Some("fr")).unwrap_err();
        assert!(matches!(err, ToolError::NoCaptionLanguage(lang) if lang == "fr"));
    }

    #[test]
    fn missing_tracks_and_browser_token_are_errors() {
        assert!(matches!(
            select_track(&player_no_tracks(), None),
            Err(ToolError::NoCaptionTrack)
        ));
        let track = select_track(&player_token(), None).unwrap();
        assert!(matches!(
            timedtext_url(&track.base_url),
            Err(ToolError::BrowserTokenRequired)
        ));
    }

    #[test]
    fn unplayable_player_is_an_error() {
        let player = serde_json::json!({
            "playabilityStatus": { "status": "LOGIN_REQUIRED", "reason": "bot check" }
        });
        assert!(matches!(
            select_track(&player, None),
            Err(ToolError::Unplayable { status, reason })
                if status == "LOGIN_REQUIRED" && reason == "bot check"
        ));
    }
}
