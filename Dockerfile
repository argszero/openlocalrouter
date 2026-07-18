# ── Stage 1: Build frontend ──────────────────────────
FROM node:22-alpine AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# ── Stage 2: Build Rust binary ──────────────────────
FROM rust:1.86-slim-bookworm AS backend
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Fake src-tauri to skip workspace member resolution
RUN mkdir -p src-tauri/src && echo 'fn main() {}' > src-tauri/src/main.rs && \
    echo '[package]\nname = "openlocalrouter-tauri"\nversion = "0.1.0"\nedition = "2021"\n\n[[bin]]\nname = "openlocalrouter-tauri"\npath = "src/main.rs"\n\n[dependencies]' > src-tauri/Cargo.toml

# Copy manifests and dummy main for dependency caching
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && echo '' > src/lib.rs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build -p openlocalrouter-core --release && \
    rm -rf /app/target/release/.fingerprint/openlocalrouter-core-* \
           /app/target/release/deps/openlocalrouter_core*

# Copy real source and build
COPY src/ ./src/
COPY rustfmt.toml ./

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build -p openlocalrouter-core --release && \
    cp /app/target/release/openlocalrouter /app/

# ── Stage 3: Runtime ────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=backend /app/openlocalrouter ./
COPY --from=frontend /app/frontend/dist ./frontend/dist

# create data dir to be used as volume
RUN mkdir -p /data

ENV OLR_LISTEN_ADDRESS=0.0.0.0
ENV OLR_LOG_LEVEL=info
ENV OLR_DATA_DIR=/data

EXPOSE 19528

CMD ["/app/openlocalrouter"]
