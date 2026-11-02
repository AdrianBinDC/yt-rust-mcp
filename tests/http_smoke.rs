//! Starts the real binary with `--http`, checks `/health` and MCP initialize.
//! No network to YouTube: initialize never calls Google.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn spawn_http(port: u16) -> (Child, std::path::PathBuf) {
    let cache = std::env::temp_dir().join(format!(
        "yt-http-smoke-{}-{}",
        std::process::id(),
        port
    ));
    let child = Command::new(env!("CARGO_BIN_EXE_yt-rust-mcp"))
        .arg("--http")
        .arg("--bind")
        .arg(format!("127.0.0.1:{port}"))
        .env("YOUTUBE_API_KEY", "dummy-key")
        .env(
            "YT_RUST_MCP_CONFIG",
            concat!(env!("CARGO_MANIFEST_DIR"), "/config/default.toml"),
        )
        .env(
            "YT_RUST_MCP_PROMPTS",
            concat!(env!("CARGO_MANIFEST_DIR"), "/prompts"),
        )
        .env("YT_RUST_MCP_CACHE_DIR", &cache)
        .env_remove("ENABLE_MCP_TRANSCRIPT")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    (child, cache)
}

fn wait_health(port: u16, timeout: Duration) {
    let url = format!("http://127.0.0.1:{port}/health");
    let start = Instant::now();
    loop {
        if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let req = format!("GET /health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
            if stream.write_all(req.as_bytes()).is_ok() {
                let mut buf = String::new();
                let _ = stream.read_to_string(&mut buf);
                if buf.contains("200") && buf.contains(r#"{"ok":true}"#) {
                    return;
                }
            }
        }
        if start.elapsed() > timeout {
            panic!("server did not become healthy on {url}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn http_post_json(port: u16, path: &str, body: &str) -> String {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut buf = String::new();
    stream.read_to_string(&mut buf).unwrap();
    buf
}

#[test]
fn http_health_and_initialize() {
    let port = free_port();
    let (mut child, _cache) = spawn_http(port);
    let stderr = child.stderr.take();

    let result = std::panic::catch_unwind(|| {
        wait_health(port, Duration::from_secs(15));

        let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"http-smoke","version":"0"}}}"#;
        let resp = http_post_json(port, "/mcp", init);
        assert!(
            resp.contains("yt-rust-mcp") || resp.contains("serverInfo") || resp.contains("\"id\":1"),
            "unexpected initialize response: {resp}"
        );
    });

    let _ = child.kill();
    let _ = child.wait();
    if let Some(mut err) = stderr {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s);
        if result.is_err() {
            eprintln!("server stderr:\n{s}");
        }
    }
    result.unwrap();
}
