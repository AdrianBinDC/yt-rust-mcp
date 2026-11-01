use crate::call::execute;
use crate::config::Config;
use crate::error::ToolError;
use crate::store::Store;
use crate::transcript::execute::{execute as transcript_execute, ReqwestTranscript};
use crate::youtube::{prepare, Method, ReqwestYoutube};
use jiff::tz::TimeZone;
use jiff::Zoned;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::Mutex;

/// Turn tool args into the JSON `prepare` reads. Unset typed fields are omitted
/// (not null) because `prepare` treats any present key as set.
fn to_args(args: &impl Serialize) -> Result<Value, rmcp::ErrorData> {
    serde_json::to_value(args).map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))
}

#[derive(Clone)]
pub struct YoutubeServer {
    config: Config,
    store: std::sync::Arc<Mutex<Store>>,
    http: std::sync::Arc<ReqwestYoutube>,
    transcript_http: Option<std::sync::Arc<ReqwestTranscript>>,
    instructions: String,
    tool_router: ToolRouter<Self>,
}

impl YoutubeServer {
    pub fn new(
        config: Config,
        store: Store,
        http: ReqwestYoutube,
        transcript_http: Option<ReqwestTranscript>,
        instructions: String,
    ) -> Self {
        let mut tool_router = Self::data_api_router();
        if config.enable_transcript {
            tool_router += Self::transcript_router();
        }
        Self {
            config,
            store: std::sync::Arc::new(Mutex::new(store)),
            http: std::sync::Arc::new(http),
            transcript_http: transcript_http.map(std::sync::Arc::new),
            instructions,
            tool_router,
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

    /// Blocking, like `run`: call only from spawn_blocking.
    fn run_transcript(&self, args: TranscriptArgs) -> Result<Value, ToolError> {
        let (video_id, lang, format, bypass_cache) = prepare_transcript(&args)?;
        let tz =
            TimeZone::get("America/Los_Angeles").map_err(|e| ToolError::Store(e.to_string()))?;
        let now_zoned = Zoned::now().with_time_zone(tz);
        let today = now_zoned.date().to_string();
        let now = now_zoned.timestamp().as_second();
        let store = self
            .store
            .lock()
            .map_err(|e| ToolError::Store(e.to_string()))?;
        let http = self.transcript_http.as_ref().ok_or_else(|| {
            ToolError::Store("transcript tool is disabled (ENABLE_MCP_TRANSCRIPT)".into())
        })?;
        transcript_execute(
            &store,
            http.as_ref(),
            &video_id,
            lang.as_deref(),
            format.as_deref(),
            bypass_cache,
            self.config.ttl_seconds.transcript,
            self.config.transcript_budget,
            now,
            &today,
            self.config.max_entries,
        )
    }

    async fn finish_transcript(
        &self,
        args: TranscriptArgs,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let this = self.clone();
        let outcome = tokio::task::spawn_blocking(move || this.run_transcript(args))
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
    /// Every key not declared as a field here (auth-only, camelCase, typos, videos paging).
    /// Hidden from the schema but kept, null included, so `prepare` can refuse it.
    #[serde(flatten)]
    #[schemars(skip)]
    extra: Map<String, Value>,
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
    /// Every key not declared as a field here (auth-only, camelCase, typos, videos paging).
    /// Hidden from the schema but kept, null included, so `prepare` can refuse it.
    #[serde(flatten)]
    #[schemars(skip)]
    extra: Map<String, Value>,
    /// Comma-separated video ids.
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bypass_cache: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ChannelsArgs {
    /// Every key not declared as a field here (auth-only, camelCase, typos, videos paging).
    /// Hidden from the schema but kept, null included, so `prepare` can refuse it.
    #[serde(flatten)]
    #[schemars(skip)]
    extra: Map<String, Value>,
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
    /// Every key not declared as a field here (auth-only, camelCase, typos, videos paging).
    /// Hidden from the schema but kept, null included, so `prepare` can refuse it.
    #[serde(flatten)]
    #[schemars(skip)]
    extra: Map<String, Value>,
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
    /// Every key not declared as a field here (auth-only, camelCase, typos, videos paging).
    /// Hidden from the schema but kept, null included, so `prepare` can refuse it.
    #[serde(flatten)]
    #[schemars(skip)]
    extra: Map<String, Value>,
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct TranscriptArgs {
    /// Any key not declared here. Hidden from the schema, kept so it can be refused.
    #[serde(flatten)]
    #[schemars(skip)]
    extra: Map<String, Value>,
    /// Public video id (11 characters).
    #[serde(skip_serializing_if = "Option::is_none")]
    video_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lang: Option<String>,
    /// text (default), srt, or vtt.
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bypass_cache: Option<bool>,
}

/// Returns (video_id, lang, format, bypass_cache). Format and id shape are
/// validated later by `execute`.
fn prepare_transcript(
    args: &TranscriptArgs,
) -> Result<(String, Option<String>, Option<String>, bool), ToolError> {
    if let Some(name) = args.extra.keys().next() {
        return Err(ToolError::UnknownParam(name.clone()));
    }
    let video_id = args
        .video_id
        .clone()
        .filter(|id| !id.is_empty())
        .ok_or(ToolError::MissingSelector("video_id"))?;
    Ok((
        video_id,
        args.lang.clone(),
        args.format.clone(),
        args.bypass_cache.unwrap_or(false),
    ))
}

#[tool_router(router = data_api_router)]
impl YoutubeServer {
    #[tool(
        name = "youtube_search",
        description = "Use to find public videos, channels, or playlists by words. Do not use when you already have an id. Quota: 1 search per live page."
    )]
    async fn youtube_search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Search, to_args(&args)?).await
    }

    #[tool(
        name = "youtube_videos_list",
        description = "Use for details about known video ids. Do not use for discovery or pass paging args. Quota: 1 non_search per live call."
    )]
    async fn youtube_videos_list(
        &self,
        Parameters(args): Parameters<VideosArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Videos, to_args(&args)?).await
    }

    #[tool(
        name = "youtube_channels_list",
        description = "Use for channel metadata and uploads playlist ids via id, handle, or username. Do not use to search by topic. Quota: 1 non_search per live page."
    )]
    async fn youtube_channels_list(
        &self,
        Parameters(args): Parameters<ChannelsArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Channels, to_args(&args)?).await
    }

    #[tool(
        name = "youtube_playlists_list",
        description = "Use for playlist metadata by playlist or channel id. Do not use to list a playlist's videos. Quota: 1 non_search per live page."
    )]
    async fn youtube_playlists_list(
        &self,
        Parameters(args): Parameters<PlaylistsArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Playlists, to_args(&args)?).await
    }

    #[tool(
        name = "youtube_playlist_items_list",
        description = "Use to list videos inside a known playlist. Do not use for playlist metadata or discovery. Quota: 1 non_search per live page."
    )]
    async fn youtube_playlist_items_list(
        &self,
        Parameters(args): Parameters<PlaylistItemsArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::PlaylistItems, to_args(&args)?).await
    }
}

