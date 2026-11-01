# Testing policy (v1)

## Rules

1. **Unit-test the logic.** Parsers, tool handlers, quota ledger, request builders, response mappers, error paths — each gets tests.
2. **Fixtures only.** Tests load JSON (and similar) from `fixtures/`. They must not open sockets to YouTube or Google.
3. **No live endpoint in CI or `cargo test`.** If a test needs HTTP, inject a fake transport / `wiremock`-style local mock fed by fixtures — still no real `googleapis.com`.
4. **Quota tests are local.** Assert counters and refusal behavior; do not query Cloud Monitoring in unit tests.
5. **Deterministic.** Fixed clocks / budgets where time or daily reset matters.

## Fixture layout

```
fixtures/youtube/
  search.list.ok.json
  search.list.page2.json
  videos.list.ok.json
  error.quotaExceeded.json
  error.forbidden.json
  transcript.timed.json
  transcript.timed.text
  transcript.timed.srt
  transcript.timed.vtt
  transcript.player.ok.json
  transcript.player.no-tracks.json
  transcript.player.token.json
  transcript.empty.json
```

Transcript tests render the saved timed list and never call YouTube.

Names mirror the API method and the outcome. Bodies are synthetic.

`search.list.ok.json`, `search.list.page2.json`, and `videos.list.ok.json` feed the call-path tests. `error.quotaExceeded.json` feeds the Google error mapper. `error.forbidden.json` is saved and is not loaded by a test.

## What the suite covers

| Area | Covered now |
|---|---|
| Request build | default `part`, page size 25, auth-only and unknown arguments, `videos.list` paging refusal |
| Call path | a second identical search is a cache hit and does not debit again; two pages merge `items` and debit twice |
| Quota ledger | debit stops at the budget, resets on a new day, and `search` does not change `non_search` |
| Errors | `quotaExceeded` JSON maps to status and reason; the message does not contain an API key |
| Tools | refusal still happens after the MCP JSON path; prompt files name the read-only rules |
| Process | default log filter is `warn`; stdio answers initialize and `tools/list` without calling YouTube |

Manual smoke against a real key is **allowed for a human**, outside the test suite (a `cargo run` check). It is **not** a unit test.
