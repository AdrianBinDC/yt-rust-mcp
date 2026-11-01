Unofficial MCP server, not affiliated with Google or YouTube.
Read-only YouTube Data API v3. Auth is an API key. Data is public.

Tools: youtube_search, youtube_videos_list, youtube_channels_list, youtube_playlists_list, youtube_playlist_items_list, youtube_transcript.

No uploads, no edits, no OAuth, no comments, no live chat.
Prefer an id lookup over youtube_search when the id is already known.
youtube_search spends the search counter. The other Data API tools spend non_search.
youtube_transcript is experimental and unsupported. It reads YouTube's public caption track via an undocumented path, can break when YouTube changes, and is not a Data API method. It does not use the API key. A cached repeat costs 0. A live fetch spends the local transcript counter.
