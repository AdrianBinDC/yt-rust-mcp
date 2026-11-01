use crate::config::TtlSeconds;
use crate::error::ToolError;
use serde_json::Value;
use std::collections::BTreeMap;

pub trait YoutubeHttp {
    fn get(
        &self,
        method: Method,
        query: &BTreeMap<String, String>,
    ) -> Result<serde_json::Value, ToolError>;
}

pub struct ReqwestYoutube {
    api_key: String,
    http: reqwest::blocking::Client,
}

impl ReqwestYoutube {
    pub fn new(api_key: String) -> Result<Self, ToolError> {
        let http = reqwest::blocking::Client::builder()
            .build()
            .map_err(map_reqwest_error)?;
        Ok(Self { api_key, http })
    }

    pub fn url(method: Method, query: &BTreeMap<String, String>, api_key: &str) -> String {
        let mut url = format!("https://youtube.googleapis.com/{}?", method.path());
        for (name, value) in query {
            url.push_str(&urlencoding_pair(name, value));
            url.push('&');
        }
        url.push_str(&urlencoding_pair("key", api_key));
        url
    }
}

/// `reqwest::blocking::Client` owns its own runtime and panics if built or
/// dropped on an async worker thread, so async callers must build it here.
pub async fn build_http(api_key: String) -> Result<ReqwestYoutube, ToolError> {
    tokio::task::spawn_blocking(move || ReqwestYoutube::new(api_key))
        .await
        .map_err(|e| ToolError::Http(format!("client build task failed: {e}")))?
}

fn urlencoding_pair(name: &str, value: &str) -> String {
    format!("{}={}", urlencoding(name), urlencoding(value))
}

