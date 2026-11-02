//! Binary entry: stdio MCP by default, or Streamable HTTP with `--http`.

use clap::Parser;
use rmcp::ServiceExt;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use yt_rust_mcp::config::Config;
use yt_rust_mcp::default_log_filter;
use yt_rust_mcp::http::run_http;
use yt_rust_mcp::server::YoutubeServer;
use yt_rust_mcp::store::Store;
use yt_rust_mcp::transcript::execute::build_transcript_http;
use yt_rust_mcp::transport_opts::{resolve_transport, TransportMode};
use yt_rust_mcp::youtube::build_http;
use yt_rust_mcp::youtube_api_key;

/// Read-only YouTube Data API v3 MCP server.
#[derive(Parser, Debug)]
#[command(name = "yt-rust-mcp", version, about)]
struct Cli {
    /// Use Streamable HTTP transport (default: stdio).
    #[arg(long)]
    http: bool,

    /// Address to bind for --http (default 127.0.0.1:8087).
    #[arg(long)]
    bind: Option<String>,

    /// Use stdio transport (default; accepted for symmetry with --http).
    #[arg(long)]
    stdio: bool,
}

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

    let cli = Cli::parse();
    let mode = resolve_transport(cli.http, cli.stdio, cli.bind);

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

    match mode {
        TransportMode::Http { bind } => run_http(&bind, server).await?,
        TransportMode::Stdio => {
            let running = server.serve(rmcp::transport::stdio()).await?;
            running.waiting().await?;
        }
    }
    Ok(())
}
