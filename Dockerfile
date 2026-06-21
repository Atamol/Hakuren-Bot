FROM rust:1-slim-bookworm AS build
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release
COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/hakuren-bot /usr/local/bin/hakuren-bot
COPY config.docker.toml /app/config.toml
RUN mkdir -p /app/data
ENV HAKUREN_CONFIG=/app/config.toml RUST_LOG=hakuren_bot=info
CMD ["hakuren-bot"]
