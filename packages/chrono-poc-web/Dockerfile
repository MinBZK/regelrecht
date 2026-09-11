# syntax=docker/dockerfile:1
# De testopstelling: het axum-binary plus de Vue-bundel die erop draait, in één
# image. Hetzelfde patroon als frontend/Dockerfile (node-stage voor de bundel,
# cargo-chef voor de Rust-kant, alpine als runtime), zonder de WASM-stap: deze
# frontend praat met een server en rekent niet zelf.
#
# Wat er met opzet NIET in zit: wetten en wereldbestanden. De server haalt die
# bij het starten op uit de bron die `CHRONO_POC_WORLD_SOURCE` en
# `CHRONO_POC_CORPUS_SOURCE` noemen, met een token uit de omgeving. Daardoor is
# dit image publiek te publiceren, ook als de wereld die het draait dat niet is.

# Stage 1: Build frontend (npm workspace root — bouwt de workspace frontend-chrono-poc)
FROM node:26-alpine@sha256:ef24c5053d50fdc3e4e56eb4e7ddb7861874ab0fdc797046ba897581deb8e868 AS frontend-builder
WORKDIR /app
# Workspace manifests first so `npm ci` is cached independently of source.
# Alleen de twee manifesten die deze bundel nodig heeft: `npm ci` slaat een
# workspace waarvan de map ontbreekt over, dus de andere frontends kosten hier
# geen install-tijd. Komt er een manifest bij dat deze bundel wél nodig heeft,
# dan hoort het hier.
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend-chrono-poc/package.json frontend-chrono-poc/
# --ignore-scripts: een gekaapte release mag zijn install-hooks hier niet
# draaien; de build eronder heeft ze niet nodig.
RUN npm ci --ignore-scripts
# Source: het gedeelde pakket + deze frontend.
COPY packages/frontend-shared/ packages/frontend-shared/
COPY frontend-chrono-poc/ frontend-chrono-poc/
RUN npm run build -w frontend-chrono-poc

# Stage 2: Chef base
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:a41f7740f8b45d45795624eec13a8b42263cc700f19f7e4e86e04d3dda08a479 AS chef
RUN apk add --no-cache musl-dev curl xz
# cargo-chef als officiële prebuilt binary van de releasepagina van het project,
# in plaats van vanaf broncode te compileren. De sha256's zijn de gepubliceerde
# .sha256-bestanden van diezelfde release; zonder die pin zou een gewijzigd of
# afgebroken artefact ongemerkt de build in glijden.
ARG TARGETARCH
ARG CARGO_CHEF_VERSION=0.1.77
ARG CARGO_CHEF_SHA256_AMD64=a3733ab416c3ffddd37914cd13919ca05fee1a1cf654f3016dcfe7f399d89cd1
ARG CARGO_CHEF_SHA256_ARM64=3bff2e7ff979db837c7e820dfd8d51d2d97b9c94283506c6e596ae4887ef178b
RUN set -eu; \
    case "${TARGETARCH:-}" in \
      amd64) arch=x86_64; sha="${CARGO_CHEF_SHA256_AMD64}" ;; \
      arm64) arch=aarch64; sha="${CARGO_CHEF_SHA256_ARM64}" ;; \
      *) echo "geen gepinde cargo-chef-binary voor TARGETARCH='${TARGETARCH:-}'" >&2; exit 1 ;; \
    esac; \
    dir="cargo-chef-${arch}-unknown-linux-musl"; \
    curl -fsSLo "/tmp/${dir}.tar.xz" \
      "https://github.com/LukeMathWalker/cargo-chef/releases/download/v${CARGO_CHEF_VERSION}/${dir}.tar.xz"; \
    echo "${sha}  /tmp/${dir}.tar.xz" | sha256sum -c -; \
    tar -xJf "/tmp/${dir}.tar.xz" -C /tmp; \
    install -m0755 "/tmp/${dir}/cargo-chef" /usr/local/bin/cargo-chef; \
    rm -rf "/tmp/${dir}.tar.xz" "/tmp/${dir}"; \
    cargo chef --version
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
    sed -i '/"admin"/d; /"arch-extract"/d; /"editor-api"/d; /"harvester"/d; /"pipeline"/d; /"tui"/d' Cargo.toml
COPY packages/auth/ auth/
COPY packages/chrono-poc-web/ chrono-poc-web/
COPY packages/corpus/ corpus/
COPY packages/engine/ engine/
COPY packages/github/ github/
COPY packages/law-model/ law-model/
COPY packages/shared/ shared/
COPY packages/simulator/ simulator/
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
COPY packages/chrono-poc-web/ chrono-poc-web/
COPY packages/corpus/ corpus/
COPY packages/engine/ engine/
COPY packages/github/ github/
COPY packages/law-model/ law-model/
COPY packages/shared/ shared/
COPY packages/simulator/ simulator/
# regelrecht-engine en regelrecht-corpus bakken de JSON-schema's in met
# `include_str!("../../../schema/...")`, relatief aan hun eigen src/. De crates
# staan hier op /build/engine en /build/corpus (niet /build/packages/...), dus
# die literal wijst naar /schema/... — dezelfde layout-truc als in
# frontend/Dockerfile en packages/admin/Dockerfile.
COPY schema/ /schema/
# Invalidate cargo's fingerprint cache so it recompiles with real source
# instead of using the stub binary from cargo-chef cook.
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin chrono-poc-web

# Stage 5: Runtime
FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
# ca-certificates: het opstarten haalt de wereld en het corpus over HTTPS bij
# GitHub op. Geen git — dat gaat via de tarball-API, niet via een clone.
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/chrono-poc-web /usr/local/bin/
COPY --from=frontend-builder /app/frontend-chrono-poc/dist /app/static

WORKDIR /app
ENV STATIC_DIR=/app/static
USER app
EXPOSE 8000
CMD ["chrono-poc-web"]
