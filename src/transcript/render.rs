use crate::transcript::model::{Cue, Format};

pub fn render(format: Format, cues: &[Cue]) -> String {
    match format {
        Format::Text => render_text(cues),
        Format::Srt => render_timed(cues, true, true),
        Format::Vtt => format!("WEBVTT\n\n{}", render_timed(cues, false, false)),
    }
}

fn render_text(cues: &[Cue]) -> String {
    let mut out = cues
        .iter()
        .map(|cue| cue.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn render_timed(cues: &[Cue], comma: bool, numbered: bool) -> String {
    let mut out = String::new();
    for (index, cue) in cues.iter().enumerate() {
        if numbered {
            out.push_str(&format!("{}\n", index + 1));
        }
        let end = cue.start_ms.saturating_add(cue.duration_ms);
        out.push_str(&timestamp(cue.start_ms, comma));
        out.push_str(" --> ");
        out.push_str(&timestamp(end, comma));
        out.push('\n');
        out.push_str(&cue.text);
        out.push_str("\n\n");
    }
    out
}

fn timestamp(ms: u64, comma: bool) -> String {
    let hours = ms / 3_600_000;
    let minutes = (ms % 3_600_000) / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    let sep = if comma { ',' } else { '.' };
    format!("{hours:02}:{minutes:02}:{seconds:02}{sep}{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::model::{Cue, Format};

    fn cues_from_saved_fixture() -> Vec<Cue> {
        let body: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/youtube/transcript.timed.json"))
                .unwrap();
        body["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|event| {
                let text: String = event["segs"]
                    .as_array()?
                    .iter()
                    .filter_map(|seg| seg["utf8"].as_str())
                    .collect();
                let text = text.trim().to_string();
                if text.is_empty() {
                    return None;
                }
                Some(Cue {
                    start_ms: event["tStartMs"].as_u64().unwrap(),
                    duration_ms: event["dDurationMs"].as_u64().unwrap_or(0),
                    text,
                })
            })
            .collect()
    }

    #[test]
    fn renders_the_saved_timed_list_into_each_format() {
        let cues = cues_from_saved_fixture();
        assert_eq!(
            render(Format::Text, &cues),
            include_str!("../../fixtures/youtube/transcript.timed.text")
        );
        assert_eq!(
            render(Format::Srt, &cues),
            include_str!("../../fixtures/youtube/transcript.timed.srt")
        );
        assert_eq!(
            render(Format::Vtt, &cues),
            include_str!("../../fixtures/youtube/transcript.timed.vtt")
        );
    }

    #[test]
    fn srt_keeps_a_newline_inside_one_cue() {
        let cues = vec![Cue {
            start_ms: 0,
            duration_ms: 1000,
            text: "line a\nline b".to_string(),
        }];
        let srt = render(Format::Srt, &cues);
        assert!(srt.contains("00:00:00,000 --> 00:00:01,000\nline a\nline b\n"));
    }
}
