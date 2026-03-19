# Stage 1: Build
FROM node:24-alpine AS builder
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json* frontend/.npmrc ./
RUN --mount=type=secret,id=GITHUB_TOKEN \
    GITHUB_TOKEN=$(cat /run/secrets/GITHUB_TOKEN) npm ci
COPY frontend/ .
COPY regulation/ ../regulation/
RUN npm run build

# Stage 2: Production (unprivileged nginx for Kubernetes/OpenShift)
FROM nginxinc/nginx-unprivileged:alpine
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf
COPY --chown=nginx:nginx --from=builder /app/dist /usr/share/nginx/html
COPY --chmod=755 frontend/docker-entrypoint.sh /docker-entrypoint.d/20-env-config.sh
EXPOSE 8000
