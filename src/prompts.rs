use serde::Deserialize;
use std::path::Path;

use crate::config::ConfigError;

const DATA_API_TOOLS: &[&str] = &[
    "youtube_search",
    "youtube_videos_list",
    "youtube_channels_list",
    "youtube_playlists_list",
    "youtube_playlist_items_list",
];

#[derive(Debug, Deserialize)]
pub struct ServerPrompt {
    pub intro: String,
    pub rules: String,
    pub transcript: String,
}

/// Build server instructions from `prompts/server.yaml`.
/// When `enable_transcript` is false, the Tools line and body omit youtube_transcript.
pub fn build_instructions(prompt: &ServerPrompt, enable_transcript: bool) -> String {
    let mut tools: Vec<&str> = DATA_API_TOOLS.to_vec();
    if enable_transcript {
        tools.push("youtube_transcript");
    }
    let mut out = String::new();
    out.push_str(prompt.intro.trim_end());
    out.push_str("\n\n");
    out.push_str("Tools: ");
    out.push_str(&tools.join(", "));
    out.push_str(".\n\n");
    out.push_str(prompt.rules.trim_end());
    out.push('\n');
    if enable_transcript {
        out.push('\n');
        out.push_str(prompt.transcript.trim_end());
        out.push('\n');
    }
    out
}

pub fn load_server_prompt(prompts_dir: &Path) -> Result<ServerPrompt, ConfigError> {
    let path = prompts_dir.join("server.yaml");
    let text = std::fs::read_to_string(&path)
        .map_err(|source| ConfigError::Instructions { path, source })?;
    serde_yaml::from_str(&text).map_err(|source| ConfigError::PromptYaml {
        path: prompts_dir.join("server.yaml"),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ServerPrompt {
        ServerPrompt {
            intro: "Unofficial.\nRead-only.\n".into(),
            rules: "No uploads.\nQuota rules.\n".into(),
            transcript: "youtube_transcript is experimental.\n".into(),
        }
    }

    #[test]
    fn without_transcript_omits_tool_and_paragraph() {
        let text = build_instructions(&sample(), false);
        assert!(text.contains("Tools: youtube_search, youtube_videos_list, youtube_channels_list, youtube_playlists_list, youtube_playlist_items_list."));
        assert!(!text.contains("youtube_transcript"));
        assert!(!text.contains("experimental"));
    }

    #[test]
    fn with_transcript_lists_tool_and_paragraph() {
        let text = build_instructions(&sample(), true);
        assert!(text.contains("youtube_playlist_items_list, youtube_transcript."));
        assert!(text.contains("youtube_transcript is experimental."));
    }
}
