FROM nginxinc/nginx-unprivileged:alpine
COPY landing/nginx.conf /etc/nginx/conf.d/default.conf
COPY landing/index.html landing/styles.css landing/script.js /usr/share/nginx/html/
COPY landing/src/ /usr/share/nginx/html/src/
COPY landing/aanmelden/ /usr/share/nginx/html/aanmelden/
EXPOSE 8000
CMD ["nginx", "-g", "daemon off;"]
