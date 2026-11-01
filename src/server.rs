use crate::call::execute;
use crate::config::Config;
use crate::error::ToolError;
use crate::store::Store;
use crate::youtube::{prepare, Method, ReqwestYoutube};
use jiff::tz::TimeZone;
use jiff::Zoned;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;

/// Turn tool args into the JSON `prepare` reads. Unset fields are omitted
/// (not null) because `prepare` treats any present key as set.
fn to_args(args: &impl Serialize) -> Result<Value, rmcp::ErrorData> {
    serde_json::to_value(args).map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))
}

#[derive(Clone)]
pub struct YoutubeServer {
    config: Config,
    store: std::sync::Arc<Mutex<Store>>,
    http: std::sync::Arc<ReqwestYoutube>,
}

impl YoutubeServer {
    pub fn new(config: Config, store: Store, http: ReqwestYoutube) -> Self {
        Self {
            config,
            store: std::sync::Arc::new(Mutex::new(store)),
            http: std::sync::Arc::new(http),
        }
    }

    /// Blocking: SQLite and reqwest::blocking. Call only from spawn_blocking.
    fn run(&self, method: Method, args: Value) -> Result<Value, ToolError> {
        let request = prepare(method, &args, self.config.max_pages_hard_cap)?;
        let tz =
            TimeZone::get("America/Los_Angeles").map_err(|e| ToolError::Store(e.to_string()))?;
        let now_zoned = Zoned::now().with_time_zone(tz);
        let today = now_zoned.date().to_string();
        let now = now_zoned.timestamp().as_second();
        let store = self
            .store
            .lock()
            .map_err(|e| ToolError::Store(e.to_string()))?;
        execute(
            &store,
            self.http.as_ref(),
            request,
            &self.config.ttl_seconds,
            self.config.search_budget,
            self.config.non_search_budget,
            now,
            &today,
            self.config.max_entries,
        )
    }

    /// Tool failures return Ok(error result) so the agent sees the message;
    /// only a panicked worker becomes a protocol error.
    async fn finish(&self, method: Method, args: Value) -> Result<CallToolResult, rmcp::ErrorData> {
        let this = self.clone();
        let outcome = tokio::task::spawn_blocking(move || this.run(method, args))
            .await
            .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
        match outcome {
            Ok(body) => Ok(CallToolResult::success(vec![ContentBlock::text(
                body.to_string(),
            )])),
            Err(err) => Ok(CallToolResult::error(vec![ContentBlock::text(
                err.to_string(),
            )])),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SearchArgs {
    /// Search words.
    #[serde(skip_serializing_if = "Option::is_none")]
    q: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<String>,
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    published_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    published_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    region_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relevance_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safe_search: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_pages: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bypass_cache: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct VideosArgs {
    /// Comma-separated video ids.
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_pages: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bypass_cache: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ChannelsArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    for_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    for_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_pages: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bypass_cache: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct PlaylistsArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_pages: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bypass_cache: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct PlaylistItemsArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    playlist_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_pages: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bypass_cache: Option<bool>,
}

#[tool_router]
impl YoutubeServer {
    #[tool(
        name = "youtube_search",
        description = "Find a public video, channel, or playlist by words. Do not use when you already have an id. Costs 1 search per live page."
    )]
    async fn youtube_search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Search, to_args(&args)?).await
    }

    #[tool(
        name = "youtube_videos_list",
        description = "Details for known video ids. Do not pass max_results, max_pages, or page_token. Costs 1 non_search per live call."
    )]
    async fn youtube_videos_list(
        &self,
        Parameters(args): Parameters<VideosArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Videos, to_args(&args)?).await
    }

    #[tool(
        name = "youtube_channels_list",
        description = "Channel metadata and the uploads playlist id. Pass id, for_handle, or for_username. Costs 1 non_search per live page."
    )]
    async fn youtube_channels_list(
        &self,
        Parameters(args): Parameters<ChannelsArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Channels, to_args(&args)?).await
    }

    #[tool(
        name = "youtube_playlists_list",
        description = "Playlist metadata. Pass id or channel_id. Costs 1 non_search per live page."
    )]
    async fn youtube_playlists_list(
        &self,
        Parameters(args): Parameters<PlaylistsArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Playlists, to_args(&args)?).await
    }

    #[tool(
        name = "youtube_playlist_items_list",
        description = "Videos inside a playlist. Pass playlist_id. Costs 1 non_search per live page."
    )]
    async fn youtube_playlist_items_list(
        &self,
        Parameters(args): Parameters<PlaylistItemsArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::PlaylistItems, to_args(&args)?).await
    }
}

#[tool_handler(name = "yt-rust-mcp", version = "0.1.0")]
impl ServerHandler for YoutubeServer {
    fn get_info(&self) -> ServerConfig {
        let mut config = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        let instructions = std::fs::read_to_string(self.config.prompts_dir.join("server.md"))
            .unwrap_or_else(|_| "Read-only YouTube Data API v3.".to_string());
        config.instructions = Some(instructions);
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_args_are_omitted_so_videos_paging_check_passes() {
        let args = VideosArgs {
            id: Some("abc".into()),
            part: None,
            hl: None,
            max_results: None,
            max_pages: None,
            page_token: None,
            bypass_cache: None,
        };
        let value = to_args(&args).unwrap();
        assert!(prepare(Method::Videos, &value, 5).is_ok());
    }

    #[test]
    fn prompts_name_the_rules() {
        let server = include_str!("../prompts/server.md");
        assert!(server.contains("Read-only"));
        assert!(server.contains("API key"));
        assert!(server.contains("youtube_search"));
        let search = include_str!("../prompts/tools/youtube_search.md");
        assert!(search.contains("When to use"));
        assert!(search.contains("When not to use"));
        assert!(search.contains("Quota"));
    }
}
