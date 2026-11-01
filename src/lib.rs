pub mod call;
pub mod config;
pub mod error;
pub mod prompts;
pub mod server;
pub mod store;
pub mod transcript;
pub mod youtube;

/// Default log filter is quiet (`warn`). Hot-path decision lines are `debug!` only.
///
/// Opt in: `RUST_LOG=yt_rust_mcp=debug` (stderr). Off unless set — never enable by
/// default. Debug logs are metadata only (tool, cache hit/miss, quota counter, ids).
/// They must not include API keys, URLs with tokens, queries, or transcript bodies.
pub fn default_log_filter() -> &'static str {
    "warn"
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_log_filter_is_warn() {
        assert_eq!(crate::default_log_filter(), "warn");
    }
}
