# Stage 1: Build (npm workspace root — builds the frontend-lawmaking workspace)
FROM node:24-alpine@sha256:01743339035a5c3c11a373cd7c83aeab6ed1457b55da6a69e014a95ac4e4700b AS builder
WORKDIR /app
# Workspace manifests first so `npm ci` is cached independently of source.
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend/package.json frontend/
COPY frontend-lawmaking/package.json frontend-lawmaking/
COPY packages/admin/frontend-src/package.json packages/admin/frontend-src/
RUN npm ci
# Source: the shared package + this frontend.
COPY packages/frontend-shared/ packages/frontend-shared/
COPY frontend-lawmaking/ frontend-lawmaking/
RUN npm run build -w frontend-lawmaking

# Stage 2: Production (unprivileged nginx for Kubernetes/OpenShift)
FROM nginxinc/nginx-unprivileged:1.27-alpine@sha256:65e3e85dbaed8ba248841d9d58a899b6197106c23cb0ff1a132b7bfe0547e4c0
COPY frontend-lawmaking/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/frontend-lawmaking/dist /usr/share/nginx/html
EXPOSE 8000
CMD ["nginx", "-g", "daemon off;"]
