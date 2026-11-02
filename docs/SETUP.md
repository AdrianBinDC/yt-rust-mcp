# Setup

Get yt-rust-mcp running after a clone. No published Docker image required.

## Steps

1. Clone the repo.
2. Install Rust if you do not have it (`https://rustup.rs`).
3. Create a Google API key and enable **YouTube Data API v3** on the project (Google Cloud → APIs & Services).
4. `cp .env.example .env`
5. Set `YOUTUBE_API_KEY=...` in `.env`.
6. Optional: add `ENABLE_MCP_TRANSCRIPT=1` in `.env` if you want the experimental transcript tool. Optional: `DEFAULT_TRANSCRIPT_LANGUAGE=en` so omitted `lang` prefers English captions.
7. From the repo root, run `cargo run --release` once to confirm it starts (Ctrl+C to stop). Or skip and wire the harness next.
8. Open **Cursor Settings → MCP**.
9. Add a server using the Cursor snippet below. Set `cwd` to your clone path.
10. Reload / restart the MCP server in Cursor.
11. Ask the agent something that needs YouTube tools. You should see the Data API tools (and `youtube_transcript` only if you enabled it).

If startup says `YOUTUBE_API_KEY is not set`, finish steps 4–5 (or export the key in the environment). The binary loads `.env` when that file exists; it does not create it for you.

## Cursor MCP snippet

Replace `/ABS/PATH/TO/yt-rust-mcp` with your clone path.

**stdio (dev):**

```json
{
  "mcpServers": {
    "youtube": {
      "command": "cargo",
      "args": ["run", "--release", "--quiet"],
      "cwd": "/ABS/PATH/TO/yt-rust-mcp"
    }
  }
}
```

**Shared HTTP** (one long-lived server for Cursor + other clients):

```json
{
  "mcpServers": {
    "youtube": {
      "url": "http://localhost:8087/mcp"
    }
  }
}
```

`cwd` must be the repo root so `config/` and `prompts/` resolve. The API key comes from `.env` (or from an `env` block you add). You do not need to put the key in this JSON when `.env` is set.

Optional in an `env` object on that server entry:

- `ENABLE_MCP_TRANSCRIPT`: `1` for the experimental transcript tool
- `DEFAULT_TRANSCRIPT_LANGUAGE`: e.g. `en` when the tool omits `lang`
- `RUST_LOG`: `yt_rust_mcp=debug` for metadata-only debug logs on stderr

## Other MCP clients

**stdio (dev):** working directory = repo root.

- **Command:** `cargo`
- **Args:** `run --release --quiet`
- **Working directory:** clone root

**Shared HTTP:** run one long-lived server and point every client at the same URL:

```text
http://localhost:8087/mcp
```

Example: `cargo run --release -- --http --bind 127.0.0.1:8087`  
Or Docker: `docker run --rm -p 127.0.0.1:8087:8087 --env-file .env yt-rust-mcp:local --http --bind 0.0.0.0:8087`

Use a short server name without hyphens in client config (e.g. `youtube`) so tool names stay simple.

Claude Desktop uses its own config file and JSON shape. Logs stay on stderr; stdio MCP traffic stays on stdout.

## Optional local Docker

No published image. From the repo root:

```bash
docker build -t yt-rust-mcp:local .
```

Prefer HTTP (`--http`) for a shared long-lived container. Short-lived stdio containers also work for a single Cursor client; a fixed container name races if two clients start at once. Pass secrets only with `--env-file` — never put `YOUTUBE_API_KEY` in the MCP JSON `env` block or as `-e YOUTUBE_API_KEY=...`. The image runs as non-root; set `--user` to your host `uid:gid` so the cache mount is writable.

```json
{
  "mcpServers": {
    "youtube": {
      "command": "docker",
      "args": [
        "run",
        "-i",
        "--rm",
        "--name",
        "yt-rust-mcp",
        "--env-file",
        "/ABS/PATH/TO/yt-rust-mcp/.env",
        "-e",
        "ENABLE_MCP_TRANSCRIPT=1",
        "-e",
        "RUST_LOG=yt_rust_mcp=debug",
        "-e",
        "YT_RUST_MCP_CACHE_DIR=/data",
        "-v",
        "/ABS/PATH/TO/yt-rust-mcp-cache:/data",
        "--user",
        "1000:1000",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges:true",
        "yt-rust-mcp:local"
      ]
    }
  }
}
```

Replace `1000:1000` with `id -u`:`id -g` on your machine. Do not copy `.env` into the image. Logs stay on stderr; open Cursor’s MCP server output to read `RUST_LOG` debug lines. The volume is only for the SQLite cache and daily quota counters.

## Environment

Set `YOUTUBE_API_KEY` (`.env` or the environment). Optional:

| Variable | Default | Meaning |
| --- | --- | --- |
| `YOUTUBE_SEARCH_BUDGET` | 100 | Daily search-call budget |
| `YOUTUBE_NON_SEARCH_BUDGET` | 10000 | Daily non-search budget |
| `YT_RUST_MCP_CACHE_DIR` | OS cache dir | Where the SQLite cache and quota counters live |
| `YT_RUST_MCP_CONFIG` | `config/default.toml` | Config file. Startup fails if unreadable |
| `YT_RUST_MCP_PROMPTS` | `prompts` | Directory holding `server.yaml`. Startup fails if missing |
| `ENABLE_MCP_TRANSCRIPT` | off | Set `1`/`true` to register experimental `youtube_transcript` |
| `DEFAULT_TRANSCRIPT_LANGUAGE` | unset | Prefer this caption language when the tool omits `lang` |
| `RUST_LOG` | `warn` | Opt-in debug: `yt_rust_mcp=debug` (stderr; no secrets/bodies) |

Path defaults are relative to the process working directory. Prefer repo root as `cwd`.

## Tests

`cargo test` uses fixtures. It does not call YouTube.
