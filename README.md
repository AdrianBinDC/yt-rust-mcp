# yt-rust-mcp

Read-only YouTube Data API v3 tools for an MCP harness. Stdio transport. API key only.

## Run

```bash
cp .env.example .env
# Set YOUTUBE_API_KEY in the environment, not in git.
export YOUTUBE_API_KEY=your-key
cargo run --release
```

## Harness

Command: `yt-rust-mcp`

The process speaks MCP on stdin/stdout. Logs go to stderr.

Set `YOUTUBE_API_KEY`. Optional:

| Variable | Default | Meaning |
| --- | --- | --- |
| `YOUTUBE_SEARCH_BUDGET` | 100 | Daily search-call budget |
| `YOUTUBE_NON_SEARCH_BUDGET` | 10000 | Daily non-search budget |
| `YT_RUST_MCP_CACHE_DIR` | OS cache dir | Where the SQLite cache and quota counters live |
| `YT_RUST_MCP_CONFIG` | `config/default.toml` | Config file. Startup fails if unreadable |
| `YT_RUST_MCP_PROMPTS` | `prompts` | Directory holding `server.md`. Startup fails if missing |

The two path defaults are relative to the process's working directory. Run from
the repo root, or set both to absolute paths (the Docker image does).

## Docker

```bash
docker build -t yt-rust-mcp .
docker run --rm -i -e YOUTUBE_API_KEY \
  -v yt-rust-mcp-cache:/cache -e YT_RUST_MCP_CACHE_DIR=/cache yt-rust-mcp
```

Without the volume, the cache and daily quota counters are lost when the container exits.

## Tests

`cargo test` uses fixtures. It does not call YouTube.
