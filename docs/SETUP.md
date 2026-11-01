# Setup

Get yt-rust-mcp running after a clone. No published Docker image required.

## Steps

1. Clone the repo.
2. Install Rust if you do not have it (`https://rustup.rs`).
3. Create a Google API key and enable **YouTube Data API v3** on the project (Google Cloud → APIs & Services).
4. `cp .env.example .env`
5. Set `YOUTUBE_API_KEY=...` in `.env`.
6. Optional: add `ENABLE_MCP_TRANSCRIPT=1` in `.env` if you want the experimental transcript tool.
7. From the repo root, run `cargo run --release` once to confirm it starts (Ctrl+C to stop). Or skip and wire the harness next.
8. Open **Cursor Settings → MCP**.
9. Add a server using the Cursor snippet below. Set `cwd` to your clone path.
10. Reload / restart the MCP server in Cursor.
11. Ask the agent something that needs YouTube tools. You should see the Data API tools (and `youtube_transcript` only if you enabled it).

If startup says `YOUTUBE_API_KEY is not set`, finish steps 4–5 (or export the key in the environment). The binary loads `.env` when that file exists; it does not create it for you.

## Cursor MCP snippet

Replace `/ABS/PATH/TO/yt-rust-mcp` with your clone path.

```json
{
  "mcpServers": {
    "yt-rust-mcp": {
      "command": "cargo",
      "args": ["run", "--release", "--quiet"],
      "cwd": "/ABS/PATH/TO/yt-rust-mcp"
    }
  }
}
```

`cwd` must be the repo root so `config/` and `prompts/` resolve. The API key comes from `.env` (or from an `env` block you add). You do not need to put the key in this JSON when `.env` is set.

Optional in an `env` object on that server entry:

- `ENABLE_MCP_TRANSCRIPT`: `1` for the experimental transcript tool
- `RUST_LOG`: `yt_rust_mcp=debug` for metadata-only debug logs on stderr

## Other MCP clients

Same idea: run stdio with working directory = repo root.

- **Command:** `cargo`
- **Args:** `run --release --quiet`
- **Working directory:** clone root

Claude Desktop uses its own config file and JSON shape; point it at the same command and directory. Logs stay on stderr; MCP traffic stays on stdout.

## Optional local Docker

You can `docker build` locally from the Dockerfile. That is optional and not required for setup above.
