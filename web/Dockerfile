# Build stage
FROM node:20-alpine AS builder

WORKDIR /app

# Copy package files
COPY web/package*.json ./
RUN npm ci

# Copy source and build
COPY web/ ./
RUN npm run build

# Runtime stage
FROM nginx:alpine

# Install envsubst for config templating
RUN apk add --no-cache gettext

# Copy built assets
COPY --from=builder /app/dist /usr/share/nginx/html

# Copy nginx config template and entrypoint
COPY web/nginx.conf.template /etc/nginx/conf.d/default.conf.template
COPY web/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 80

ENTRYPOINT ["/entrypoint.sh"]