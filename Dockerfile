FROM rust:1.97-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations
RUN cargo build --locked --release -p qalqon-bot

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home qalqon

COPY --from=builder /app/target/release/qalqon-bot /usr/local/bin/qalqon-bot
USER 10001
EXPOSE 8080 8081
HEALTHCHECK --interval=15s --timeout=5s --start-period=15s --retries=3 \
    CMD ["/usr/local/bin/qalqon-bot", "--healthcheck"]
ENTRYPOINT ["/usr/local/bin/qalqon-bot"]
