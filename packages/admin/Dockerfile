# syntax=docker/dockerfile:1
# API-only image: the harvester-admin binary is a standalone harvest API. Its
# dashboard UI now lives in the editor (frontend/src/harvester), which reaches
# this API through the editor-api /api/harvest-admin/* proxy — so there is no
# frontend build stage and no static assets here.

# Stage 1: Chef base
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:f87aa870663e2b57ec8c69de82c7eedf7383bee987eef7612c0359635eaadb41 AS chef
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --locked
WORKDIR /build

# Stage 2: Plan dependencies
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
COPY packages/law-model/ law-model/
COPY packages/pipeline/ pipeline/
COPY packages/harvester/ harvester/
COPY packages/shared/ shared/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build Rust binary (deps cached separately from source)
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
COPY packages/law-model/ law-model/
COPY packages/pipeline/ pipeline/
COPY packages/harvester/ harvester/
COPY packages/shared/ shared/
# Invalidate cargo's fingerprint cache so it recompiles with real source
# instead of using the stub binary from cargo-chef cook.
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-admin

# Stage 4: Runtime
FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/regelrecht-admin /usr/local/bin/

WORKDIR /app
USER app
EXPOSE 8000
CMD ["regelrecht-admin"]
