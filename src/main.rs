use rmcp::ServiceExt;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use yt_rust_mcp::config::Config;
use yt_rust_mcp::default_log_filter;
use yt_rust_mcp::server::YoutubeServer;
use yt_rust_mcp::store::Store;
use yt_rust_mcp::youtube::ReqwestYoutube;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let api_key = std::env::var("YOUTUBE_API_KEY")?;
    let store = Store::open(&config.db_path, config.sqlite_page_cache_kib)?;
    let http = ReqwestYoutube::new(api_key)?;
    let server = YoutubeServer::new(config, store, http);
    let running = server.serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
