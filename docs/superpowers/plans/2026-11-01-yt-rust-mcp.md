# yt-rust-mcp v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a stdio MCP server that reads public YouTube Data API v3 data through five tools, with a SQLite cache and two quota counters.

**Architecture:** Tool handlers validate arguments, then one call path checks SQLite, debits one counter, and GETs YouTube on a miss. Response bodies stay in SQLite. A lookup loads one row. `rmcp` speaks stdio. Logs go to stderr.

**Tech Stack:** Rust 1.89, `rmcp` 3.8 (`server`, `transport-io`), `tokio`, `reqwest` (`json`, `rustls-tls`), `rusqlite` (`bundled`), `serde`/`serde_json`, `schemars` 1, `toml` 0.9, `thiserror` 2, `tracing` + `tracing-subscriber`, `jiff` (`static`), `dirs` 6.

## Global Constraints

- v1 is read-only YouTube Data API v3. Auth is `YOUTUBE_API_KEY` as query `key`. No OAuth, no writes.
- Tools: `youtube_search`, `youtube_videos_list`, `youtube_channels_list`, `youtube_playlists_list`, `youtube_playlist_items_list`. Deferred catalog methods are not tools.
- Transport is stdio only. Do not enable `rmcp`'s HTTP transport features.
- Refuse auth-only arguments: `mine`, `forMine`, `myRating`, `mySubscribers`, `managedByMe`, `forContentOwner`, `forDeveloper`, `onBehalfOfContentOwner`, `onBehalfOfContentOwnerChannel`. Do not drop them and call YouTube.
- Default `part` when omitted: search `snippet`; videos and channels `snippet,contentDetails,statistics`; playlists and playlist items `snippet,contentDetails`.
- `max_results` default 25, cap 50. `max_pages` default 1. `max_pages_hard_cap` is 5.
- `youtube_videos_list` is an `id` lookup. Do not send `maxResults` or `pageToken`. Refuse `max_results`, `max_pages`, or `page_token` on that tool.
- Required selector before debit or HTTP: videos `id`; channels one of `id`, `for_handle`, `for_username`; playlists one of `id`, `channel_id`; playlist items `playlist_id`. Search has no required selector.
- Cache key is the effective YouTube query after defaults. Sort names. Sort `part` and comma-separated ids. Include `pageToken` and `maxResults` only when sent. Exclude the API key, `bypass_cache`, and `max_pages`.
- One SQLite file `yt-rust-mcp.db`. Page cache capped at 2 MiB. Max 10,000 cache rows. LRU eviction selects keys and `last_used` only.
- Counters are `search` and `non_search`. `search.list` spends 1 `search` only (default budget 100). The other four spend 1 `non_search` only (default budget 10,000). A cache hit spends neither. Debit once per live page and do not refund on YouTube errors.
- Reset both counters when the `America/Los_Angeles` calendar day changes.
- TTLs: channels and playlists 7 days; videos and playlist items 24 hours; search 1 hour. Values live in TOML.
- `cached` is true only when every page in the call was a cache hit. Do not store a merged multi-page blob.
- Never swallow errors. Never log or return the API key.
- Logs: `tracing` to stderr. Default level warn. Debug may log hit/miss, debit, and request shape.
- Tests use fixtures under `fixtures/youtube/`. `cargo test` must not open a socket to Google.
- Comments are concise and high-signal. Explain a decision the code does not already show.
- Initial development commits go to `main`.

## File structure

| File | Responsibility |
|---|---|
| `rust-toolchain.toml` | Pin Rust 1.89 |
| `Cargo.toml` | The crate list above |
| `config/default.toml` | TTLs, caps, budgets |
| `prompts/server.md` | Server instructions |
| `prompts/tools/*.md` | When to use, when not to, quota hint |
| `src/config.rs` | Load TOML, apply env overrides |
| `src/error.rs` | Tool errors and Google error JSON mapping |
| `src/youtube.rs` | Cache key, query builder, HTTP trait |
| `src/store.rs` | SQLite cache rows and quota counters |
| `src/call.rs` | Cache, then quota, then HTTP, including pages |
| `src/server.rs` | Five `rmcp` tools |
| `src/main.rs` | stderr logging and stdio serve |
| `fixtures/youtube/` | Synthetic JSON for tests |
| `Dockerfile` | Image name `yt-rust-mcp` |
| `README.md` | How a harness launches the binary |

---

### Task 1: Crate, toolchain, and config

**Files:**
- Create: `rust-toolchain.toml`
- Create: `Cargo.toml`
- Create: `config/default.toml`
- Create: `src/config.rs`
- Create: `src/lib.rs`
- Create: `src/main.rs` (stub until Task 8)

**Interfaces:**
- Consumes: nothing
- Produces: `yt_rust_mcp::config::Config` and `Config::load(config_path: &Path) -> Result<Config, ConfigError>`

- [ ] **Step 1: Write the failing test**

