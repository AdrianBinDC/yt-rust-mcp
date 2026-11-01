FROM rust:1.89-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home --home-dir /home/ytmcp ytmcp \
    && mkdir -p /data \
    && chown ytmcp:ytmcp /data
COPY --from=build /src/target/release/yt-rust-mcp /usr/local/bin/yt-rust-mcp
COPY config /app/config
COPY prompts /app/prompts
ENV YT_RUST_MCP_CONFIG=/app/config/default.toml
ENV YT_RUST_MCP_PROMPTS=/app/prompts
ENV YT_RUST_MCP_CACHE_DIR=/data
USER ytmcp
ENTRYPOINT ["yt-rust-mcp"]
