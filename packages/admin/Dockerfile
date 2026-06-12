# syntax=docker/dockerfile:1
# Stage 1: Build frontend
FROM node:26-alpine@sha256:3ad34ca6292aec4a91d8ddeb9229e29d9c2f689efd0dd242860889ac71842eba AS frontend-builder
WORKDIR /app
COPY packages/admin/frontend-src/package.json packages/admin/frontend-src/package-lock.json* packages/admin/frontend-src/.npmrc ./
RUN npm ci
COPY packages/admin/frontend-src/ .
# vite outDir is '../static' which resolves to /static from /app
RUN npm run build

# Stage 2: Chef base
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
    sed -i '/"demo-api"/d; /"demo-profile"/d; /"editor-api"/d; /"tui"/d' Cargo.toml
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
FROM alpine:3.24@sha256:a2d49ea686c2adfe3c992e47dc3b5e7fa6e6b5055609400dc2acaeb241c829f4
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/regelrecht-admin /usr/local/bin/
COPY --from=frontend-builder /static /app/static

WORKDIR /app
ENV STATIC_DIR=/app/static
USER app
EXPOSE 8000
CMD ["regelrecht-admin"]
