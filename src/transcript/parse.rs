use crate::error::ToolError;
use crate::transcript::model::Cue;
use serde_json::Value;

pub fn parse_timedtext(body: &str) -> Result<Vec<Cue>, ToolError> {
    let parsed: Value = serde_json::from_str(body).map_err(|_| ToolError::EmptyTranscript)?;
    let events = parsed
        .get("events")
        .and_then(Value::as_array)
        .ok_or(ToolError::EmptyTranscript)?;
    let cues: Vec<Cue> = events.iter().filter_map(read_cue).collect();
    if cues.is_empty() {
        return Err(ToolError::EmptyTranscript);
    }
    Ok(cues)
}

fn read_cue(event: &Value) -> Option<Cue> {
    let text: String = event
        .get("segs")?
        .as_array()?
        .iter()
        .filter_map(|seg| seg.get("utf8").and_then(Value::as_str))
        .collect();
    let text = text.trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some(Cue {
        start_ms: event.get("tStartMs").and_then(Value::as_u64).unwrap_or(0),
        duration_ms: event
            .get("dDurationMs")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ToolError;

    #[test]
    fn parses_the_saved_timed_list_and_drops_blank_cues() {
        let cues =
            parse_timedtext(include_str!("../../fixtures/youtube/transcript.timed.json")).unwrap();
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start_ms, 1200);
        assert_eq!(cues[0].duration_ms, 3300);
        assert_eq!(cues[0].text, "hello world");
        assert_eq!(cues[1].text, "second line");
    }

    #[test]
    fn blank_and_unreadable_bodies_are_errors() {
        assert!(matches!(
            parse_timedtext(include_str!("../../fixtures/youtube/transcript.empty.json")),
            Err(ToolError::EmptyTranscript)
        ));
        assert!(matches!(
            parse_timedtext(""),
            Err(ToolError::EmptyTranscript)
        ));
        assert!(matches!(
            parse_timedtext("not-json"),
            Err(ToolError::EmptyTranscript)
        ));
    }
}
