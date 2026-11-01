use crate::config::TtlSeconds;
use crate::error::ToolError;
use crate::store::Store;
use crate::youtube::{cache_key, ttl_seconds, CallRequest, Method, YoutubeHttp};
use serde_json::{json, Value};

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
    let mut merged_body = None;
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
                    .and_then(Value::as_u64)
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

        if merged_body.is_none() {
            merged_body = Some(body.clone());
        }
        if let Some(items) = body.get("items").and_then(Value::as_array) {
            merged_items.extend(items.clone());
        }
        carry_token = body
            .get("nextPageToken")
            .and_then(Value::as_str)
            .map(str::to_string);
        if carry_token.is_none() {
            break;
        }
    }

    let mut out = merged_body.unwrap_or_else(|| json!({}));
    let object = out
        .as_object_mut()
        .ok_or_else(|| ToolError::Http("YouTube response body was not an object".to_string()))?;
    object.insert("items".to_string(), json!(merged_items));
    object.insert("cached".to_string(), json!(all_cached));
    if let Some(token) = carry_token {
        object.insert("nextPageToken".to_string(), json!(token));
        object.insert(
            "quota_note".to_string(),
            json!("Another page costs another call. Pass page_token or raise max_pages."),
        );
    } else {
        object.remove("nextPageToken");
        object.remove("quota_note");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TtlSeconds;
    use crate::store::Store;
    use crate::youtube::{prepare, Method, YoutubeHttp};
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    struct Fake {
        hits: Mutex<Vec<String>>,
    }

    impl YoutubeHttp for Fake {
        fn get(
            &self,
            method: Method,
            query: &BTreeMap<String, String>,
        ) -> Result<serde_json::Value, crate::error::ToolError> {
            let token = query.get("pageToken").map(String::as_str).unwrap_or("");
            self.hits
                .lock()
                .unwrap()
                .push(format!("{}:{token}", method.api_name()));
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
        TtlSeconds {
            search: 3600,
            videos: 86400,
            channels: 604800,
            playlists: 604800,
            playlist_items: 86400,
        }
    }

    #[test]
    fn second_identical_search_is_cached_and_does_not_debit_again() {
        let path = std::env::temp_dir().join(format!("yt-call-{}.db", std::process::id()));
        let store = Store::open(&path, 2048).unwrap();
        let http = Fake {
            hits: Mutex::new(vec![]),
        };
        let request = prepare(Method::Search, &json!({"q": "rust"}), 5).unwrap();
        let first = execute(
            &store,
            &http,
            request.clone(),
            &ttl(),
            100,
            10_000,
            1_000,
            "2026-11-01",
            10_000,
        )
        .unwrap();
        assert_eq!(first["cached"], false);
        assert_eq!(first["kind"], "youtube#searchListResponse");
        let second = execute(
            &store,
            &http,
            request,
            &ttl(),
            100,
            10_000,
            1_100,
            "2026-11-01",
            10_000,
        )
        .unwrap();
        assert_eq!(second["cached"], true);
        assert_eq!(second["kind"], "youtube#searchListResponse");
        assert_eq!(http.hits.lock().unwrap().len(), 1);
        assert!(store.can_spend("search", 99, 100, "2026-11-01").unwrap());
    }

    #[test]
    fn two_pages_concatenate_items_and_debit_twice() {
        let path = std::env::temp_dir().join(format!("yt-pages-{}.db", std::process::id()));
        let store = Store::open(&path, 2048).unwrap();
        let http = Fake {
            hits: Mutex::new(vec![]),
        };
        let request = prepare(Method::Search, &json!({"q": "rust", "max_pages": 2}), 5).unwrap();
        let body = execute(
            &store,
            &http,
            request,
            &ttl(),
            100,
            10_000,
            1_000,
            "2026-11-01",
            10_000,
        )
        .unwrap();
        assert_eq!(body["items"].as_array().unwrap().len(), 2);
        assert!(body.get("nextPageToken").is_none());
        assert_eq!(body["cached"], false);
        assert!(store.can_spend("search", 98, 100, "2026-11-01").unwrap());
        assert!(!store.can_spend("search", 99, 100, "2026-11-01").unwrap());
    }
}
