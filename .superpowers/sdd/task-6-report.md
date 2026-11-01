# Task 6 Report

Status: Complete

## RED

`cargo test live_client_sends_the_key -- --nocapture` failed to compile as expected because
`ReqwestYoutube` did not exist.

## GREEN

Implemented the blocking `ReqwestYoutube` client, percent-encoded URL builder, API-key query
parameter, JSON response parsing, and Google error-body mapping. The unit test only builds a URL
and does not open a socket.

Verification:

- `cargo test live_client_sends_the_key -- --nocapture`: 1 passed
- `cargo fmt -- --check`: passed
- `cargo test`: 16 passed, 0 failed
- `cargo clippy --all-targets --all-features`: passed with one pre-existing warning
- `git diff --check`: passed

Commit: `fc2b244 Send the API key on YouTube requests`

Concerns: Strict Clippy (`-D warnings`) is blocked by the pre-existing
`clippy::too_many_arguments` warning in `src/call.rs`.

## Fix

All reqwest failures now pass through `map_reqwest_error`, which calls
`without_url()` before creating `ToolError::Http`. The regression test constructs an
unsupported-scheme request error carrying `key=secret-key`; reqwest rejects it before opening a
socket, and the mapped error contains neither the URL nor the key.
