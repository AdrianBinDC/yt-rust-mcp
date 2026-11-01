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

Set `YOUTUBE_API_KEY`. Optional: `YOUTUBE_SEARCH_BUDGET` (100), `YOUTUBE_NON_SEARCH_BUDGET` (10000), `YT_RUST_MCP_CACHE_DIR`.

## Docker

```bash
docker build -t yt-rust-mcp .
docker run --rm -i -e YOUTUBE_API_KEY yt-rust-mcp
```

## Tests

`cargo test` uses fixtures. It does not call YouTube.
