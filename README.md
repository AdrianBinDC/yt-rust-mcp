# yt-rust-mcp

[![CI](https://github.com/AdrianBinDC/yt-rust-mcp/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/AdrianBinDC/yt-rust-mcp/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust 1.89+](https://img.shields.io/badge/rust-1.89%2B-blue.svg)](https://www.rust-lang.org)
[![MCP stdio](https://img.shields.io/badge/MCP-stdio-informational.svg)](./docs/SETUP.md)

Unofficial, not affiliated with Google or YouTube.

Read-only YouTube Data API v3 tools for an MCP harness. Stdio transport. API key only.

**Setup:** follow [`docs/SETUP.md`](./docs/SETUP.md) (clone → `.env` → Cursor MCP).

Optional experimental tool: `youtube_transcript` (off by default; set `ENABLE_MCP_TRANSCRIPT=1`).

## Run

```bash
cp .env.example .env
# Set YOUTUBE_API_KEY in .env (the binary loads .env if present).
cargo run --release
```

## Harness

Command: `yt-rust-mcp` (or `cargo run --release` with `cwd` at the repo root).

The process speaks MCP on stdin/stdout. Logs go to stderr.

Set `YOUTUBE_API_KEY` (`.env` or the environment). Optional:

| Variable | Default | Meaning |
| --- | --- | --- |
| `YOUTUBE_SEARCH_BUDGET` | 100 | Daily search-call budget |
| `YOUTUBE_NON_SEARCH_BUDGET` | 10000 | Daily non-search budget |
| `YT_RUST_MCP_CACHE_DIR` | OS cache dir | Where the SQLite cache and quota counters live |
| `YT_RUST_MCP_CONFIG` | `config/default.toml` | Config file. Startup fails if unreadable |
| `YT_RUST_MCP_PROMPTS` | `prompts` | Directory holding `server.yaml`. Startup fails if missing |
| `ENABLE_MCP_TRANSCRIPT` | off | Set `1`/`true` to register experimental `youtube_transcript` |
| `RUST_LOG` | `warn` | Opt-in debug: `yt_rust_mcp=debug` (stderr; no secrets/bodies) |

Path defaults are relative to the process working directory. Prefer repo root as `cwd`.

## Tests

`cargo test` uses fixtures. It does not call YouTube.
