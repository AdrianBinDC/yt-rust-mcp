//! Streamable HTTP transport: `GET /health` and MCP at `/mcp`.

use std::sync::Arc;

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tracing::info;

use crate::server::YoutubeServer;

/// Build `/health` plus Streamable HTTP `/mcp` (no bearer auth).
pub fn build_router(server: YoutubeServer) -> Router {
    let server = Arc::new(server);
    let factory = move || Ok(server.as_ref().clone());
    let mcp_service = StreamableHttpService::new(
        factory,
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    Router::new()
        .route("/health", get(health))
        .nest_service("/mcp", mcp_service)
}

/// Serve Streamable HTTP at `/mcp` plus `GET /health`.
pub async fn run_http(
    bind: &str,
    server: YoutubeServer,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr: std::net::SocketAddr = bind
        .parse()
        .map_err(|e| format!("invalid bind address '{bind}': {e}"))?;
    let app = build_router(server);
    info!(%addr, "http transport starting");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, r#"{"ok":true}"#)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::store::Store;
    use crate::youtube::ReqwestYoutube;
    use http::Request;
    use std::path::PathBuf;
    use tower::ServiceExt;

    fn test_server() -> YoutubeServer {
        let db = std::env::temp_dir().join(format!(
            "yt-http-health-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/default.toml");
        let mut config = Config::load(&config_path).unwrap();
        config.db_path = db;
        config.enable_transcript = false;
        let store = Store::open(&config.db_path, config.sqlite_page_cache_kib).unwrap();
        let http = ReqwestYoutube::new("dummy".into()).unwrap();
        YoutubeServer::new(config, store, http, None, "test".into())
    }

    #[tokio::test]
    async fn health_returns_200_json() {
        // blocking reqwest client must not be built on a tokio worker thread
        let server = tokio::task::spawn_blocking(test_server).await.unwrap();
        let app = build_router(server);
        let req = Request::builder()
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], br#"{"ok":true}"#);
    }
}
