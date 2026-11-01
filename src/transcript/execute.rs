use crate::error::ToolError;
use crate::store::Store;
use crate::transcript::model::{cache_key, parse_format, validate_video_id, Cue, Format};
use crate::transcript::parse::parse_timedtext;
use crate::transcript::render::render;
use crate::transcript::select::{select_track, timedtext_url};
use serde_json::{json, Value};

/// Network seam so tests can run on fixtures. The live impl is `ReqwestTranscript`.
pub trait TranscriptHttp {
    fn player(&self, video_id: &str) -> Result<Value, ToolError>;
    fn timedtext(&self, url: &str) -> Result<String, ToolError>;
}

/// Cache-first transcript fetch. Cues are cached once per (video, lang);
/// the output format is applied on read, so text/srt/vtt share one live fetch.
#[allow(clippy::too_many_arguments)]
pub fn execute(
    store: &Store,
    http: &impl TranscriptHttp,
    video_id: &str,
    lang: Option<&str>,
    format: Option<&str>,
    bypass_cache: bool,
    ttl_seconds: i64,
    budget: i64,
    now: i64,
    today: &str,
    max_entries: i64,
) -> Result<Value, ToolError> {
    validate_video_id(video_id)?;
    let format = parse_format(format)?;
    let key = cache_key(video_id, lang);
    let cached = if bypass_cache {
        None
    } else {
        store.get_fresh(&key, now)?
    };
    let (saved, cached_flag) = if let Some(body) = cached {
        tracing::debug!(
            video_id,
            lang,
            format = format.as_str(),
            bypass_cache,
            cache_hit = true,
            "transcript"
        );
        (
            serde_json::from_str(&body).map_err(|e| ToolError::Store(e.to_string()))?,
            true,
        )
    } else {
        // Only live fetches spend budget; cache hits are free.
        if !store.can_spend("transcript", 1, budget, today)? {
            let remaining = store.remaining("transcript", budget, today)?;
            return Err(ToolError::QuotaExhausted {
                counter: "transcript",
                remaining,
                cost: 1,
            });
        }
        // Debit before the network call: a failed attempt still hits YouTube.
        store.debit("transcript", 1, budget, today)?;
        tracing::debug!(
            video_id,
            lang,
            format = format.as_str(),
            bypass_cache,
            cache_hit = false,
            counter = "transcript",
            "transcript"
        );
        let player = http.player(video_id)?;
        let track = select_track(&player, lang)?;
        let url = timedtext_url(&track.base_url)?;
        let body = http.timedtext(&url)?;
        let cues = parse_timedtext(&body)?;
        let saved = json!({
            "video_id": video_id,
            "language_code": track.language_code,
            "kind": track.kind,
            "cues": cues.iter().map(|cue| json!({
                "start_ms": cue.start_ms,
                "duration_ms": cue.duration_ms,
                "text": cue.text,
            })).collect::<Vec<_>>(),
        });
        // Errors above return early, so failures are never cached.
        store.put(
            &key,
            &saved.to_string(),
            now + ttl_seconds,
            now,
            max_entries,
        )?;
        (saved, false)
    };
    response(video_id, format, &saved, cached_flag)
}

fn response(
    video_id: &str,
    format: Format,
    saved: &Value,
    cached: bool,
) -> Result<Value, ToolError> {
    let cues = read_cues(saved.get("cues"))?;
    let rendered = render(format, &cues);
    let mut out = json!({
        "video_id": video_id,
        "language_code": saved["language_code"],
        "kind": saved["kind"],
        "format": format.as_str(),
        "cached": cached,
    });
    match format {
        Format::Text => out["text"] = Value::String(rendered),
        Format::Srt | Format::Vtt => out["body"] = Value::String(rendered),
    }
    Ok(out)
}

fn read_cues(value: Option<&Value>) -> Result<Vec<Cue>, ToolError> {
    let rows = value
        .and_then(Value::as_array)
        .ok_or(ToolError::EmptyTranscript)?;
    let mut cues = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        // A malformed cached row means the cache is corrupt: surface it, don't drop it.
        let bad =
            |field: &str| ToolError::Store(format!("cached cue {i} missing or invalid {field}"));
        cues.push(Cue {
            start_ms: row
                .get("start_ms")
                .and_then(Value::as_u64)
                .ok_or_else(|| bad("start_ms"))?,
            duration_ms: row
                .get("duration_ms")
                .and_then(Value::as_u64)
                .ok_or_else(|| bad("duration_ms"))?,
            text: row
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("text"))?
                .to_string(),
        });
    }
    if cues.is_empty() {
        return Err(ToolError::EmptyTranscript);
    }
    Ok(cues)
}

pub const ANDROID_CLIENT_VERSION: &str = "21.03.36";
// Public InnerTube key embedded in the YouTube Android app, not a user secret.
const INNERTUBE_API_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";
const ANDROID_USER_AGENT: &str =
    "com.google.android.youtube/21.03.36 (Linux; U; Android 16; en_US) gzip";

/// Live client. Not exercised by `cargo test`.
pub struct ReqwestTranscript {
    client: reqwest::blocking::Client,
}

impl ReqwestTranscript {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent(ANDROID_USER_AGENT)
            .build()
            .expect("reqwest client");
        Self { client }
    }
}

/// `reqwest::blocking::Client` panics if built or dropped on an async worker
/// thread, so async callers (main) must build it here. Mirrors `youtube::build_http`.
pub async fn build_transcript_http() -> Result<ReqwestTranscript, ToolError> {
    tokio::task::spawn_blocking(ReqwestTranscript::new)
        .await
        .map_err(|e| ToolError::Http(format!("transcript client build task failed: {e}")))
}