Create `src/config.rs` with only the test and a stub type so the test compiles after Step 3. First add the files from Step 3's `Cargo.toml` and toolchain, then this test, then run it.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn env_budget_overrides_toml() {
        let dir = std::env::temp_dir().join(format!("yt-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("default.toml");
        let mut file = std::fs::File::create(&path).unwrap();
        write!(
            file,
            "search_budget = 100\nnon_search_budget = 10000\nmax_entries = 10000\nmax_pages_hard_cap = 5\nsqlite_page_cache_kib = 2048\ncache_dir = \"/tmp/yt-default\"\n"
        )
        .unwrap();
        write!(
            file,
            "[ttl_seconds]\nsearch = 3600\nvideos = 86400\nchannels = 604800\nplaylists = 604800\nplaylist_items = 86400\n"
        )
        .unwrap();
        std::env::set_var("YOUTUBE_SEARCH_BUDGET", "7");
        std::env::set_var("YT_RUST_MCP_CACHE_DIR", dir.join("cache").to_str().unwrap());
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.search_budget, 7);
        assert_eq!(cfg.non_search_budget, 10_000);
        assert_eq!(cfg.ttl_seconds.search, 3600);
        assert_eq!(cfg.db_path, dir.join("cache").join("yt-rust-mcp.db"));
        std::env::remove_var("YOUTUBE_SEARCH_BUDGET");
        std::env::remove_var("YT_RUST_MCP_CACHE_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test env_budget_overrides_toml -- --nocapture`

Expected: FAIL because `Config` is not defined yet. After the stub exists, FAIL on the assertion or on missing `load`.

- [ ] **Step 3: Write the crate and the loader**

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.89.0"
```

`Cargo.toml`:

```toml
[package]
name = "yt-rust-mcp"
version = "0.1.0"
edition = "2021"
rust-version = "1.89"
license = "MIT"
description = "Read-only YouTube Data API v3 MCP server"

[dependencies]
rmcp = { version = "3.8", features = ["server", "transport-io"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "blocking"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "1"
toml = "0.9"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
jiff = { version = "0.2", features = ["static"] }
dirs = "6"
rusqlite = { version = "0.37", features = ["bundled"] }
```

Use these versions. Do not add crates that are not in this file.

`config/default.toml`:

```toml
search_budget = 100
non_search_budget = 10000
max_entries = 10000
max_pages_hard_cap = 5
sqlite_page_cache_kib = 2048

[ttl_seconds]
search = 3600
videos = 86400
channels = 604800
playlists = 604800
playlist_items = 86400
```

`src/lib.rs`:

```rust
pub mod config;
```

`src/config.rs`:

```rust
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("read config: {0}")]
    Read(#[from] std::io::Error),
    #[error("parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("bad integer in {name}: {value}")]
    BadInt { name: &'static str, value: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct TtlSeconds {
    pub search: i64,
    pub videos: i64,
    pub channels: i64,
    pub playlists: i64,
    pub playlist_items: i64,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub search_budget: i64,
    pub non_search_budget: i64,
    pub max_entries: i64,
    pub max_pages_hard_cap: u32,
    pub sqlite_page_cache_kib: i64,
    pub ttl_seconds: TtlSeconds,
    pub cache_dir: PathBuf,
    pub db_path: PathBuf,
    pub prompts_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    search_budget: i64,
    non_search_budget: i64,
    max_entries: i64,
    max_pages_hard_cap: u32,
    sqlite_page_cache_kib: i64,
    cache_dir: Option<PathBuf>,
    ttl_seconds: TtlSeconds,
}

impl Config {
    pub fn load(config_path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(config_path)?;
        let file: FileConfig = toml::from_str(&text)?;
        let search_budget = env_i64("YOUTUBE_SEARCH_BUDGET", file.search_budget)?;
        let non_search_budget = env_i64("YOUTUBE_NON_SEARCH_BUDGET", file.non_search_budget)?;
        let cache_dir = std::env::var_os("YT_RUST_MCP_CACHE_DIR")
            .map(PathBuf::from)
            .or(file.cache_dir)
            .or_else(|| dirs::cache_dir().map(|d| d.join("yt-rust-mcp")))
            .unwrap_or_else(|| PathBuf::from("yt-rust-mcp-cache"));
        let prompts_dir = std::env::var_os("YT_RUST_MCP_PROMPTS")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("prompts"));
        Ok(Self {
            search_budget,
            non_search_budget,
            max_entries: file.max_entries,
            max_pages_hard_cap: file.max_pages_hard_cap,
            sqlite_page_cache_kib: file.sqlite_page_cache_kib,
            ttl_seconds: file.ttl_seconds,
            db_path: cache_dir.join("yt-rust-mcp.db"),
            cache_dir,
            prompts_dir,
        })
    }
}

fn env_i64(name: &'static str, default: i64) -> Result<i64, ConfigError> {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| ConfigError::BadInt { name, value }),
        Err(_) => Ok(default),
    }
}
```

`src/main.rs` stub:

```rust
fn main() {
    println!("yt-rust-mcp");
}
```

The stub prints to stdout. Task 8 replaces it. Do not run this stub as an MCP server.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test env_budget_overrides_toml -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rust-toolchain.toml Cargo.toml Cargo.lock config/default.toml src/lib.rs src/config.rs src/main.rs
git commit -m "$(cat <<'EOF'
Load TOML config with env overrides for budgets and the cache path.

EOF
)"
```

---

### Task 2: Errors and Google error JSON

**Files:**
- Create: `src/error.rs`
- Create: `fixtures/youtube/error.quotaExceeded.json`
- Create: `fixtures/youtube/error.forbidden.json`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: nothing
- Produces: `ToolError`, `ToolError::from_google_body(status: u16, body: &str) -> ToolError`

- [ ] **Step 1: Write the failing test**

`fixtures/youtube/error.quotaExceeded.json`:

```json
{"error":{"code":403,"message":"The request cannot be completed because you have exceeded your quota.","errors":[{"reason":"quotaExceeded","domain":"youtube.quota","message":"The request cannot be completed because you have exceeded your quota."}]}}
```

`fixtures/youtube/error.forbidden.json`:

```json
{"error":{"code":403,"message":"Forbidden","errors":[{"reason":"forbidden","domain":"global","message":"Forbidden"}]}}
```

In `src/error.rs`:

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test maps_quota_exceeded_reason -- --nocapture`

Expected: FAIL to compile (`ToolError` missing)

- [ ] **Step 3: Implement the mapper**

```rust
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
```

`src/lib.rs` add `pub mod error;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test maps_quota_exceeded_reason -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/error.rs src/lib.rs fixtures/youtube/error.quotaExceeded.json fixtures/youtube/error.forbidden.json
git commit -m "$(cat <<'EOF'
Map Google error JSON to a tool error without including the API key.

EOF
)"
```

---

### Task 3: Cache key and YouTube query

**Files:**
- Create: `src/youtube.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `ToolError`
- Produces:
  - `pub enum Method { Search, Videos, Channels, Playlists, PlaylistItems }`
  - `pub struct CallRequest { pub method: Method, pub query: BTreeMap<String, String>, pub max_pages: u32, pub bypass_cache: bool }`
  - `pub fn prepare(method, raw: &serde_json::Value, hard_cap: u32) -> Result<CallRequest, ToolError>`
  - `pub fn cache_key(method: Method, query: &BTreeMap<String, String>) -> String`
  - `pub fn ttl_seconds(method: Method, ttl: &crate::config::TtlSeconds) -> i64`

- [ ] **Step 1: Write the failing test**

```rust
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
        assert!(prepared.query.get("maxResults").is_none());
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
        assert_eq!(prepared.query.get("maxResults").map(String::as_str), Some("25"));
        assert_eq!(prepared.max_pages, 1);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib youtube -- --nocapture`

Expected: FAIL to compile

- [ ] **Step 3: Implement prepare and cache_key**

```rust
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
                return Err(ToolError::MaxPages { got: n, cap: hard_cap });
            }
            n.max(1)
        }
        None => 1,
    };
    let bypass_cache = obj.get("bypass_cache").and_then(|v| v.as_bool()).unwrap_or(false);
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
    let mut parts: Vec<&str> = value.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    parts.sort_unstable();
    parts.join(",")
}

fn copy_string(obj: &serde_json::Map<String, Value>, query: &mut BTreeMap<String, String>, name: &str) {
    if let Some(v) = obj.get(name).and_then(|v| v.as_str()) {
        if !v.is_empty() {
            query.insert(name.to_string(), v.to_string());
        }
    }
}

fn copy_csv_id(obj: &serde_json::Map<String, Value>, query: &mut BTreeMap<String, String>, name: &str) {
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
```

`src/lib.rs` add `pub mod youtube;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib youtube:: -- --nocapture`

Expected: PASS for the four tests in this task

- [ ] **Step 5: Commit**

```bash
git add src/youtube.rs src/lib.rs
git commit -m "$(cat <<'EOF'
Build the YouTube query and cache key from tool arguments.

EOF
)"
```

---

### Task 4: SQLite cache and quota counters

**Files:**
- Create: `src/store.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `Config` fields `db_path`, `sqlite_page_cache_kib`, `max_entries`, budgets
- Produces:
  - `Store::open(path, page_cache_kib) -> Result<Store, ToolError>`
  - `Store::get_fresh(&self, key: &str, now: i64) -> Result<Option<String>, ToolError>`
  - `Store::put(&self, key, body, expires_at, now, max_entries) -> Result<(), ToolError>`
  - `Store::can_spend(&self, counter, cost, budget, today: &str) -> Result<bool, ToolError>`
  - `Store::remaining(&self, counter, budget, today: &str) -> Result<i64, ToolError>`
  - `Store::debit(&self, counter, cost, budget, today: &str) -> Result<(), ToolError>`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> Store {
        let path = std::env::temp_dir().join(format!(
            "yt-store-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        Store::open(&path, 2048).unwrap()
    }

    #[test]
    fn fresh_row_hits_and_expired_row_misses() {
        let store = temp_store();
        store.put("k", "{\"items\":[]}", 200, 100, 10).unwrap();
        assert!(store.get_fresh("k", 150).unwrap().is_some());
        assert!(store.get_fresh("k", 200).unwrap().is_none());
    }

    #[test]
    fn debit_stops_at_the_budget_and_resets_on_a_new_day() {
        let store = temp_store();
        store.debit("search", 1, 2, "2026-11-01").unwrap();
        store.debit("search", 1, 2, "2026-11-01").unwrap();
        assert!(store.debit("search", 1, 2, "2026-11-01").is_err());
        store.debit("search", 1, 2, "2026-11-02").unwrap();
    }

    #[test]
    fn search_debit_does_not_change_non_search() {
        let store = temp_store();
        store.debit("search", 1, 100, "2026-11-01").unwrap();
        assert!(store.can_spend("non_search", 1, 10_000, "2026-11-01").unwrap());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib store:: -- --nocapture`

Expected: FAIL to compile

- [ ] **Step 3: Implement the store**

```rust
use crate::error::ToolError;
use rusqlite::{params, Connection};
use std::path::Path;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path, page_cache_kib: i64) -> Result<Self, ToolError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::Store(e.to_string()))?;
        }
        let conn = Connection::open(path).map_err(|e| ToolError::Store(e.to_string()))?;
        // Negative cache_size is kibibytes. 2048 KiB is the 2 MiB cap.
        conn.pragma_update(None, "cache_size", -page_cache_kib)
            .map_err(|e| ToolError::Store(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cache (
                cache_key TEXT PRIMARY KEY,
                body TEXT NOT NULL,
                expires_at INTEGER NOT NULL,
                last_used INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS quota (
                name TEXT PRIMARY KEY,
                spent INTEGER NOT NULL,
                day TEXT NOT NULL
            );",
        )
        .map_err(|e| ToolError::Store(e.to_string()))?;
        Ok(Self { conn })
    }

    pub fn get_fresh(&self, key: &str, now: i64) -> Result<Option<String>, ToolError> {
        let mut stmt = self
            .conn
            .prepare("SELECT body, expires_at FROM cache WHERE cache_key = ?1")
            .map_err(|e| ToolError::Store(e.to_string()))?;
        let mut rows = stmt
            .query(params![key])
            .map_err(|e| ToolError::Store(e.to_string()))?;
        let Some(row) = rows.next().map_err(|e| ToolError::Store(e.to_string()))? else {
            return Ok(None);
        };
        let body: String = row.get(0).map_err(|e| ToolError::Store(e.to_string()))?;
        let expires_at: i64 = row.get(1).map_err(|e| ToolError::Store(e.to_string()))?;
        if expires_at <= now {
            self.conn
                .execute("DELETE FROM cache WHERE cache_key = ?1", params![key])
                .map_err(|e| ToolError::Store(e.to_string()))?;
            return Ok(None);
        }
        self.conn
            .execute(
                "UPDATE cache SET last_used = ?1 WHERE cache_key = ?2",
                params![now, key],
            )
            .map_err(|e| ToolError::Store(e.to_string()))?;
        Ok(Some(body))
    }

    pub fn put(
        &self,
        key: &str,
        body: &str,
        expires_at: i64,
        now: i64,
        max_entries: i64,
    ) -> Result<(), ToolError> {
        self.conn
            .execute(
                "INSERT INTO cache (cache_key, body, expires_at, last_used)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(cache_key) DO UPDATE SET
                    body = excluded.body,
                    expires_at = excluded.expires_at,
                    last_used = excluded.last_used",
                params![key, body, expires_at, now],
            )
            .map_err(|e| ToolError::Store(e.to_string()))?;
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM cache", [], |row| row.get(0))
            .map_err(|e| ToolError::Store(e.to_string()))?;
        if count > max_entries {
            let extra = count - max_entries;
            self.conn
                .execute(
                    "DELETE FROM cache WHERE cache_key IN (
                        SELECT cache_key FROM cache ORDER BY last_used ASC LIMIT ?1
                    )",
                    params![extra],
                )
                .map_err(|e| ToolError::Store(e.to_string()))?;
        }
        Ok(())
    }

    pub fn can_spend(
        &self,
        counter: &str,
        cost: i64,
        budget: i64,
        today: &str,
    ) -> Result<bool, ToolError> {
        let spent = self.spent_today(counter, today)?;
        Ok(spent + cost <= budget)
    }

    pub fn remaining(&self, counter: &str, budget: i64, today: &str) -> Result<i64, ToolError> {
        Ok(budget - self.spent_today(counter, today)?)
    }

    pub fn debit(
        &self,
        counter: &str,
        cost: i64,
        budget: i64,
        today: &str,
    ) -> Result<(), ToolError> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| ToolError::Store(e.to_string()))?;
        let spent = spent_today_tx(&tx, counter, today)?;
        if spent + cost > budget {
            return Err(ToolError::QuotaExhausted {
                counter: if counter == "search" { "search" } else { "non_search" },
                remaining: budget - spent,
                cost,
            });
        }
        tx.execute(
            "INSERT INTO quota (name, spent, day) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET spent = ?2, day = ?3",
            params![counter, spent + cost, today],
        )
        .map_err(|e| ToolError::Store(e.to_string()))?;
        tx.commit().map_err(|e| ToolError::Store(e.to_string()))?;
        Ok(())
    }

    fn spent_today(&self, counter: &str, today: &str) -> Result<i64, ToolError> {
        spent_today_tx(&self.conn, counter, today)
    }
}

fn spent_today_tx(conn: &Connection, counter: &str, today: &str) -> Result<i64, ToolError> {
    let row: Option<(i64, String)> = conn
        .query_row(
            "SELECT spent, day FROM quota WHERE name = ?1",
            params![counter],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();
    match row {
        Some((spent, day)) if day == today => Ok(spent),
        _ => Ok(0),
    }
}
```

A day change returns spent 0 from `spent_today_tx` until the next debit writes the new day. `can_spend` uses that same read. Do not refund a debit anywhere in this file.

`src/lib.rs` add `pub mod store;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib store:: -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/store.rs src/lib.rs
git commit -m "$(cat <<'EOF'
Store cache rows and the two quota counters in one SQLite file.

EOF
)"
```

---

### Task 5: Call path with a fake HTTP client

**Files:**
- Create: `src/call.rs`
- Create: `fixtures/youtube/search.list.ok.json`
- Create: `fixtures/youtube/search.list.page2.json`
- Create: `fixtures/youtube/videos.list.ok.json`
- Modify: `src/lib.rs`
- Modify: `src/youtube.rs` (add the HTTP trait)

**Interfaces:**
- Consumes: `CallRequest`, `Store`, `ToolError`, `TtlSeconds`
- Produces:
  - `pub trait YoutubeHttp { fn get(&self, method: Method, query: &BTreeMap<String, String>) -> Result<serde_json::Value, ToolError>; }`
  - `pub fn execute(store, http, request, ttl, budgets, now, today, max_entries) -> Result<serde_json::Value, ToolError>`

The returned JSON is the YouTube body plus `cached` (bool) and, when a further page exists, `quota_note` (string). `nextPageToken` stays the YouTube field.

- [ ] **Step 1: Write fixtures and the failing test**

`fixtures/youtube/search.list.ok.json`:

```json
{"items":[{"id":{"videoId":"vid1"}}],"nextPageToken":"PAGE2"}
```

`fixtures/youtube/search.list.page2.json`:

```json
{"items":[{"id":{"videoId":"vid2"}}]}
```

`fixtures/youtube/videos.list.ok.json`:

```json
{"items":[{"id":"vid1","snippet":{"title":"Example"}}]}
```

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::youtube::{prepare, Method};
    use crate::config::TtlSeconds;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    struct Fake {
        hits: Mutex<Vec<String>>,
    }

    impl YoutubeHttp for Fake {
        fn get(&self, method: Method, query: &BTreeMap<String, String>) -> Result<serde_json::Value, crate::error::ToolError> {
            let token = query.get("pageToken").map(String::as_str).unwrap_or("");
            self.hits.lock().unwrap().push(format!("{}:{token}", method.api_name()));
            let body = if token == "PAGE2" {
                include_str!("../fixtures/youtube/search.list.page2.json")
            } else if method == Method::Videos {
                include_str!("../fixtures/youtube/videos.list.ok.json")
            } else {
                include_str!("../fixtures/youtube/search.list.ok.json")
            };
            Ok(serde_json::from_str(body).unwrap())
        }
    }

    fn ttl() -> TtlSeconds {
        TtlSeconds { search: 3600, videos: 86400, channels: 604800, playlists: 604800, playlist_items: 86400 }
    }

    #[test]
    fn second_identical_search_is_cached_and_does_not_debit_again() {
        let path = std::env::temp_dir().join(format!("yt-call-{}.db", std::process::id()));
        let store = Store::open(&path, 2048).unwrap();
        let http = Fake { hits: Mutex::new(vec![]) };
        let request = prepare(Method::Search, &json!({"q": "rust"}), 5).unwrap();
        let first = execute(&store, &http, request.clone(), &ttl(), 100, 10_000, 1_000, "2026-11-01", 10_000).unwrap();
        assert_eq!(first["cached"], false);
        let second = execute(&store, &http, request, &ttl(), 100, 10_000, 1_100, "2026-11-01", 10_000).unwrap();
        assert_eq!(second["cached"], true);
        assert_eq!(http.hits.lock().unwrap().len(), 1);
        assert!(store.can_spend("search", 99, 100, "2026-11-01").unwrap());
    }

    #[test]
    fn two_pages_concatenate_items_and_debit_twice() {
        let path = std::env::temp_dir().join(format!("yt-pages-{}.db", std::process::id()));
        let store = Store::open(&path, 2048).unwrap();
        let http = Fake { hits: Mutex::new(vec![]) };
        let request = prepare(Method::Search, &json!({"q": "rust", "max_pages": 2}), 5).unwrap();
        let body = execute(&store, &http, request, &ttl(), 100, 10_000, 1_000, "2026-11-01", 10_000).unwrap();
        assert_eq!(body["items"].as_array().unwrap().len(), 2);
        assert!(body.get("nextPageToken").is_none());
        assert_eq!(body["cached"], false);
        assert!(store.can_spend("search", 98, 100, "2026-11-01").unwrap());
        assert!(!store.can_spend("search", 99, 100, "2026-11-01").unwrap());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib call:: -- --nocapture`

Expected: FAIL to compile

- [ ] **Step 3: Implement execute**

Add the trait to `src/youtube.rs`:

```rust
pub trait YoutubeHttp {
    fn get(
        &self,
        method: Method,
        query: &BTreeMap<String, String>,
    ) -> Result<serde_json::Value, ToolError>;
}
```

`src/call.rs`:

```rust
use crate::config::TtlSeconds;
use crate::error::ToolError;
use crate::store::Store;
use crate::youtube::{cache_key, ttl_seconds, CallRequest, Method, YoutubeHttp};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn execute(
    store: &Store,
    http: &impl YoutubeHttp,
    request: CallRequest,
    ttl: &TtlSeconds,
    search_budget: i64,
    non_search_budget: i64,
    now: i64,
    today: &str,
    max_entries: i64,
) -> Result<Value, ToolError> {
    let mut page_query = request.query.clone();
    let mut merged_items = Vec::new();
    let mut carry_token = page_query.get("pageToken").cloned();
    let mut all_cached = true;
    let ttl_for_method = ttl_seconds(request.method, ttl);
    let budget = if request.method == Method::Search {
        search_budget
    } else {
        non_search_budget
    };
    for page in 0..request.max_pages {
        if page > 0 {
            let Some(token) = carry_token.clone() else {
                break;
            };
            page_query.insert("pageToken".to_string(), token);
        }
        let key = cache_key(request.method, &page_query);
        let cached_body = if request.bypass_cache {
            None
        } else {
            store.get_fresh(&key, now)?
        };
        let body = if let Some(text) = cached_body {
            serde_json::from_str(&text).map_err(|e| ToolError::Store(e.to_string()))?
        } else {
            all_cached = false;
            if !store.can_spend(request.method.counter(), 1, budget, today)? {
                let remaining = store.remaining(request.method.counter(), budget, today)?;
                return Err(ToolError::QuotaExhausted {
                    counter: request.method.counter(),
                    remaining,
                    cost: 1,
                });
            }
            store.debit(request.method.counter(), 1, budget, today)?;
            let live = http.get(request.method, &page_query)?;
            if live.get("error").is_some() {
                let status = live
                    .pointer("/error/code")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(400) as u16;
                return Err(ToolError::from_google_body(status, &live.to_string()));
            }
            store.put(
                &key,
                &live.to_string(),
                now + ttl_for_method,
                now,
                max_entries,
            )?;
            live
        };
        if let Some(items) = body.get("items").and_then(|v| v.as_array()) {
            merged_items.extend(items.clone());
        }
        carry_token = body
            .get("nextPageToken")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        if carry_token.is_none() {
            break;
        }
    }
    let mut out = json!({"items": merged_items, "cached": all_cached});
    if let Some(token) = carry_token {
        out["nextPageToken"] = json!(token);
        out["quota_note"] = json!(
            "Another page costs another call. Pass page_token or raise max_pages."
        );
    }
    Ok(out)
}
```

`src/lib.rs` add `pub mod call;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib call:: -- --nocapture`

Expected: PASS. `second_identical_search` shows one HTTP call and `search` spent 1. `two_pages` shows `search` spent 2.

- [ ] **Step 5: Commit**

```bash
git add src/call.rs src/youtube.rs src/lib.rs fixtures/youtube/search.list.ok.json fixtures/youtube/search.list.page2.json fixtures/youtube/videos.list.ok.json
git commit -m "$(cat <<'EOF'
Serve a fresh cache row before debiting quota for a live page.

EOF
)"
```

---

### Task 6: Live HTTP client

**Files:**
- Modify: `src/youtube.rs`

**Interfaces:**
- Consumes: `YoutubeHttp`, `Method`
- Produces: `ReqwestYoutube { api_key: String, http: reqwest::Client }` implementing `YoutubeHttp`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn live_client_sends_the_key_and_does_not_return_it() {
    let query = BTreeMap::from([("part".to_string(), "snippet".to_string())]);
    let url = ReqwestYoutube::url(Method::Videos, &query, "test-key");
    assert!(url.contains("key=test-key"));
    assert!(url.contains("part=snippet"));
    assert!(!url.contains("test-keytest-key"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test live_client_sends_the_key -- --nocapture`

Expected: FAIL to compile

- [ ] **Step 3: Implement the client**

```rust
pub struct ReqwestYoutube {
    api_key: String,
    http: reqwest::blocking::Client,
}

impl ReqwestYoutube {
    pub fn new(api_key: String) -> Result<Self, ToolError> {
        let http = reqwest::blocking::Client::builder()
            .build()
            .map_err(|e| ToolError::Http(e.to_string()))?;
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

impl YoutubeHttp for ReqwestYoutube {
    fn get(&self, method: Method, query: &BTreeMap<String, String>) -> Result<serde_json::Value, ToolError> {
        let url = Self::url(method, query, &self.api_key);
        let response = self.http.get(&url).send().map_err(|e| ToolError::Http(e.to_string()))?;
        let status = response.status().as_u16();
        let text = response.text().map_err(|e| ToolError::Http(e.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(ToolError::from_google_body(status, &text));
        }
        serde_json::from_str(&text).map_err(|e| ToolError::Http(e.to_string()))
    }
}
```

`reqwest` already includes the `blocking` feature from Task 1. One tool call walks a handful of pages on a blocking thread. The SQLite connection stays behind a mutex.

Do not log `url`. Debug logs may log `method.api_name()` and the query map, which has no key.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test live_client_sends_the_key -- --nocapture`

Expected: PASS. This test does not open a socket.

- [ ] **Step 5: Commit**

```bash
git add src/youtube.rs Cargo.toml Cargo.lock
git commit -m "$(cat <<'EOF'
Send the API key on the YouTube request and keep it out of errors.

EOF
)"
```

---

### Task 7: MCP tools

**Files:**
- Create: `prompts/server.md`
- Create: `prompts/tools/youtube_search.md`
- Create: `prompts/tools/youtube_videos_list.md`
- Create: `prompts/tools/youtube_channels_list.md`
- Create: `prompts/tools/youtube_playlists_list.md`
- Create: `prompts/tools/youtube_playlist_items_list.md`
- Create: `src/server.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `execute`, `prepare`, `Config`, `Store`, `ReqwestYoutube`
- Produces: `YoutubeServer` implementing the five tools and `ServerHandler`

