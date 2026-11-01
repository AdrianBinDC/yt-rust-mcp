//! Starts the real binary with a dummy key and talks MCP over stdio.
//! No network: initialize and tools/list never call YouTube.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_yt-rust-mcp"));
    cmd.env("YOUTUBE_API_KEY", "dummy-key")
        .env(
            "YT_RUST_MCP_CONFIG",
            concat!(env!("CARGO_MANIFEST_DIR"), "/config/default.toml"),
        )
        .env(
            "YT_RUST_MCP_PROMPTS",
            concat!(env!("CARGO_MANIFEST_DIR"), "/prompts"),
        );
    cmd
}

#[test]
fn starts_and_answers_initialize_and_tools_list() {
    let cache = std::env::temp_dir().join(format!("yt-smoke-{}", std::process::id()));
    let mut child = bin()
        .env("YT_RUST_MCP_CACHE_DIR", &cache)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    let (tx, rx) = mpsc::channel::<serde_json::Value>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str(&line) {
                let _ = tx.send(v);
            }
        }
    });

    for msg in [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
    ] {
        writeln!(stdin, "{msg}").unwrap();
    }
    stdin.flush().unwrap();

    let mut init = None;
    let mut tools = None;
    while init.is_none() || tools.is_none() {
        let v = rx
            .recv_timeout(Duration::from_secs(20))
            .expect("no reply from server");
        match v["id"].as_i64() {
            Some(1) => init = Some(v),
            Some(2) => tools = Some(v),
            _ => {}
        }
    }
    let instructions = init.unwrap()["result"]["instructions"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(instructions.contains("Read-only"));
    let tools = tools.unwrap();
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 6, "{names:?}");
    assert!(names.contains(&"youtube_transcript"), "{names:?}");
    let schema = tools["result"]["tools"].to_string();
    assert!(!schema.contains("forMine"));

    // Closing stdin must end the server cleanly (no panic while dropping the client).
    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success(), "server exited with {status}");
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn missing_prompts_file_fails_loudly() {
    let out = bin()
        .env("YT_RUST_MCP_PROMPTS", "/nonexistent-prompts-dir")
        .env(
            "YT_RUST_MCP_CACHE_DIR",
            std::env::temp_dir().join(format!("yt-smoke-np-{}", std::process::id())),
        )
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("server.md"), "stderr: {err}");
}
