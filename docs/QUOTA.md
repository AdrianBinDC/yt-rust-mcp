# Quota (v1)

YouTube Data API projects get a **daily quota**. Defaults change; check Google Cloud → APIs → YouTube Data API v3 → Quotas.

Practical rules for this MCP:

1. **Two punch cards, not one.** A live page spends exactly one card. A cache hit spends neither.

| Tool | API | Counter | Cost per live page | Default daily size |
|---|---|---|---|---|
| `youtube_search` | `search.list` | `search` | 1 | 100 (`YOUTUBE_SEARCH_BUDGET`) |
| `youtube_videos_list` | `videos.list` | `non_search` | 1 | 10,000 shared (`YOUTUBE_NON_SEARCH_BUDGET`) |
| `youtube_channels_list` | `channels.list` | `non_search` | 1 | same 10,000 |
| `youtube_playlists_list` | `playlists.list` | `non_search` | 1 | same 10,000 |
| `youtube_playlist_items_list` | `playlistItems.list` | `non_search` | 1 | same 10,000 |

`search.list` does not spend `non_search`. The other four do not spend `search`. Both counters live in the SQLite file named in the design spec.

Deferred catalog methods (`videoCategories.list`, `i18nLanguages.list`, `i18nRegions.list`) are not v1 tools. If added later, each costs 1 on `non_search`.

2. **Example:** one search, then three video lookups. `search` 100 → 99. `non_search` 10,000 → 9,997. After 100 searches, `search` is done for the day even if `non_search` is still full.
3. **Failed calls can still cost quota.** Validate inputs before HTTP.
4. **Pagination multiplies cost.** Each page of `playlistItems.list` / `search.list` is another call.
5. Server should expose remaining budget estimates in logs (and optionally in tool results) so harnesses can stop early.

Implementers: put costs next to each tool description; refuse or soft-fail when local counters say the budget is gone.
