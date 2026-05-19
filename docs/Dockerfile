# Stage 1: Build
# Builder is node:24-bookworm-slim (glibc), not node:24-alpine (musl):
# rehype-mermaid renders ```mermaid blocks to inline SVG at build time via
# a headless Chromium driven by Playwright, and `playwright install
# --with-deps chromium` only resolves its system dependencies on a
# Debian/glibc base. Digest-pinned like the runtime stage. The runtime
# stage is unchanged (unprivileged nginx).
FROM node:24-bookworm-slim@sha256:24dc26ef1e3c3690f27ebc4136c9c186c3133b25563ae4d7f0692e4d1fe5db0e AS builder
WORKDIR /app
COPY docs/package.json docs/package-lock.json* docs/.npmrc ./
RUN npm ci
COPY docs/ .
RUN npx playwright install --with-deps chromium
RUN npm run build

# Stage 2: Production (unprivileged nginx for Kubernetes/OpenShift)
FROM nginxinc/nginx-unprivileged:1.27-alpine@sha256:65e3e85dbaed8ba248841d9d58a899b6197106c23cb0ff1a132b7bfe0547e4c0
COPY docs/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html
EXPOSE 8000
CMD ["nginx", "-g", "daemon off;"]
