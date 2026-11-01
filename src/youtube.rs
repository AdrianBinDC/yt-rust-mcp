use crate::config::TtlSeconds;
use crate::error::ToolError;
use serde_json::Value;
use std::collections::BTreeMap;

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

pub fn prepare(method: Method, raw: &Value, hard_cap: u32) -> Result<CallRequest, ToolError> {
    let obj = raw.as_object().cloned().unwrap_or_default();
    for name in AUTH_ONLY {
        if obj.contains_key(*name) {
            return Err(ToolError::AuthOnlyParam((*name).to_string()));
        }
    }
    if method == Method::Videos {
        for name in ["max_results", "max_pages", "page_token"] {
            if obj.contains_key(name) {
                return Err(ToolError::VideosListPaging(name.to_string()));
            }
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
    if let Some(handle) = obj.get("for_handle").and_then(|v| v.as_str()) {
        query.insert("forHandle".to_string(), handle.to_string());
    }
    copy_string(&obj, &mut query, "forUsername");
    if let Some(name) = obj.get("for_username").and_then(|v| v.as_str()) {
        query.insert("forUsername".to_string(), name.to_string());
    }
    copy_string(&obj, &mut query, "channelId");
    if let Some(id) = obj.get("channel_id").and_then(|v| v.as_str()) {
        query.insert("channelId".to_string(), id.to_string());
    }
    copy_string(&obj, &mut query, "playlistId");
    if let Some(id) = obj.get("playlist_id").and_then(|v| v.as_str()) {
        query.insert("playlistId".to_string(), id.to_string());
    }
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
