# Multi-stage build for the enrich worker.
#
# Installs both LLM CLI tools:
#   - opencode (npm) — for VLAM provider
#   - claude (npm @anthropic-ai/claude-code) — for Anthropic provider
#
# The worker binary is built from source and runs as non-root.
#
# Auth configuration:
#   - OpenCode/VLAM: mount auth.json to /home/worker/.local/share/opencode/auth.json
#     and config to /home/worker/.config/opencode/opencode.json
#   - Claude: set ANTHROPIC_API_KEY env var or mount credentials
#
# Provider selection via env vars:
#   LLM_PROVIDER=opencode|claude  (default: opencode)
#   LLM_MODEL=<model>             (optional, e.g. vlam/ubiops-deployment/...)
#   LLM_TIMEOUT_SECS=600          (optional)

# Stage 1: Build the Rust binary
FROM rust:1.86-bookworm AS builder

WORKDIR /build
COPY packages/ packages/
COPY schema/ schema/
COPY regulation/ regulation/

WORKDIR /build/packages
RUN cargo build --release --bin regelrecht-enrich-worker

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install dependencies: Node.js (for opencode), git (for corpus), ca-certificates
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

# Install opencode and claude CLI globally
RUN npm install -g opencode @anthropic-ai/claude-code

# Create non-root user
RUN useradd --create-home --shell /bin/bash worker
USER worker
WORKDIR /home/worker

# Copy opencode provider config (VLAM)
# This configures the VLAM provider — the API key is mounted separately.
COPY --chown=worker packages/pipeline/opencode.json /home/worker/.config/opencode/opencode.json

# Install opencode provider plugin
RUN mkdir -p /home/worker/.config/opencode && \
    cd /home/worker/.config/opencode && \
    npm init -y && \
    npm install @ai-sdk/openai-compatible

# Copy the built binary
COPY --from=builder /build/packages/target/release/regelrecht-enrich-worker /usr/local/bin/regelrecht-enrich-worker

EXPOSE 8000

ENTRYPOINT ["regelrecht-enrich-worker"]
