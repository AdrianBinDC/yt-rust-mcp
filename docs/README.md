# yt-rust-mcp docs (start here)

**Product:** Rust MCP server for **YouTube Data API v3**, phase **v1**.

**Mode:** **read-only**. **Auth:** API key via `YOUTUBE_API_KEY` only. No OAuth in v1.

**Goal:** make **discoverability easy for agent harnesses** (clear tools, clear “when to use”, no write affordances).

## Read in this order

1. [`SCOPE.md`](./SCOPE.md) — hard product rails (what we build / what we refuse)
2. [`superpowers/specs/2026-11-01-yt-rust-mcp-design.md`](./superpowers/specs/2026-11-01-yt-rust-mcp-design.md) — formal v1 design (Mermaid + cache rationale)
3. [`TESTING.md`](./TESTING.md) — unit tests + fixtures only; never live endpoints
4. [`v1-methods.catalog.json`](./v1-methods.catalog.json) — v1 candidates; **implement only** methods with `"v1_status": "in_scope"` (deferred methods stay in the file as reference)
5. [`youtube-v3-discovery.v1-readonly.json`](./youtube-v3-discovery.v1-readonly.json) — trimmed Discovery doc for those methods + schemas
6. [`youtube-v3-discovery.full.json`](./youtube-v3-discovery.full.json) — full upstream Discovery dump (**do not implement from this**)

## Do not

- Do not implement insert / update / delete / upload from the full Discovery file
- Do not treat captions / comments / live / analytics as v1 scope
- Do not use OAuth in v1
- Do not wander into yt-dlp / InnerTube transcript scraping in this repo’s v1 (different product)
