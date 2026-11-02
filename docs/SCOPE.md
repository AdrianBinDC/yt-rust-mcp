# SCOPE — yt-rust-mcp v1

This file is the contract for humans and coding agents. v1 of this contract is implemented. If a change is not allowed here, do not add it.

## One-line product

A **lightweight Rust MCP server** that exposes **read-only** YouTube Data API v3 calls so agent harnesses can **discover and inspect** public YouTube videos, channels, and playlists. This project is **unofficial** and is **not affiliated with Google or YouTube**.

## In scope (v1)

| MCP concern | API method | Purpose |
|---|---|---|
| Search | `search.list` | Find videos / channels / playlists by query |
| Video details | `videos.list` | Metadata + stats for known video IDs |
| Channel details | `channels.list` | Channel metadata; uploads playlist id |
| Playlists | `playlists.list` | Playlist metadata |
| Playlist videos | `playlistItems.list` | Items inside a playlist |

**Deferred (not v1):** `videoCategories.list`, `i18nLanguages.list`, `i18nRegions.list` — useful as label decoders only; core discoverability works without them.

Auth: query parameter `key` from env **`YOUTUBE_API_KEY`**.

If a tool argument requires OAuth or content-owner credentials (`mine`, `forMine`, `myRating`, `mySubscribers`, `managedByMe`, `forContentOwner`, `forDeveloper`, `onBehalfOfContentOwner`, `onBehalfOfContentOwnerChannel`), **refuse** the call. Do not strip the argument and continue.

The v1 tools are the catalog methods with `"v1_status": "in_scope"`. Entries marked `"deferred"` are reference, not tools.

Transport: **stdio** (default, harness/dev) and **Streamable HTTP** (`/mcp` + `/health`) for a long-lived shared server (Cursor + Hermes on one container).

## Explicitly out of scope (v1)

State this in server instructions and tool descriptions so agents do not attempt it:

- Any **write**: insert / update / delete / upload / rate / moderate
- **OAuth** / private user data
- **Captions** download via Data API (owner-only; not public “Show transcript”)
- **Comments**, **subscriptions**, **live** streaming/chat
- **Members** / Super Chat
- **YouTube Analytics** / Reporting APIs
- Media download (yt-dlp) and watch-page HTML scraping

**Transcripts (experimental):** `youtube_transcript` is off unless `ENABLE_MCP_TRANSCRIPT` is `1`/`true`. When enabled, it returns YouTube's public “Show transcript” track. It is **experimental**, **unsupported**, and **undocumented** — not part of the stable Data API tool surface. It can break when YouTube changes. Cached for one day; spends a local `transcript` counter. It is not Data API `captions.download` and it does not use `YOUTUBE_API_KEY`. Server instructions are built from `prompts/server.yaml` so discovery and copy stay in sync with the flag.

## Agent-harness discoverability rules

- Server `instructions` must say: **read-only**, **API key**, **public data**, and list the tools.
- Every tool description must include: **when to use**, **when not to use**, and a **quota hint**.
- Prefer cheap ID-based reads (`videos.list`, `playlistItems.list`, `channels.list`) over `search.list` when the agent already has an ID.
- Tool names should be obvious and stable (e.g. `youtube_search`, `youtube_videos_list`, `youtube_playlist_items_list`).

## Product posture

**Lean, fast, efficient, easy to understand.** Prefer small surfaces, obvious names, and boring control flow over clever abstractions.

## Caching

- Check **cache before** any live Google call (all v1 `*.list` methods).
- Cache and quota counters live in **one SQLite file**. A lookup reads one row. Do not keep the cache table in memory.
- TTLs may differ by method (search short; video and playlist-item reads medium; channel and playlist metadata long) — configured in TOML, not hard-coded magic in hot paths.
- Tool flag: **`bypass_cache`** (boolean, default `false`). When true, skip read and **rewrite** the cache entry on success.
- Do **not** cache transient failure bodies as successful data.

## Errors

- **Never swallow errors.** Surface them to the MCP client with a clear message; log at error/warn as appropriate.
- Map Google error JSON to understandable tool errors; preserve enough detail to debug (reason / status) without dumping secrets.

## Logging

- Logs go to **stderr** only (`tracing`); never stdout (stdio MCP framing).
- Default filter is `warn`. `RUST_LOG` overrides it. The server does not log each cache hit or successful call, and it does not log the API key.

## Engineering rails (non-negotiable for “looks good”)

- **Logs** are `tracing` text on stderr. Default filter is `warn` (see above).
- **Settings / prompts outside code** — TOML for settings, Markdown for prompts; secrets in env
- **Unit tests on everything** that matters — see [`TESTING.md`](./TESTING.md)
- **Fixtures only — never live endpoints in tests.** No network to `googleapis.com` from `cargo test`
- **Rate / quota management**: counters `search` and `non_search` for Data API tools, plus the local `transcript` counter for `youtube_transcript`. If the counter that pays cannot cover the next live call, return a tool error and do not call YouTube.
- **Local complexity**: O(1) dispatch. Cache lookup and quota check are one primary-key read each. Pagination is O(N) network by nature — no quadratic scans
- **Comments and docs:** concise and high-signal. A human reading the file should be able to follow what is going on. Explain the decision or the next step when the code does not already say it. Do not narrate obvious lines.

## Source-of-truth files

| File | Role |
|---|---|
| `v1-methods.catalog.json` | v1 tools are `"v1_status": "in_scope"` only |
| `youtube-v3-discovery.v1-readonly.json` | Schemas + parameter detail for v1 methods |

Upstream Discovery URL (not stored in this repo; refresh carefully, then re-trim the readonly file):  
`https://www.googleapis.com/discovery/v1/apis/youtube/v3/rest`
