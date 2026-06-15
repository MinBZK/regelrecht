# syntax=docker/dockerfile:1
# Parameterized Dockerfile for all three pipeline images. The build stages are
# shared; select the image with --target and tell the builder which binary to
# compile with --build-arg BIN. Valid combinations (build from the repo root):
#
#   --target pipeline-api      --build-arg BIN=regelrecht-pipeline-api   (default)
#   --target harvester-worker  --build-arg BIN=regelrecht-harvest-worker
#   --target enrich-worker     --build-arg BIN=regelrecht-enrich-worker
#
# Example:
#   docker build -f packages/pipeline/Dockerfile \
#     --build-arg BIN=regelrecht-harvest-worker --target harvester-worker .

# Stage 1: Chef base
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:2ea3db105d38fdfa4e31f366674287fcaa828087e2fe3973befdc537f2d443b1 AS chef
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
    sed -i '/"admin"/d; /"auth"/d; /"editor-api"/d; /"tui"/d' Cargo.toml
COPY packages/corpus/ corpus/
COPY packages/engine/ engine/
COPY packages/pipeline/ pipeline/
COPY packages/harvester/ harvester/
COPY packages/shared/ shared/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build (deps cached separately from source). Everything up to the
# final `cargo build` layer is identical for all BIN variants, so the
# expensive `cargo chef cook` layer is shared between the three images.
FROM chef AS builder
COPY packages/Cargo.lock ./
COPY --from=planner /build/Cargo.toml ./Cargo.toml
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
# `cargo chef cook` rewrites the workspace Cargo.toml without
# `[workspace.lints]`, which breaks members that declare `[lints] workspace
# = true`. Re-COPY the real workspace manifest before the final build.
COPY --from=planner /build/Cargo.toml ./Cargo.toml

COPY packages/corpus/ corpus/
COPY packages/engine/ engine/
COPY packages/pipeline/ pipeline/
COPY packages/harvester/ harvester/
COPY packages/shared/ shared/
ARG BIN=regelrecht-pipeline-api
# Invalidate cargo's fingerprint cache so it recompiles with real source
# instead of using the stub binary from cargo-chef cook.
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin "${BIN}"

# Stage 4: Shared runtime base
FROM alpine:3.23@sha256:25109184c71bdad752c8312a8623239686a9a2071e8825f20acb8f2198c3f659 AS runtime-base
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app

# Runtime: harvester-worker
# tini runs as PID 1 so reparented git grandchildren (merge-base,
# git-remote-https, the askpass helper) get reaped instead of piling up as
# zombies against the container's low pids limit — the accumulation that wedges
# the worker into a "cannot fork() / Resource temporarily unavailable" loop on
# long harvest runs.
FROM runtime-base AS harvester-worker
RUN apk add --no-cache git tini

COPY --from=builder /build/target/release/regelrecht-harvest-worker /usr/local/bin/
COPY packages/pipeline/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

USER app
EXPOSE 8000

ENV REGULATION_REPO_PATH=/tmp/regulation-repo
ENV REGULATION_OUTPUT_BASE=regulation/nl

ENTRYPOINT ["/sbin/tini", "--", "entrypoint.sh"]

# Runtime: enrich-worker
# tini runs as PID 1 so reparented subprocesses are reaped instead of leaking as
# zombies. The enrich worker kills the LLM CLI (claude/opencode) on timeout,
# which orphans its child processes — without an init those accumulate against
# the container's low pids limit and eventually cause fork()/EAGAIN failures.
FROM runtime-base AS enrich-worker
RUN apk add --no-cache git nodejs npm tini

# Install opencode and claude CLI globally (pinned versions for reproducibility)
RUN npm install -g --ignore-scripts opencode-ai@1.2.25 @anthropic-ai/claude-code@2.1.74 && \
    npm cache clean --force

# Copy opencode provider config (VLAM) — the API key is injected at runtime
COPY packages/pipeline/opencode.json /etc/opencode/opencode.json

# Install opencode provider plugin in a shared location
COPY packages/pipeline/opencode-plugins-package.json /opt/opencode-plugins/package.json
COPY packages/pipeline/opencode-plugins-package-lock.json /opt/opencode-plugins/package-lock.json
RUN cd /opt/opencode-plugins && \
    npm ci --ignore-scripts && \
    npm cache clean --force

COPY --from=builder /build/target/release/regelrecht-enrich-worker /usr/local/bin/
COPY packages/pipeline/entrypoint-enrich.sh /usr/local/bin/entrypoint-enrich.sh
RUN chmod +x /usr/local/bin/entrypoint-enrich.sh

# Bake skill files into the image so the LLM can read them at runtime.
# ensure_skills() in enrich.rs symlinks these into each corpus checkout.
COPY .claude/skills/law-generate/   /opt/skills/.claude/skills/law-generate/
COPY .claude/skills/law-mvt-research/ /opt/skills/.claude/skills/law-mvt-research/
COPY .claude/skills/law-reverse-validate/ /opt/skills/.claude/skills/law-reverse-validate/

USER app
EXPOSE 8000

ENV REGULATION_REPO_PATH=/tmp/regulation-repo
ENV REGULATION_OUTPUT_BASE=regulation/nl

ENTRYPOINT ["/sbin/tini", "--", "entrypoint-enrich.sh"]

# Runtime: pipeline-api (last stage, so also the default build target)
FROM runtime-base AS pipeline-api
COPY --from=builder /build/target/release/regelrecht-pipeline-api /usr/local/bin/

USER app
EXPOSE 8000

CMD ["regelrecht-pipeline-api"]