fn urlencoding(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn map_reqwest_error(error: reqwest::Error) -> ToolError {
    ToolError::Http(error.without_url().to_string())
}

impl YoutubeHttp for ReqwestYoutube {
    fn get(
        &self,
        method: Method,
        query: &BTreeMap<String, String>,
    ) -> Result<serde_json::Value, ToolError> {
        let url = Self::url(method, query, &self.api_key);
        let response = self.http.get(&url).send().map_err(map_reqwest_error)?;
        let status = response.status().as_u16();
        let text = response.text().map_err(map_reqwest_error)?;
        if !(200..300).contains(&status) {
            return Err(ToolError::from_google_body(status, &text));
        }
        serde_json::from_str(&text).map_err(|error| ToolError::Http(error.to_string()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Search,
    Videos,
    Channels,
    Playlists,
    PlaylistItems,
}

impl Method {
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Search => "youtube.search.list",
            Self::Videos => "youtube.videos.list",
            Self::Channels => "youtube.channels.list",
            Self::Playlists => "youtube.playlists.list",
            Self::PlaylistItems => "youtube.playlistItems.list",
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Self::Search => "youtube/v3/search",
            Self::Videos => "youtube/v3/videos",
            Self::Channels => "youtube/v3/channels",
            Self::Playlists => "youtube/v3/playlists",
            Self::PlaylistItems => "youtube/v3/playlistItems",
        }
    }

    pub fn counter(self) -> &'static str {
        match self {
            Self::Search => "search",
            _ => "non_search",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CallRequest {
    pub method: Method,
    pub query: BTreeMap<String, String>,
    pub max_pages: u32,
    pub bypass_cache: bool,
}

const AUTH_ONLY: &[&str] = &[
    "mine",
    "forMine",
    "myRating",
    "mySubscribers",
    "managedByMe",
    "forContentOwner",
    "forDeveloper",
    "onBehalfOfContentOwner",
    "onBehalfOfContentOwnerChannel",
];

/// Tool-facing argument names per method (snake_case only). Anything else is
/// refused so a typo or camelCase alias never silently changes the request.
fn allowed_args(method: Method) -> &'static [&'static str] {
    match method {
        Method::Search => &[
            "q",
            "part",
            "type",
            "order",
            "channel_id",
            "published_after",
            "published_before",
            "video_duration",
            "region_code",
            "relevance_language",
            "safe_search",
            "page_token",
            "max_results",
            "max_pages",
            "bypass_cache",
        ],
        // No paging: paging names are refused earlier with a specific error.
        Method::Videos => &["id", "part", "hl", "bypass_cache"],
        Method::Channels => &[
            "id",
            "for_handle",
            "for_username",
            "part",
            "hl",
            "max_results",
            "max_pages",
            "page_token",
            "bypass_cache",
        ],
        Method::Playlists => &[
            "id",
            "channel_id",
            "part",
            "hl",
            "max_results",
            "max_pages",
            "page_token",
            "bypass_cache",
        ],
        Method::PlaylistItems => &[
            "playlist_id",
            "video_id",
            "part",
            "max_results",
            "max_pages",
            "page_token",
            "bypass_cache",
        ],
    }
}

pub fn prepare(method: Method, raw: &Value, hard_cap: u32) -> Result<CallRequest, ToolError> {
    let obj = raw.as_object().cloned().unwrap_or_default();
    for name in AUTH_ONLY {
        if obj.contains_key(*name) {
            return Err(ToolError::AuthOnlyParam((*name).to_string()));
        }
    }
    if method == Method::Videos {
        for name in [
            "max_results",
            "maxResults",
            "max_pages",
            "page_token",
            "pageToken",
        ] {
            if obj.contains_key(name) {
                return Err(ToolError::VideosListPaging(name.to_string()));
            }
        }
    }
    let allowed = allowed_args(method);
    for name in obj.keys() {
        if !allowed.contains(&name.as_str()) {
            return Err(ToolError::UnknownParam(name.clone()));
        }
    }
    let max_pages = match obj.get("max_pages").and_then(|v| v.as_u64()) {
        Some(n) => {
            let n = u32::try_from(n).unwrap_or(u32::MAX);
            if n > hard_cap {
                return Err(ToolError::MaxPages {
                    got: n,
                    cap: hard_cap,
                });
            }
            n.max(1)
        }
        None => 1,
    };
    let bypass_cache = obj
        .get("bypass_cache")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut query = BTreeMap::new();
    if let Some(part) = obj.get("part").and_then(|v| v.as_str()) {
        query.insert("part".to_string(), sort_csv(part));
    } else {
        query.insert("part".to_string(), default_part(method).to_string());
    }
    copy_string(&obj, &mut query, "q");
    copy_csv_id(&obj, &mut query, "id");
    copy_string(&obj, &mut query, "forHandle");
    // Tool args use snake_case. YouTube uses the names below.
    copy_aliased_string(&obj, &mut query, "for_handle", "forHandle");
    copy_string(&obj, &mut query, "forUsername");
    copy_aliased_string(&obj, &mut query, "for_username", "forUsername");
    copy_string(&obj, &mut query, "channelId");
    copy_aliased_string(&obj, &mut query, "channel_id", "channelId");
    copy_string(&obj, &mut query, "playlistId");
    copy_aliased_string(&obj, &mut query, "playlist_id", "playlistId");
    copy_string(&obj, &mut query, "type");
    copy_string(&obj, &mut query, "order");
    copy_string(&obj, &mut query, "publishedAfter");
    copy_string(&obj, &mut query, "publishedBefore");
    if let Some(v) = obj.get("published_after").and_then(|v| v.as_str()) {
        query.insert("publishedAfter".to_string(), v.to_string());
    }
    if let Some(v) = obj.get("published_before").and_then(|v| v.as_str()) {
        query.insert("publishedBefore".to_string(), v.to_string());
    }
    copy_string(&obj, &mut query, "videoDuration");
    if let Some(v) = obj.get("video_duration").and_then(|v| v.as_str()) {
        query.insert("videoDuration".to_string(), v.to_string());
    }
    copy_string(&obj, &mut query, "regionCode");
    if let Some(v) = obj.get("region_code").and_then(|v| v.as_str()) {
        query.insert("regionCode".to_string(), v.to_string());
    }
    copy_string(&obj, &mut query, "relevanceLanguage");
    if let Some(v) = obj.get("relevance_language").and_then(|v| v.as_str()) {
        query.insert("relevanceLanguage".to_string(), v.to_string());
    }
    copy_string(&obj, &mut query, "safeSearch");
    if let Some(v) = obj.get("safe_search").and_then(|v| v.as_str()) {
        query.insert("safeSearch".to_string(), v.to_string());
    }
    copy_string(&obj, &mut query, "pageToken");
    if let Some(v) = obj.get("page_token").and_then(|v| v.as_str()) {
        query.insert("pageToken".to_string(), v.to_string());
    }
    copy_string(&obj, &mut query, "hl");
    copy_string(&obj, &mut query, "videoId");
    if let Some(v) = obj.get("video_id").and_then(|v| v.as_str()) {
        query.insert("videoId".to_string(), v.to_string());
    }
    if method != Method::Videos {
        let max_results = obj
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
            .unwrap_or(25);
        if max_results > 50 {
            return Err(ToolError::MaxResults { got: max_results });
        }
        query.insert("maxResults".to_string(), max_results.to_string());
    }
    require_selector(method, &query)?;
    Ok(CallRequest {
        method,
        query,
        max_pages,
        bypass_cache,
    })
}

fn require_selector(method: Method, query: &BTreeMap<String, String>) -> Result<(), ToolError> {
    let ok = match method {
        Method::Search => true,
        Method::Videos => query.contains_key("id"),
        Method::Channels => {
            query.contains_key("id")
                || query.contains_key("forHandle")
                || query.contains_key("forUsername")
        }
        Method::Playlists => query.contains_key("id") || query.contains_key("channelId"),
        Method::PlaylistItems => query.contains_key("playlistId"),
    };
    if ok {
        Ok(())
    } else {
        Err(ToolError::MissingSelector(match method {
            Method::Videos => "id",
            Method::Channels => "id, for_handle, or for_username",
            Method::Playlists => "id or channel_id",
            Method::PlaylistItems => "playlist_id",
            Method::Search => "none",
        }))
    }
}

fn default_part(method: Method) -> &'static str {
    match method {
        Method::Search => "snippet",
        Method::Videos | Method::Channels => "contentDetails,snippet,statistics",
        Method::Playlists | Method::PlaylistItems => "contentDetails,snippet",
    }
}

fn sort_csv(value: &str) -> String {
    let mut parts: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    parts.sort_unstable();
    parts.join(",")
}

fn copy_string(
    obj: &serde_json::Map<String, Value>,
    query: &mut BTreeMap<String, String>,
    name: &str,
) {
    if let Some(v) = obj.get(name).and_then(|v| v.as_str()) {
        if !v.is_empty() {
            query.insert(name.to_string(), v.to_string());
        }
    }
}

fn copy_aliased_string(
    obj: &serde_json::Map<String, Value>,
    query: &mut BTreeMap<String, String>,
    source: &str,
    target: &str,
) {
    if let Some(v) = obj.get(source).and_then(|v| v.as_str()) {
        if !v.is_empty() {
            query.insert(target.to_string(), v.to_string());
        }
    }
}

fn copy_csv_id(
    obj: &serde_json::Map<String, Value>,
    query: &mut BTreeMap<String, String>,
    name: &str,
) {
    if let Some(v) = obj.get(name).and_then(|v| v.as_str()) {
        if !v.is_empty() {
            query.insert(name.to_string(), sort_csv(v));
        }
    }
}

pub fn cache_key(method: Method, query: &BTreeMap<String, String>) -> String {
    let mut key = String::from(method.api_name());
    for (name, value) in query {
        key.push('\n');
        key.push_str(name);
        key.push('=');
        key.push_str(value);
    }
    key
}

pub fn ttl_seconds(method: Method, ttl: &TtlSeconds) -> i64 {
    match method {
        Method::Search => ttl.search,
        Method::Videos => ttl.videos,
        Method::Channels => ttl.channels,
        Method::Playlists => ttl.playlists,
        Method::PlaylistItems => ttl.playlist_items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn live_client_sends_the_key_and_does_not_return_it() {
        let query = BTreeMap::from([("part".to_string(), "snippet".to_string())]);
        let url = ReqwestYoutube::url(Method::Videos, &query, "test-key");
        assert!(url.contains("key=test-key"));
        assert!(url.contains("part=snippet"));
        assert!(!url.contains("test-keytest-key"));
    }

    #[test]
    fn reqwest_errors_do_not_expose_request_urls() {
        let error = reqwest::blocking::Client::new()
            .get("ftp://example.test/videos?key=secret-key")
            .send()
            .unwrap_err();
        assert!(error.url().unwrap().as_str().contains("key=secret-key"));

        let mapped = map_reqwest_error(error).to_string();
        assert!(!mapped.contains("key="));
        assert!(!mapped.contains("secret-key"));
        assert!(!mapped.contains("example.test"));
    }

    #[test]
    fn omitted_part_matches_explicit_default_and_drops_the_key() {
        let raw = json!({"id": "b,a"});
        let prepared = prepare(Method::Videos, &raw, 5).unwrap();
        assert_eq!(
            prepared.query.get("part").map(String::as_str),
            Some("contentDetails,snippet,statistics")
        );
        assert!(!prepared.query.contains_key("maxResults"));
        assert!(!cache_key(Method::Videos, &prepared.query).contains("key"));

        let explicit = json!({"id": "a,b", "part": "statistics,snippet,contentDetails"});
        let other = prepare(Method::Videos, &explicit, 5).unwrap();
        assert_eq!(
            cache_key(Method::Videos, &prepared.query),
            cache_key(Method::Videos, &other.query)
        );
    }

    #[test]
    fn videos_list_refuses_paging_args() {
        let raw = json!({"id": "abc", "max_results": 10});
        let err = prepare(Method::Videos, &raw, 5).unwrap_err();
        assert!(err.to_string().contains("max_results"));
    }

    #[test]
    fn videos_list_refuses_camel_case_paging_args() {
        for name in ["maxResults", "pageToken"] {
            let mut raw = json!({"id": "abc"});
            raw.as_object_mut()
                .unwrap()
                .insert(name.to_string(), json!("unexpected"));
            let err = prepare(Method::Videos, &raw, 5).unwrap_err();
            assert!(err.to_string().contains(name));
        }
    }

    #[test]
    fn empty_playlist_selector_is_missing() {
        let err = prepare(Method::PlaylistItems, &json!({"playlist_id": ""}), 5).unwrap_err();
        assert!(matches!(err, ToolError::MissingSelector("playlist_id")));
    }

    #[test]
    fn refuses_unknown_and_camel_case_args() {
        for name in ["playlistId", "maxResults", "nope"] {
            let raw = json!({"playlist_id": "PL1", name: 1});
            let err = prepare(Method::PlaylistItems, &raw, 5).unwrap_err();
            assert!(matches!(err, ToolError::UnknownParam(ref n) if n == name));
        }
        // hl is valid for channels but not for search.
        let err = prepare(Method::Search, &json!({"q": "x", "hl": "en"}), 5).unwrap_err();
        assert!(matches!(err, ToolError::UnknownParam(ref n) if n == "hl"));
    }

    #[test]
    fn videos_list_refuses_null_paging_args() {
        for name in ["max_results", "max_pages", "page_token", "pageToken"] {
            let raw = json!({"id": "abc", name: null});
            let err = prepare(Method::Videos, &raw, 5).unwrap_err();
            assert!(matches!(err, ToolError::VideosListPaging(ref n) if n == name));
        }
    }

    #[test]
    fn refuses_auth_only_parameter() {
        let raw = json!({"forMine": true, "q": "rust"});
        let err = prepare(Method::Search, &raw, 5).unwrap_err();
        assert!(err.to_string().contains("forMine"));
    }

    #[test]
    fn search_default_page_size_is_25() {
        let prepared = prepare(Method::Search, &json!({"q": "rust"}), 5).unwrap();
        assert_eq!(
            prepared.query.get("maxResults").map(String::as_str),
            Some("25")
        );
        assert_eq!(prepared.max_pages, 1);
    }
}