- [ ] **Step 1: Write the prompt files and a failing test**

`prompts/server.md`:

```markdown
Read-only YouTube Data API v3. Auth is an API key. Data is public.

Tools: youtube_search, youtube_videos_list, youtube_channels_list, youtube_playlists_list, youtube_playlist_items_list.

No uploads, no edits, no OAuth, no comments, no captions, no live chat.
Prefer an id lookup over youtube_search when the id is already known.
youtube_search spends the search counter. The other tools spend non_search.
```

`prompts/tools/youtube_search.md`:

```markdown
When to use: you need to find a public video, channel, or playlist by words.
When not to use: you already have a video id, channel id, or playlist id.
Quota: 1 search per live page. Default one page. A cached repeat costs 0.
```

`prompts/tools/youtube_videos_list.md`:

```markdown
When to use: you have one or more video ids and want title, duration, and statistics.
When not to use: you still need to discover the id. Do not pass max_results, max_pages, or page_token.
Quota: 1 non_search per live call. A cached repeat costs 0.
```

`prompts/tools/youtube_channels_list.md`:

```markdown
When to use: you have a channel id, handle, or username. The uploads playlist id is in contentDetails.
When not to use: you need a keyword search.
Quota: 1 non_search per live page. A cached repeat costs 0.
```