impl Default for ReqwestTranscript {
    fn default() -> Self {
        Self::new()
    }
}

impl TranscriptHttp for ReqwestTranscript {
    fn player(&self, video_id: &str) -> Result<Value, ToolError> {
        // Never log or echo `url`; error strings below omit it.
        let url = format!(
            "https://www.youtube.com/youtubei/v1/player?key={INNERTUBE_API_KEY}&prettyPrint=false"
        );
        let body = json!({
            "context": {
                "client": {
                    "clientName": "ANDROID",
                    "clientVersion": ANDROID_CLIENT_VERSION,
                    "androidSdkVersion": 36,
                    "hl": "en",
                    "gl": "US"
                }
            },
            "videoId": video_id
        });
        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .map_err(|err| ToolError::Http(err.without_url().to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::Http(format!("player HTTP {}", status.as_u16())));
        }
        response
            .json()
            .map_err(|err| ToolError::Http(err.without_url().to_string()))
    }

    fn timedtext(&self, url: &str) -> Result<String, ToolError> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|err| ToolError::Http(err.without_url().to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::Http(format!(
                "timedtext HTTP {}",
                status.as_u16()
            )));
        }
        response
            .text()
            .map_err(|err| ToolError::Http(err.without_url().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ToolError;
    use crate::store::Store;
    use crate::transcript::model::cache_key;
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeHttp {
        calls: AtomicUsize,
    }

    impl TranscriptHttp for FakeHttp {
        fn player(&self, video_id: &str) -> Result<Value, ToolError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(video_id, "abcdefghijk");
            Ok(serde_json::from_str(include_str!(
                "../../fixtures/youtube/transcript.player.ok.json"
            ))
            .unwrap())
        }

        fn timedtext(&self, url: &str) -> Result<String, ToolError> {
            assert!(url.contains("fmt=json3"));
            assert!(!url.contains("fmt=srv3"));
            Ok(include_str!("../../fixtures/youtube/transcript.timed.json").to_string())
        }
    }

    fn store() -> Store {
        let path = std::env::temp_dir().join(format!(
            "yt-transcript-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Store::open(&path, 2048).unwrap()
    }

    #[test]
    fn second_call_for_the_other_format_does_not_fetch_again() {
        let store = store();
        let http = FakeHttp {
            calls: AtomicUsize::new(0),
        };
        let first = execute(
            &store,
            &http,
            "abcdefghijk",
            None,
            Some("text"),
            false,
            86_400,
            200,
            1_000,
            "2026-11-01",
            10_000,
        )
        .unwrap();
        let second = execute(
            &store,
            &http,
            "abcdefghijk",
            None,
            Some("srt"),
            false,
            86_400,
            200,
            1_100,
            "2026-11-01",
            10_000,
        )
        .unwrap();
        assert_eq!(first["format"], "text");
        assert_eq!(first["text"], "hello world\nsecond line\n");
        assert_eq!(first["cached"], false);
        assert_eq!(second["format"], "srt");
        assert!(second["body"].as_str().unwrap().contains("00:00:01,200"));
        assert_eq!(second["cached"], true);
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
        assert!(store
            .can_spend("transcript", 199, 200, "2026-11-01")
            .unwrap());
        assert!(!store
            .can_spend("transcript", 200, 200, "2026-11-01")
            .unwrap());
    }

    #[test]
    fn bypass_cache_fetches_again() {
        let store = store();
        let http = FakeHttp {
            calls: AtomicUsize::new(0),
        };
        execute(
            &store,
            &http,
            "abcdefghijk",
            None,
            None,
            false,
            86_400,
            200,
            1_000,
            "2026-11-01",
            10_000,
        )
        .unwrap();
        execute(
            &store,
            &http,
            "abcdefghijk",
            None,
            None,
            true,
            86_400,
            200,
            2_000,
            "2026-11-01",
            10_000,
        )
        .unwrap();
        assert_eq!(http.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn browser_token_error_is_not_cached() {
        let store = store();
        let err = execute(
            &store,
            &TokenHttp,
            "abcdefghijk",
            None,
            None,
            false,
            86_400,
            200,
            1_000,
            "2026-11-01",
            10_000,
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::BrowserTokenRequired));
        let key = cache_key("abcdefghijk", None);
        assert!(store.get_fresh(&key, 1_000).unwrap().is_none());
    }

    #[test]
    fn exhausted_budget_returns_quota_error_without_calling_youtube() {
        let store = store();
        let http = FakeHttp {
            calls: AtomicUsize::new(0),
        };
        let err = execute(
            &store,
            &http,
            "abcdefghijk",
            None,
            None,
            false,
            86_400,
            0,
            1_000,
            "2026-11-01",
            10_000,
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::QuotaExhausted { .. }));
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn corrupt_cached_cue_is_a_store_error() {
        let rows = serde_json::json!([{"start_ms": 0, "duration_ms": 10}]);
        let err = read_cues(Some(&rows)).unwrap_err();
        assert!(matches!(err, ToolError::Store(_)));
    }

    struct TokenHttp;

    impl TranscriptHttp for TokenHttp {
        fn player(&self, _video_id: &str) -> Result<Value, ToolError> {
            Ok(serde_json::from_str(include_str!(
                "../../fixtures/youtube/transcript.player.token.json"
            ))
            .unwrap())
        }

        fn timedtext(&self, _url: &str) -> Result<String, ToolError> {
            panic!("timedtext must not be called when a browser token is required");
        }
    }
}
