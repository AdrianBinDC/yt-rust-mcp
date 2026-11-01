# Testing policy (v1)

## Rules

1. **Unit-test the logic.** Parsers, tool handlers, quota ledger, request builders, response mappers, error paths — each gets tests.
2. **Fixtures only.** Tests load JSON (and similar) from `fixtures/`. They must not open sockets to YouTube or Google.
3. **No live endpoint in CI or `cargo test`.** If a test needs HTTP, inject a fake transport / `wiremock`-style local mock fed by fixtures — still no real `googleapis.com`.
4. **Quota tests are local.** Assert counters and refusal behavior; do not query Cloud Monitoring in unit tests.
5. **Deterministic.** Fixed clocks / budgets where time or daily reset matters.

## Fixture layout (planned)

```
fixtures/
  youtube/
    search.list.ok.json
    videos.list.ok.json
    videos.list.empty.json
    channels.list.ok.json
    playlists.list.ok.json
    playlistItems.list.ok.json
    playlistItems.list.page2.json
    error.quotaExceeded.json
    error.rateLimitExceeded.json
    error.forbidden.json
```

Names should mirror the API method + outcome. Keep bodies anonymized / synthetic where possible (no need for real channel secrets).

## What “everything” means in practice

| Area | Must have fixture-backed tests |
|---|---|
| Catalog / request build | query params, `part`, pagination tokens |
| Response parse | happy path, empty, malformed |
| Tools | arg validation, when-to-refuse |
| Quota ledger | debit to `search` or `non_search`, soft-fail at either limit |
| Errors | map Google error JSON → tool error text |

Manual smoke against a real key is **allowed for a human**, outside the test suite (e.g. a documented `cargo run` check). It is **not** a unit test.
