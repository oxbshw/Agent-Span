# syntax=docker/dockerfile:1

# Stage 1: Build the React dashboard
FROM node:20-bookworm-slim AS builder

WORKDIR /app

# Copy lock file and manifest first to cache dependency install.
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund

# Copy the rest of the source and build.
COPY . .
RUN npm run build

# Stage 2: Serve the built assets with nginx
FROM nginx:alpine

# Copy the built dist into nginx's serve directory.
COPY --from=builder /app/dist /usr/share/nginx/html

# nginx config: SPA fallback so client-side routing works.
RUN printf 'server {\n\
    listen 80;\n\
    root /usr/share/nginx/html;\n\
    index index.html;\n\
    location / {\n\
        try_files $uri $uri/ /index.html;\n\
    }\n\
}\n' > /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
