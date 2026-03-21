# Stage 1: Build
FROM node:24-alpine AS builder
WORKDIR /app
COPY frontend-lawmaking/package.json frontend-lawmaking/package-lock.json* frontend-lawmaking/.npmrc ./
RUN --mount=type=secret,id=GITHUB_TOKEN \
    GITHUB_TOKEN=$(cat /run/secrets/GITHUB_TOKEN) npm ci
COPY frontend-lawmaking/ .
RUN npm run build

# Stage 2: Production (unprivileged nginx for Kubernetes/OpenShift)
FROM nginxinc/nginx-unprivileged:alpine
COPY frontend-lawmaking/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html
EXPOSE 8000
CMD ["nginx", "-g", "daemon off;"]
