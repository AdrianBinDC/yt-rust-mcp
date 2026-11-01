# yt-rust-mcp docs (start here)

**Product:** Unofficial Rust MCP server for **YouTube Data API v3** (not affiliated with Google/YouTube). **v1 is implemented.**

**Mode:** **read-only**. **Auth:** API key via `YOUTUBE_API_KEY` only. No OAuth in v1.

**Transcripts:** optional **experimental** tool — undocumented caption path; see [`SCOPE.md`](./SCOPE.md) and [`QUOTA.md`](./QUOTA.md).

**Goal:** make **discoverability easy for agent harnesses** (clear tools, clear “when to use”, no write affordances).

## Read in this order

1. [`SETUP.md`](./SETUP.md) — clone → `.env` → Cursor / other MCP clients
2. [`SCOPE.md`](./SCOPE.md) — hard product rails (what v1 includes and what it refuses)
3. [`TESTING.md`](./TESTING.md) — unit tests + fixtures only; never live endpoints
4. [`QUOTA.md`](./QUOTA.md) — local counters for Data API and transcript
5. [`v1-methods.catalog.json`](./v1-methods.catalog.json) — v1 tools are methods with `"v1_status": "in_scope"` (deferred methods stay in the file as reference)
6. [`youtube-v3-discovery.v1-readonly.json`](./youtube-v3-discovery.v1-readonly.json) — trimmed Discovery doc for those methods + schemas