`prompts/tools/youtube_playlists_list.md`:

```markdown
When to use: you have a playlist id or a channel id and want playlist metadata.
When not to use: you need the videos inside the playlist. Use youtube_playlist_items_list.
Quota: 1 non_search per live page. A cached repeat costs 0.
```

`prompts/tools/youtube_playlist_items_list.md`:

```markdown
When to use: you have a playlist id and want the videos in it.
When not to use: you only need the playlist title. Use youtube_playlists_list.
Quota: 1 non_search per live page. A cached repeat costs 0.
```

```rust
#[cfg(test)]
mod tests {
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
```

Put this test in `src/server.rs`.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test prompts_name_the_rules -- --nocapture`

Expected: FAIL until `src/server.rs` is a module. After adding the module with only the test, PASS is acceptable for this assertion because the fixtures are `include_str`. The tool wiring is verified by `cargo build` in Step 4.

- [ ] **Step 3: Implement the server**

`src/server.rs` (keep the test from Step 1):

```rust
use crate::call::execute;
use crate::config::Config;
use crate::error::ToolError;
use crate::store::Store;
use crate::youtube::{prepare, Method, ReqwestYoutube, YoutubeHttp};
use jiff::tz::TimeZone;
use jiff::Zoned;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Mutex;

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

    fn run(&self, method: Method, args: Value) -> Result<Value, ToolError> {
        let request = prepare(method, &args, self.config.max_pages_hard_cap)?;
        let tz = TimeZone::get("America/Los_Angeles").map_err(|e| ToolError::Store(e.to_string()))?;
        let now_zoned = Zoned::now().with_time_zone(tz);
        let today = now_zoned.date().to_string();
        let now = now_zoned.timestamp().as_second();
        let store = self.store.lock().map_err(|e| ToolError::Store(e.to_string()))?;
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

    async fn finish(&self, method: Method, args: Value) -> Result<CallToolResult, rmcp::ErrorData> {
        let this = self.clone();
        let outcome = tokio::task::spawn_blocking(move || this.run(method, args))
            .await
            .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
        match outcome {
            Ok(body) => Ok(CallToolResult::success(vec![ContentBlock::text(body.to_string())])),
            Err(err) => Ok(CallToolResult::error(vec![ContentBlock::text(err.to_string())])),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchArgs {
    /// Search words.
    q: Option<String>,
    part: Option<String>,
    #[serde(rename = "type")]
    type_: Option<String>,
    order: Option<String>,
    channel_id: Option<String>,
    published_after: Option<String>,
    published_before: Option<String>,
    video_duration: Option<String>,
    region_code: Option<String>,
    relevance_language: Option<String>,
    safe_search: Option<String>,
    page_token: Option<String>,
    max_results: Option<u32>,
    max_pages: Option<u32>,
    bypass_cache: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct VideosArgs {
    /// Comma-separated video ids.
    id: Option<String>,
    part: Option<String>,
    hl: Option<String>,
    max_results: Option<u32>,
    max_pages: Option<u32>,
    page_token: Option<String>,
    bypass_cache: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ChannelsArgs {
    id: Option<String>,
    for_handle: Option<String>,
    for_username: Option<String>,
    part: Option<String>,
    hl: Option<String>,
    max_results: Option<u32>,
    max_pages: Option<u32>,
    page_token: Option<String>,
    bypass_cache: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PlaylistsArgs {
    id: Option<String>,
    channel_id: Option<String>,
    part: Option<String>,
    hl: Option<String>,
    max_results: Option<u32>,
    max_pages: Option<u32>,
    page_token: Option<String>,
    bypass_cache: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PlaylistItemsArgs {
    playlist_id: Option<String>,
    video_id: Option<String>,
    part: Option<String>,
    max_results: Option<u32>,
    max_pages: Option<u32>,
    page_token: Option<String>,
    bypass_cache: Option<bool>,
}

#[tool_router]
impl YoutubeServer {
    #[tool(
        name = "youtube_search",
        description = "Find a public video, channel, or playlist by words. Do not use when you already have an id. Costs 1 search per live page."
    )]
    async fn youtube_search(&self, Parameters(args): Parameters<SearchArgs>) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Search, serde_json::to_value(args).unwrap_or(json!({}))).await
    }

    #[tool(
        name = "youtube_videos_list",
        description = "Details for known video ids. Do not pass max_results, max_pages, or page_token. Costs 1 non_search per live call."
    )]
    async fn youtube_videos_list(&self, Parameters(args): Parameters<VideosArgs>) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Videos, serde_json::to_value(args).unwrap_or(json!({}))).await
    }

    #[tool(
        name = "youtube_channels_list",
        description = "Channel metadata and the uploads playlist id. Pass id, for_handle, or for_username. Costs 1 non_search per live page."
    )]
    async fn youtube_channels_list(&self, Parameters(args): Parameters<ChannelsArgs>) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Channels, serde_json::to_value(args).unwrap_or(json!({}))).await
    }

    #[tool(
        name = "youtube_playlists_list",
        description = "Playlist metadata. Pass id or channel_id. Costs 1 non_search per live page."
    )]
    async fn youtube_playlists_list(&self, Parameters(args): Parameters<PlaylistsArgs>) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::Playlists, serde_json::to_value(args).unwrap_or(json!({}))).await
    }

    #[tool(
        name = "youtube_playlist_items_list",
        description = "Videos inside a playlist. Pass playlist_id. Costs 1 non_search per live page."
    )]
    async fn youtube_playlist_items_list(
        &self,
        Parameters(args): Parameters<PlaylistItemsArgs>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.finish(Method::PlaylistItems, serde_json::to_value(args).unwrap_or(json!({}))).await
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
```

`SearchArgs` and the other arg structs need `Serialize` as well as `Deserialize` so `serde_json::to_value` works. Add `serde::Serialize` to each derive.

`prepare` reads snake_case keys from that JSON (`q`, `channel_id`, `for_handle`, `max_pages`, `bypass_cache`). That matches Task 3.

`src/lib.rs` add `pub mod server;`

`rmcp` 3.8 `ServerConfig` has a public `instructions: Option<String>` field. `get_info` sets it from `prompts/server.md` read at startup. SQLite and the blocking HTTP client run inside `spawn_blocking` so they do not sit on the tokio worker.

- [ ] **Step 4: Run the test and the build**

Run: `cargo test prompts_name_the_rules -- --nocapture && cargo build`

Expected: test PASS and build finishes with no errors.

- [ ] **Step 5: Commit**

```bash
git add prompts src/server.rs src/lib.rs
git commit -m "$(cat <<'EOF'
Expose the five read-only YouTube tools over MCP.

EOF
)"
```

---

### Task 8: Process entry, Docker, and harness README

**Files:**
- Modify: `src/main.rs`
- Create: `Dockerfile`
- Modify: `.dockerignore`
- Create: `README.md`

**Interfaces:**
- Consumes: `Config::load`, `Store::open`, `ReqwestYoutube::new`, `YoutubeServer::new`
- Produces: a binary that speaks MCP on stdio and logs to stderr

- [ ] **Step 1: Write the failing test**

There is no socket test. Assert the binary's logging helper would target stderr by a pure function:

```rust
#[test]
fn default_log_filter_is_warn() {
    assert_eq!(crate::default_log_filter(), "warn");
}
```

Put `default_log_filter` in `src/lib.rs` for this test.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test default_log_filter_is_warn -- --nocapture`

