# syntax=docker/dockerfile:1
# Stage 1a: Build WASM engine (same pattern as frontend/Dockerfile)
FROM rust:1.95-alpine@sha256:606fd313a0f49743ee2a7bd49a0914bab7deedb12791f3a846a34a4711db7ed2 AS wasm-builder
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

# Stage 1b: Bundle demo-assets (law YAMLs + persona JSON) so the Vite build
# picks them up under public/demo-assets.
FROM alpine:3.23@sha256:5b10f432ef3da1b8d4c7eb6c487f2f5a8f096bc91145e68878dd4a5019afde11 AS assets-builder
RUN apk add --no-cache bash python3 py3-yaml coreutils
WORKDIR /build
COPY corpus/ corpus/
COPY corpus-demo/ corpus-demo/
COPY scripts/build-demo-assets.sh scripts/build-demo-assets.sh
RUN mkdir -p frontend-demo && bash scripts/build-demo-assets.sh

# Stage 2: Build frontend
FROM node:24-alpine@sha256:d1b3b4da11eefd5941e7f0b9cf17783fc99d9c6fc34884a665f40a06dbdfc94f AS frontend-builder
WORKDIR /app
COPY frontend-demo/package.json frontend-demo/package-lock.json frontend-demo/.npmrc ./
RUN --mount=type=secret,id=GITHUB_TOKEN \
    GITHUB_TOKEN=$(cat /run/secrets/GITHUB_TOKEN) npm ci --no-audit --no-fund
COPY frontend-demo/ .
COPY --from=wasm-builder /wasm-pkg/ public/wasm/pkg/
COPY --from=assets-builder /build/frontend-demo/public/demo-assets/ public/demo-assets/
RUN npm run build

# Stage 3: Serve via unprivileged nginx
FROM nginxinc/nginx-unprivileged:1.27-alpine@sha256:65e3e85dbaed8ba248841d9d58a899b6197106c23cb0ff1a132b7bfe0547e4c0
COPY frontend-demo/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=frontend-builder /app/dist /usr/share/nginx/html
EXPOSE 8000
CMD ["nginx", "-g", "daemon off;"]
