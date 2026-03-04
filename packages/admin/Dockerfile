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
FROM rust:1.93-alpine AS chef
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --locked
WORKDIR /build

# Stage 3: Plan dependencies
FROM chef AS planner
COPY packages/admin/Cargo.toml packages/admin/Cargo.lock ./
COPY packages/admin/src/ src/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 4: Build Rust binary (deps cached separately from source)
FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
COPY packages/admin/Cargo.toml packages/admin/Cargo.lock ./
RUN cargo chef cook --release --recipe-path recipe.json

COPY packages/admin/src/ src/
COPY packages/admin/migrations/ migrations/
RUN cargo build --release --bin regelrecht-admin

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