Expected: FAIL to compile

- [ ] **Step 3: Implement main, Docker, and README**

`src/lib.rs`:

```rust
pub fn default_log_filter() -> &'static str {
    "warn"
}
```

`src/main.rs`:

```rust
use rmcp::ServiceExt;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use yt_rust_mcp::config::Config;
use yt_rust_mcp::server::YoutubeServer;
use yt_rust_mcp::store::Store;
use yt_rust_mcp::youtube::ReqwestYoutube;
use yt_rust_mcp::default_log_filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_log_filter())),
        )
        .init();

    let config_path = std::env::var_os("YT_RUST_MCP_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config/default.toml"));
    let config = Config::load(&config_path)?;
    let api_key = std::env::var("YOUTUBE_API_KEY")?;
    let store = Store::open(&config.db_path, config.sqlite_page_cache_kib)?;
    let http = ReqwestYoutube::new(api_key)?;
    let server = YoutubeServer::new(config, store, http);
    let running = server.serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
```

`YoutubeServer` must be `Clone` because `serve` consumes it. It already is.

`Dockerfile`:

```dockerfile
FROM rust:1.89-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=build /src/target/release/yt-rust-mcp /usr/local/bin/yt-rust-mcp
COPY config /app/config
COPY prompts /app/prompts
ENV YT_RUST_MCP_CONFIG=/app/config/default.toml
ENV YT_RUST_MCP_PROMPTS=/app/prompts
ENTRYPOINT ["yt-rust-mcp"]
```

