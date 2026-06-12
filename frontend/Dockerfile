# syntax=docker/dockerfile:1
# Stage 1a: Build WASM module using cargo + wasm-bindgen-cli (no wasm-pack needed)
FROM rust:1.96-alpine@sha256:66f48b19d6e88519e2e58bebe0d945779a6a4ca41c2db17db78c9569655b50ac AS wasm-builder
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
    sed -i '/"admin"/d; /"auth"/d; /"corpus"/d; /"demo-api"/d; /"demo-profile"/d; /"editor-api"/d; /"harvester"/d; /"pipeline"/d; /"tui"/d' Cargo.toml
COPY packages/engine/ engine/
COPY packages/shared/ shared/
RUN cargo build --target wasm32-unknown-unknown --release --features wasm -p regelrecht-engine && \
    wasm-bindgen --target web --out-dir /wasm-pkg \
      target/wasm32-unknown-unknown/release/regelrecht_engine.wasm

# Stage 1b: Build frontend (with WASM module)
FROM node:26-alpine@sha256:144769ec3f32e8ee36b3cfde91e82bee25d9367b20f31a151f3f7eea3a2a8541 AS frontend-builder
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json* frontend/.npmrc ./
RUN npm ci
COPY frontend/ .
COPY --from=wasm-builder /wasm-pkg/ public/wasm/pkg/
RUN npm run build

# Stage 2: Chef base
FROM rust:1.96-alpine@sha256:66f48b19d6e88519e2e58bebe0d945779a6a4ca41c2db17db78c9569655b50ac AS chef
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
    sed -i '/"admin"/d; /"demo-api"/d; /"demo-profile"/d; /"tui"/d' Cargo.toml
COPY packages/auth/ auth/
COPY packages/editor-api/ editor-api/
COPY packages/corpus/ corpus/
COPY packages/engine/ engine/
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
FROM alpine:3.23@sha256:5b10f432ef3da1b8d4c7eb6c487f2f5a8f096bc91145e68878dd4a5019afde11
RUN apk add --no-cache ca-certificates git && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/regelrecht-editor-api /usr/local/bin/
COPY --from=frontend-builder /app/dist /app/static
COPY corpus-registry.yaml /app/corpus-registry.yaml
COPY corpus/regulation/ /app/corpus/regulation/

WORKDIR /app
ENV STATIC_DIR=/app/static
ENV CORPUS_REGISTRY_PATH=/app/corpus-registry.yaml
USER app
EXPOSE 8000
CMD ["regelrecht-editor-api"]