#[tool_router(router = transcript_router)]
impl YoutubeServer {
    #[tool(
        name = "youtube_transcript",
        description = "Experimental: public Show transcript text or an SRT/VTT caption file. Unofficial and undocumented — not a Data API method; can break. Do not use for discovery or captions.download. Quota: 1 local transcript fetch; a cached repeat costs 0."
    )]
    async fn youtube_transcript(
        &self,
        Parameters(args): Parameters<TranscriptArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish_transcript(args).await
    }
}

#[tool_handler(name = "yt-rust-mcp", version = "0.1.0", router = self.tool_router)]
impl ServerHandler for YoutubeServer {
    fn get_info(&self) -> ServerConfig {
        let mut config = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        config.instructions = Some(self.instructions.clone());
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_only_search_arg_survives_tool_json_path_and_is_refused() {
        let args: SearchArgs = serde_json::from_value(serde_json::json!({
            "q": "rust",
            "forMine": true
        }))
        .unwrap();
        let value = to_args(&args).unwrap();
        assert_eq!(value["forMine"], true);
        assert!(matches!(
            prepare(Method::Search, &value, 5),
            Err(ToolError::AuthOnlyParam(name)) if name == "forMine"
        ));
    }

    #[test]
    fn null_auth_only_search_arg_survives_tool_json_path_and_is_refused() {
        let args: SearchArgs = serde_json::from_value(serde_json::json!({
            "q": "rust",
            "forMine": Value::Null
        }))
        .unwrap();
        let value = to_args(&args).unwrap();
        assert!(value.get("forMine").is_some());
        assert_eq!(value["forMine"], Value::Null);
        assert!(matches!(
            prepare(Method::Search, &value, 5),
            Err(ToolError::AuthOnlyParam(name)) if name == "forMine"
        ));
    }

    #[test]
    fn unset_args_are_omitted_so_videos_paging_check_passes() {
        let args: VideosArgs = serde_json::from_value(serde_json::json!({"id": "abc"})).unwrap();
        let value = to_args(&args).unwrap();
        assert!(prepare(Method::Videos, &value, 5).is_ok());
    }

    #[test]
    fn videos_paging_args_survive_tool_json_path_and_are_refused() {
        for name in [
            "max_results",
            "maxResults",
            "max_pages",
            "page_token",
            "pageToken",
        ] {
            for v in [serde_json::json!(5), Value::Null] {
                let args: VideosArgs =
                    serde_json::from_value(serde_json::json!({"id": "abc", name: v})).unwrap();
                let value = to_args(&args).unwrap();
                assert!(matches!(
                    prepare(Method::Videos, &value, 5),
                    Err(ToolError::VideosListPaging(ref n)) if n == name
                ));
            }
        }
    }

    #[test]
    fn unknown_arg_survives_tool_json_path_and_is_refused() {
        let args: ChannelsArgs = serde_json::from_value(serde_json::json!({
            "id": "UC1",
            "forHandle": "@x"
        }))
        .unwrap();
        let value = to_args(&args).unwrap();
        assert!(matches!(
            prepare(Method::Channels, &value, 5),
            Err(ToolError::UnknownParam(ref n)) if n == "forHandle"
        ));
    }

    #[test]
    fn hidden_args_are_not_in_json_schema() {
        let schema = |s: schemars::Schema| serde_json::to_string(&s).unwrap();
        let videos = schema(schemars::schema_for!(VideosArgs));
        let search = schema(schemars::schema_for!(SearchArgs));
        for hidden in ["forMine", "mine", "maxResults", "extra"] {
            assert!(!videos.contains(hidden), "videos schema leaks {hidden}");
            assert!(!search.contains(hidden), "search schema leaks {hidden}");
        }
        for paging in ["max_results", "max_pages", "page_token"] {
            assert!(!videos.contains(paging), "videos schema leaks {paging}");
            assert!(search.contains(paging));
        }
    }

    #[test]
    fn transcript_prompt_names_the_unofficial_rules() {
        let prompt = include_str!("../prompts/tools/youtube_transcript.md");
        assert!(prompt.contains("When to use"));
        assert!(prompt.contains("When not to use"));
        assert!(prompt.contains("Quota"));
        assert!(prompt.contains("unofficial"));
        assert!(prompt.contains("experimental"));
        let yaml = include_str!("../prompts/server.yaml");
        assert!(yaml.contains("youtube_transcript"));
        assert!(yaml.contains("experimental"));
        assert!(yaml.contains("not affiliated"));
        assert!(yaml.contains("ENABLE_MCP_TRANSCRIPT"));
    }

    #[test]
    fn transcript_tool_refuses_an_unknown_argument() {
        let args: TranscriptArgs = serde_json::from_value(serde_json::json!({
            "video_id": "abcdefghijk",
            "mine": true
        }))
        .unwrap();
        let err = prepare_transcript(&args).unwrap_err();
        assert!(matches!(err, ToolError::UnknownParam(name) if name == "mine"));
    }

    #[test]
    fn prompts_name_the_rules() {
        let yaml = include_str!("../prompts/server.yaml");
        assert!(yaml.contains("Read-only"));
        assert!(yaml.contains("API key"));
        assert!(yaml.contains("youtube_search"));
        let search = include_str!("../prompts/tools/youtube_search.md");
        assert!(search.contains("When to use"));
        assert!(search.contains("When not to use"));
        assert!(search.contains("Quota"));
    }

    #[test]
    fn data_api_router_omits_transcript_until_composed() {
        let data = YoutubeServer::data_api_router();
        let names: Vec<_> = data.list_all().into_iter().map(|t| t.name.to_string()).collect();
        assert_eq!(names.len(), 5, "{names:?}");
        assert!(!names.iter().any(|n| n == "youtube_transcript"));
        let both = data + YoutubeServer::transcript_router();
        let names: Vec<_> = both.list_all().into_iter().map(|t| t.name.to_string()).collect();
        assert!(names.iter().any(|n| n == "youtube_transcript"), "{names:?}");
    }
}
