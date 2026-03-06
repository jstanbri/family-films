# ── Stage 1: build ──────────────────────────────────────────────────────────
FROM rust:latest AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies separately
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm src/main.rs

# Build the real app
COPY src ./src
COPY templates ./templates
RUN touch src/main.rs && cargo build --release

# ── Stage 2: runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/family-films .
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/migrations ./migrations

EXPOSE 3000

CMD ["./family-films"]