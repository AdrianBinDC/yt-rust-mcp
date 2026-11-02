# yt-rust-mcp

[![CI](https://github.com/AdrianBinDC/yt-rust-mcp/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/AdrianBinDC/yt-rust-mcp/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust 1.89+](https://img.shields.io/badge/rust-1.89%2B-blue.svg)](https://www.rust-lang.org)
[![MCP stdio](https://img.shields.io/badge/MCP-stdio-informational.svg)](./docs/SETUP.md)

Unofficial, not affiliated with Google or YouTube.

## 🎯 What it is

**v0.2.0.** A small Rust server that plugs into AI coding assistants (via [MCP](https://modelcontextprotocol.io/)) so they can look up public YouTube data: search, video details, channels, and playlists. Read-only. API key only.

## 💬 What you can ask

- What’s new on this channel this week?
- Summarize the metadata for this video URL.
- List everything on this playlist, in order.
- Find recent videos about X and pull titles, lengths, and view counts.

Optional experimental transcript fetch is available (off by default).

## ⚙️ How it behaves

- Speaks MCP over stdio, or optional HTTP, so assistants can call YouTube tools.
- Cache-first SQLite store; local daily quota counters before live API calls.
- Tests use fixtures only — `cargo test` never hits YouTube.
- Refuses writes and anything that needs OAuth or private data.

## 🚀 Get started

Follow [`docs/SETUP.md`](./docs/SETUP.md) (clone → API key → Cursor or another MCP client).

## 📚 Docs

- [`docs/SCOPE.md`](./docs/SCOPE.md) — what v0.2.0 includes and what it refuses
- [`docs/TESTING.md`](./docs/TESTING.md) — unit tests and fixtures-only policy
- [`docs/QUOTA.md`](./docs/QUOTA.md) — local counters for Data API and transcript
- [`docs/v1-methods.catalog.json`](./docs/v1-methods.catalog.json) — in-scope methods

## 🚫 Out of scope

No uploads, comments, private accounts, or downloading video files.