`.dockerignore`:

```
target
.git
.env
```

`README.md`:

```markdown
# yt-rust-mcp

Read-only YouTube Data API v3 tools for an MCP harness. Stdio transport. API key only.

## Run

```bash
cp .env.example .env
# set YOUTUBE_API_KEY in the environment, not in git
export YOUTUBE_API_KEY=your-key
cargo run --release
```

## Harness

Command: `yt-rust-mcp`

The process speaks MCP on stdin/stdout. Logs go to stderr.

Set `YOUTUBE_API_KEY`. Optional: `YOUTUBE_SEARCH_BUDGET` (100), `YOUTUBE_NON_SEARCH_BUDGET` (10000), `YT_RUST_MCP_CACHE_DIR`.

## Docker

```bash
docker build -t yt-rust-mcp .
docker run --rm -i -e YOUTUBE_API_KEY yt-rust-mcp
```

## Tests

`cargo test` uses fixtures. It does not call YouTube.
```

- [ ] **Step 4: Run the test and the build**

Run: `cargo test --offline && cargo build`

If `--offline` fails because the registry is needed, run `cargo test` once online, then `cargo test` again.

Expected: all tests PASS and the binary builds.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/lib.rs Dockerfile .dockerignore README.md
git commit -m "$(cat <<'EOF'
Serve the MCP tools on stdio and log to stderr.

EOF
)"
```

---

## Self-review notes for the implementer

- Spec coverage: config and prompts (Tasks 1, 7, 8), error mapping (Task 2), cache key and videos.list refusal (Task 3), SQLite and both counters (Task 4), cache-before-debit and multi-page merge (Task 5), API key on the request only (Task 6), five tools and instructions (Task 7), Docker and stderr (Task 8).
- `youtube_videos_list` never inserts `maxResults` into the query. Task 3's test locks that.
- A live YouTube error returns after `debit`. No code path subtracts from `spent`.
- Do not add `transport-streamable-http-server`.
