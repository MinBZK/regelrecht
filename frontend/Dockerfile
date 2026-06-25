# syntax=docker/dockerfile:1
# Stage 1a: Build WASM module using cargo + wasm-bindgen-cli (no wasm-pack needed)
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:2ea3db105d38fdfa4e31f366674287fcaa828087e2fe3973befdc537f2d443b1 AS wasm-builder
RUN apk add --no-cache musl-dev
RUN rustup target add wasm32-unknown-unknown
RUN cargo install wasm-bindgen-cli@0.2.114 --locked
WORKDIR /build
COPY packages/Cargo.lock packages/Cargo.toml ./
# Trim workspace members to engine + shared (the only crates the wasm build
# needs); everything else is dropped so chef does not try to resolve their
# source paths. The `grep` assertion fails the build loudly if
# packages/Cargo.toml is ever reformatted away from a multi-line members
# array — without it the `sed` patterns would silently no-op.
# When adding a new workspace member that this image does NOT build, add it
# to this exclusion list as well; otherwise chef will fail to find its source.
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"auth"/d; /"corpus"/d; /"editor-api"/d; /"harvester"/d; /"pipeline"/d; /"tui"/d' Cargo.toml
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/shared/ shared/
RUN cargo build --target wasm32-unknown-unknown --release --features wasm -p regelrecht-engine && \
    wasm-bindgen --target web --out-dir /wasm-pkg \
      target/wasm32-unknown-unknown/release/regelrecht_engine.wasm

# Stage 1b: Build frontend (with WASM module)
FROM node:26-alpine@sha256:725aeba2364a9b16beae49e180d83bd597dbd0b15c47f1f28875c290bfd255b9 AS frontend-builder
WORKDIR /app
# Workspace manifests first so `npm ci` is cached independently of source.
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend/package.json frontend/
COPY frontend-lawmaking/package.json frontend-lawmaking/
COPY packages/admin/frontend-src/package.json packages/admin/frontend-src/
RUN npm ci
# Source: the shared package + the editor frontend, plus the WASM module.
COPY packages/frontend-shared/ packages/frontend-shared/
COPY frontend/ frontend/
COPY --from=wasm-builder /wasm-pkg/ frontend/public/wasm/pkg/
RUN npm run build -w regelrecht-ui-prototype

# Stage 2: Chef base
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:2ea3db105d38fdfa4e31f366674287fcaa828087e2fe3973befdc537f2d443b1 AS chef
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
    sed -i '/"admin"/d; /"tui"/d' Cargo.toml
COPY packages/auth/ auth/
COPY packages/editor-api/ editor-api/
COPY packages/corpus/ corpus/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/harvester/ harvester/
COPY packages/pipeline/ pipeline/
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

COPY packages/auth/ auth/
COPY packages/editor-api/ editor-api/
COPY packages/corpus/ corpus/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/harvester/ harvester/
COPY packages/pipeline/ pipeline/
COPY packages/shared/ shared/
# regelrecht-corpus (annotation-validation feature) embeds the note schema
# via `include_str!("../../../schema/...")`, relative to packages/corpus/src/.
# WORKDIR is /build and the crate is COPYed to /build/corpus/, so that
# literal resolves to /schema/... here. Put the canonical schema there so
# the embed compiles; this is the same file the source tree has at repo
# root (no second copy — it is COPYed, not vendored).
COPY schema/ /schema/
# Invalidate cargo's fingerprint cache so it recompiles with real source
# instead of using the stub binary from cargo-chef cook.
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-editor-api

# Stage 5: Runtime
FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
RUN apk add --no-cache ca-certificates git && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/regelrecht-editor-api /usr/local/bin/
COPY --from=frontend-builder /app/frontend/dist /app/static
COPY corpus-registry.yaml /app/corpus-registry.yaml
COPY corpus/regulation/ /app/corpus/regulation/

WORKDIR /app
ENV STATIC_DIR=/app/static
ENV CORPUS_REGISTRY_PATH=/app/corpus-registry.yaml
USER app
EXPOSE 8000
CMD ["regelrecht-editor-api"]
