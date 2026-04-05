FROM nginxinc/nginx-unprivileged:1.27-alpine@sha256:65e3e85dbaed8ba248841d9d58a899b6197106c23cb0ff1a132b7bfe0547e4c0
COPY landing/nginx.conf /etc/nginx/conf.d/default.conf
COPY landing/index.html landing/styles.css landing/script.js /usr/share/nginx/html/
COPY landing/src/ /usr/share/nginx/html/src/
COPY landing/aanmelden/ /usr/share/nginx/html/aanmelden/
EXPOSE 8000
CMD ["nginx", "-g", "daemon off;"]
