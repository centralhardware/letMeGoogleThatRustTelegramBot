FROM rust:1-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/untitled /app/untitled

USER 10001

# Unhealthy as soon as the last successful getMe is older than two heartbeats;
# the heartbeat itself is written by the health module.
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
  CMD [ -f /tmp/health ] && [ $(( $(date +%s) - $(stat -c %Y /tmp/health) )) -lt 120 ]

ENTRYPOINT ["/app/untitled"]
