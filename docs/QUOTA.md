# Quota (v1)

YouTube Data API projects get a **daily quota**. Defaults change; check Google Cloud → APIs → YouTube Data API v3 → Quotas.

Practical rules for this MCP:

1. **Most `*.list` calls** cost a small number of units (often **1**). Budget them under the main daily pool (~**10,000** units by default historically).
2. **`search.list` is scarce.** It often sits in a **separate daily call bucket** (on the order of **~100 search calls/day**). Treat search as expensive even when unit math looks fine.
3. **Failed calls can still cost quota.** Validate inputs before HTTP.
4. **Pagination multiplies cost.** Each page of `playlistItems.list` / `search.list` is another call.
5. Server should expose remaining budget estimates in logs (and optionally in tool results) so harnesses can stop early.

Implementers: put costs next to each tool description; refuse or soft-fail when local counters say the budget is gone.
