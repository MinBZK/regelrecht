# syntax=docker/dockerfile:1
# Stage 1a: Build WASM module using cargo + wasm-bindgen-cli (no wasm-pack needed)
FROM rust:1.94-alpine@sha256:7f752ee8ea5deb9f4863d8c3f228a216a6466619882f09a44b9eda9617dc7770 AS wasm-builder
RUN apk add --no-cache musl-dev
RUN rustup target add wasm32-unknown-unknown
RUN cargo install wasm-bindgen-cli@0.2.114 --locked
WORKDIR /build
COPY packages/Cargo.lock ./
RUN printf '[workspace]\nresolver = "2"\nmembers = [\n    "packages/engine",\n    "packages/shared",\n]\n' > Cargo.toml
COPY packages/engine/ packages/engine/
COPY packages/shared/ packages/shared/
RUN cargo build --target wasm32-unknown-unknown --release --features wasm -p regelrecht-engine && \
    wasm-bindgen --target web --out-dir /wasm-pkg \
      target/wasm32-unknown-unknown/release/regelrecht_engine.wasm

# Stage 1b: Build frontend (with WASM module)
FROM node:24-alpine@sha256:01743339035a5c3c11a373cd7c83aeab6ed1457b55da6a69e014a95ac4e4700b AS frontend-builder
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json* frontend/.npmrc ./
RUN --mount=type=secret,id=GITHUB_TOKEN \
    GITHUB_TOKEN=$(cat /run/secrets/GITHUB_TOKEN) npm ci
COPY frontend/ .
COPY --from=wasm-builder /wasm-pkg/ public/wasm/pkg/
RUN npm run build

# Stage 2: Chef base
FROM rust:1.94-alpine@sha256:7f752ee8ea5deb9f4863d8c3f228a216a6466619882f09a44b9eda9617dc7770 AS chef
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --locked
WORKDIR /build

# Stage 3: Plan dependencies
FROM chef AS planner
COPY packages/Cargo.lock ./
RUN printf '[workspace]\nresolver = "2"\nmembers = [\n    "packages/auth",\n    "packages/editor-api",\n    "packages/corpus",\n    "packages/engine",\n    "packages/shared",\n]\n' > Cargo.toml
COPY packages/auth/ packages/auth/
COPY packages/editor-api/ packages/editor-api/
COPY packages/corpus/ packages/corpus/
COPY packages/engine/ packages/engine/
COPY packages/shared/ packages/shared/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 4: Build Rust binary (deps cached separately from source)
FROM chef AS builder
COPY packages/Cargo.lock ./
RUN printf '[workspace]\nresolver = "2"\nmembers = [\n    "packages/editor-api",\n    "packages/corpus",\n    "packages/engine",\n    "packages/shared",\n]\n' > Cargo.toml
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY packages/editor-api/ packages/editor-api/
COPY packages/corpus/ packages/corpus/
COPY packages/engine/ packages/engine/
COPY packages/shared/ packages/shared/
# Invalidate cargo's fingerprint cache so it recompiles with real source
# instead of using the stub binary from cargo-chef cook.
RUN find packages -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-editor-api

# Stage 5: Runtime
FROM alpine:3.23@sha256:25109184c71bdad752c8312a8623239686a9a2071e8825f20acb8f2198c3f659
RUN apk add --no-cache ca-certificates && \
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
