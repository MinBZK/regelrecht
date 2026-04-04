# Stage 1: Build
FROM node:24-alpine@sha256:01743339035a5c3c11a373cd7c83aeab6ed1457b55da6a69e014a95ac4e4700b AS builder
WORKDIR /app
COPY docs/package.json docs/package-lock.json* docs/.npmrc ./
RUN --mount=type=secret,id=GITHUB_TOKEN \
    GITHUB_TOKEN=$(cat /run/secrets/GITHUB_TOKEN) npm ci
COPY docs/ .
RUN npm run build

# Stage 2: Production (unprivileged nginx for Kubernetes/OpenShift)
FROM nginxinc/nginx-unprivileged:1.27-alpine@sha256:65e3e85dbaed8ba248841d9d58a899b6197106c23cb0ff1a132b7bfe0547e4c0
COPY docs/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/.vitepress/dist /usr/share/nginx/html
EXPOSE 8000
CMD ["nginx", "-g", "daemon off;"]
