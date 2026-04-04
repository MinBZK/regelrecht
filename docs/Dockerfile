# Stage 1: Build
FROM node:24-alpine AS builder
WORKDIR /app
COPY docs/package.json docs/package-lock.json* docs/.npmrc ./
RUN --mount=type=secret,id=GITHUB_TOKEN \
    GITHUB_TOKEN=$(cat /run/secrets/GITHUB_TOKEN) npm ci
COPY docs/ .
RUN npm run build

# Stage 2: Production (unprivileged nginx for Kubernetes/OpenShift)
FROM nginxinc/nginx-unprivileged:1.27-alpine
COPY docs/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/.vitepress/dist /usr/share/nginx/html
EXPOSE 8000
CMD ["nginx", "-g", "daemon off;"]
