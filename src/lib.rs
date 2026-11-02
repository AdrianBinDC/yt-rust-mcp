pub mod call;
pub mod config;
pub mod error;
pub mod http;
pub mod prompts;
pub mod server;
pub mod store;
pub mod transcript;
pub mod transport_opts;
pub mod youtube;

/// Default log filter is quiet (`warn`). Hot-path decision lines are `debug!` only.
///
/// Opt in: `RUST_LOG=yt_rust_mcp=debug` (stderr). Off unless set — never enable by
/// default. Debug logs are metadata only (tool, cache hit/miss, quota counter, ids).
/// They must not include API keys, URLs with tokens, queries, or transcript bodies.
pub fn default_log_filter() -> &'static str {
    "warn"
}

/// Message when `YOUTUBE_API_KEY` is missing after optional `.env` load.
pub fn missing_api_key_message() -> &'static str {
    "YOUTUBE_API_KEY is not set. Copy .env.example to .env, set YOUTUBE_API_KEY, and see docs/SETUP.md."
}

/// Read `YOUTUBE_API_KEY` from the environment (after dotenv, if used).
pub fn youtube_api_key() -> Result<String, String> {
    std::env::var("YOUTUBE_API_KEY").map_err(|_| missing_api_key_message().to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_log_filter_is_warn() {
        assert_eq!(crate::default_log_filter(), "warn");
    }

    #[test]
    fn missing_api_key_message_points_at_setup() {
        let msg = crate::missing_api_key_message();
        assert!(msg.contains(".env.example"));
        assert!(msg.contains("docs/SETUP.md"));
        assert!(msg.contains("YOUTUBE_API_KEY"));
    }
}
