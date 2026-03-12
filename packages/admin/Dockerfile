# syntax=docker/dockerfile:1
# Stage 1: Build frontend
FROM node:24-alpine AS frontend-builder
WORKDIR /app
COPY packages/admin/frontend-src/package.json packages/admin/frontend-src/package-lock.json* packages/admin/frontend-src/.npmrc ./
RUN --mount=type=secret,id=GITHUB_TOKEN \
    GITHUB_TOKEN=$(cat /run/secrets/GITHUB_TOKEN) npm ci
COPY packages/admin/frontend-src/ .
# vite outDir is '../static' which resolves to /static from /app
RUN npm run build

# Stage 2: Chef base
FROM rust:1.94-alpine AS chef
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --locked
WORKDIR /build

# Stage 3: Plan dependencies
FROM chef AS planner
COPY packages/Cargo.lock ./
RUN printf '[workspace]\nresolver = "2"\nmembers = [\n    "packages/admin",\n    "packages/corpus",\n    "packages/harvester",\n    "packages/pipeline",\n    "packages/shared",\n]\n' > Cargo.toml
COPY packages/admin/ packages/admin/
COPY packages/corpus/ packages/corpus/
COPY packages/pipeline/ packages/pipeline/
COPY packages/harvester/ packages/harvester/
COPY packages/shared/ packages/shared/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 4: Build Rust binary (deps cached separately from source)
FROM chef AS builder
COPY packages/Cargo.lock ./
RUN printf '[workspace]\nresolver = "2"\nmembers = [\n    "packages/admin",\n    "packages/corpus",\n    "packages/harvester",\n    "packages/pipeline",\n    "packages/shared",\n]\n' > Cargo.toml
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY packages/admin/ packages/admin/
COPY packages/corpus/ packages/corpus/
COPY packages/pipeline/ packages/pipeline/
COPY packages/harvester/ packages/harvester/
COPY packages/shared/ packages/shared/
# Invalidate cargo's fingerprint cache so it recompiles with real source
# instead of using the stub binary from cargo-chef cook.
RUN find packages -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-admin

# Stage 5: Runtime
FROM alpine:3.23
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/regelrecht-admin /usr/local/bin/
COPY --from=frontend-builder /static /app/static

WORKDIR /app
ENV STATIC_DIR=/app/static
USER app
EXPOSE 8000
CMD ["regelrecht-admin"]
