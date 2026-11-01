use rmcp::ServiceExt;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use yt_rust_mcp::config::Config;
use yt_rust_mcp::default_log_filter;
use yt_rust_mcp::server::YoutubeServer;
use yt_rust_mcp::store::Store;
use yt_rust_mcp::transcript::execute::build_transcript_http;
use yt_rust_mcp::youtube::build_http;
use yt_rust_mcp::youtube_api_key;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load `.env` if present; missing file is fine. Does not override existing env.
    let _ = dotenvy::dotenv();

    // Logs → stderr only (stdio MCP uses stdout). Default warn; see default_log_filter.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(default_log_filter())),
        )
        .init();

    let config_path = std::env::var_os("YT_RUST_MCP_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config/default.toml"));
    let config = Config::load(&config_path)?;
    let api_key = youtube_api_key().map_err(|msg| -> Box<dyn std::error::Error> { msg.into() })?;
    let store = Store::open(&config.db_path, config.sqlite_page_cache_kib)?;
    let instructions = config.load_instructions()?;
    let http = build_http(api_key).await?;
    let transcript_http = if config.enable_transcript {
        Some(build_transcript_http().await?)
    } else {
        None
    };
    let server = YoutubeServer::new(config, store, http, transcript_http, instructions);
    let running = server.serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
