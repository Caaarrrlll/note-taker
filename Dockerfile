# syntax=docker/dockerfile:1

# =============================================================================
# Note Taker web server: note-server plus the web build of the frontend,
# served together on one port.
#
#   docker build -t note-taker .
#   docker run -p 8080:8080 -v note-taker-data:/data note-taker
#
# Configuration: see docs/self-hosting.md.
# =============================================================================


# -----------------------------------------------------------------------------
# Stage 1: build the web frontend (vite --mode web, talks to note-server)
# -----------------------------------------------------------------------------
FROM node:24-bookworm-slim AS web

WORKDIR /src

# Dependencies first, so this layer stays cached until package files change
COPY package.json package-lock.json ./
RUN npm ci

COPY web ./web
RUN npm run build:web


# -----------------------------------------------------------------------------
# Stage 2: build note-server (only the server crate; no desktop/Tauri deps)
# -----------------------------------------------------------------------------
FROM rust:1-bookworm AS server

WORKDIR /src

# The whole workspace is copied because Cargo loads every member's manifest
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps

RUN cargo build \
        --release \
        --locked \
        --package note-server


# -----------------------------------------------------------------------------
# Stage 3: small runtime image, running as a non-root user
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Default user; Unraid templates override it with --user 99:100
RUN useradd \
        --system \
        --uid 10001 \
        --home-dir /app \
        --shell /usr/sbin/nologin \
        notes \
    && mkdir -p /data \
    && chown notes /data

WORKDIR /app

COPY --from=server /src/target/release/note-server /app/note-server
COPY --from=web /src/web/dist /app/web

ENV DATA_DIR=/data \
    WEB_DIR=/app/web \
    PORT=8080

VOLUME ["/data"]
EXPOSE 8080
USER notes

HEALTHCHECK \
    --interval=30s \
    --timeout=5s \
    --start-period=10s \
    --retries=3 \
    CMD ["/app/note-server", "healthcheck"]

CMD ["/app/note-server"]
