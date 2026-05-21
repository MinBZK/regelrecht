# syntax=docker/dockerfile:1
# Stage 1: Build frontend
FROM node:26-alpine@sha256:7c6af15abe4e3de859690e7db171d0d711bf37d27528eddfe625b2fe89e097f8 AS frontend-builder
WORKDIR /app
COPY packages/admin/frontend-src/package.json packages/admin/frontend-src/package-lock.json* packages/admin/frontend-src/.npmrc ./
RUN npm ci
COPY packages/admin/frontend-src/ .
# vite outDir is '../static' which resolves to /static from /app
RUN npm run build

# Stage 2: Chef base
FROM rust:1.95-alpine@sha256:606fd313a0f49743ee2a7bd49a0914bab7deedb12791f3a846a34a4711db7ed2 AS chef
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --locked
WORKDIR /build

# Stage 3: Plan dependencies
FROM chef AS planner
COPY packages/Cargo.lock packages/Cargo.toml ./
# Trim workspace members to those actually built by this Dockerfile so chef
# does not try to resolve crates whose source we do not COPY. The `grep`
# assertion ensures the multi-line members format is intact; otherwise the
# `sed` patterns would silently no-op.
# When adding a new workspace member that this image does NOT build, add it
# to this exclusion list as well; otherwise chef will fail to find its source.
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"editor-api"/d; /"tui"/d' Cargo.toml
COPY packages/admin/ admin/
COPY packages/auth/ auth/
COPY packages/corpus/ corpus/
COPY packages/engine/ engine/
COPY packages/pipeline/ pipeline/
COPY packages/harvester/ harvester/
COPY packages/shared/ shared/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 4: Build Rust binary (deps cached separately from source)
FROM chef AS builder
COPY packages/Cargo.lock ./
COPY --from=planner /build/Cargo.toml ./Cargo.toml
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
# `cargo chef cook` rewrites the workspace Cargo.toml without
# `[workspace.lints]`, which breaks members that declare `[lints] workspace
# = true`. Re-COPY the real workspace manifest before the final build.
COPY --from=planner /build/Cargo.toml ./Cargo.toml

COPY packages/admin/ admin/
COPY packages/auth/ auth/
COPY packages/corpus/ corpus/
COPY packages/engine/ engine/
COPY packages/pipeline/ pipeline/
COPY packages/harvester/ harvester/
COPY packages/shared/ shared/
# Invalidate cargo's fingerprint cache so it recompiles with real source
# instead of using the stub binary from cargo-chef cook.
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-admin

# Stage 5: Runtime
FROM alpine:3.23@sha256:5b10f432ef3da1b8d4c7eb6c487f2f5a8f096bc91145e68878dd4a5019afde11
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/regelrecht-admin /usr/local/bin/
COPY --from=frontend-builder /static /app/static

WORKDIR /app
ENV STATIC_DIR=/app/static
USER app
EXPOSE 8000
CMD ["regelrecht-admin"]
