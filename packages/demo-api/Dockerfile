# syntax=docker/dockerfile:1
# Small Rust build for the burger-demo LLM-uitleg-proxy.

FROM rust:1.95-alpine@sha256:606fd313a0f49743ee2a7bd49a0914bab7deedb12791f3a846a34a4711db7ed2 AS chef
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --locked
WORKDIR /build

FROM chef AS planner
COPY packages/Cargo.lock ./
RUN printf '[workspace]\nresolver = "2"\nmembers = [\n    "packages/demo-api",\n]\n' > Cargo.toml
COPY packages/demo-api/ packages/demo-api/
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY packages/Cargo.lock ./
RUN printf '[workspace]\nresolver = "2"\nmembers = [\n    "packages/demo-api",\n]\n' > Cargo.toml
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY packages/demo-api/ packages/demo-api/
RUN find packages -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-demo-api

FROM alpine:3.23@sha256:5b10f432ef3da1b8d4c7eb6c487f2f5a8f096bc91145e68878dd4a5019afde11
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app
COPY --from=builder /build/target/release/regelrecht-demo-api /usr/local/bin/
USER app
EXPOSE 7181
CMD ["regelrecht-demo-api"]
