FROM rust:1.89-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=build /src/target/release/yt-rust-mcp /usr/local/bin/yt-rust-mcp
COPY config /app/config
COPY prompts /app/prompts
ENV YT_RUST_MCP_CONFIG=/app/config/default.toml
ENV YT_RUST_MCP_PROMPTS=/app/prompts
ENTRYPOINT ["yt-rust-mcp"]
