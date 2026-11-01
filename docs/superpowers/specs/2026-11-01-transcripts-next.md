# Next: public transcripts (not official Data API)

**Status:** Parked. Design before code. Not started.  
**Depends on:** v1 MCP tools, which are on `main`.

## Why

Discoverability without the spoken text is only half useful. Portfolio story stays honest: v1 is official API-key reads; transcripts are a **separate, clearly labeled** capability that is **not** YouTube Data API v3.

## Non-goals for the first transcript slice

- Do not pretend this is `captions.download` or OAuth
- Do not mix transcript fetches into the `search` / `non_search` Google quota counters
- Do not scrape HTML pages if a caption track URL path works

## Product shape (draft)

- New tool, e.g. `youtube_transcript` — input: video id (and maybe language)
- Server instructions + tool copy must say: **unofficial**, can break when YouTube changes, not part of the Data API
- **Cache** successful transcript text in the same SQLite file (own table, own TTL — default longer than search, e.g. days)
- **Graceful errors:** no track, disabled captions, throttle/block, empty body → clear tool error; never swallow; never put secrets in messages
- Rate behavior: soft local limiter (per process / per day) so a runaway agent cannot hammer timedtext; document that Google has no public quota number for this

## Open decisions (answer before implementing)

1. Source library vs hand-rolled timedtext client?
2. Auto-generated vs manual tracks only, or prefer manual then fall back?
3. Return plain text, timed cues (JSON), or both?
4. Same binary / same MCP server, or a second binary so the official surface stays pure?

## Implementation order (when we start)

1. Spec answers to the four decisions above  
2. Fixture-backed client (recorded caption payloads under `fixtures/` — still no live calls in `cargo test`)  
3. SQLite cache table + local rate gate  
4. MCP tool + prompt copy  
5. Live smoke on one known video with captions  

## Rails that still apply

Lean, high-signal comments, never swallow errors, stderr logging only, no API key in logs.
