#!/bin/sh
# Entrypoint to substitute environment variables in nginx config

set -e

# Default values
export API_SERVICE_NAME="${API_SERVICE_NAME:-powershell-learning-api}"
export API_SERVICE_PORT="${API_SERVICE_PORT:-3000}"

# Substitute variables in nginx config template
envsubst '${API_SERVICE_NAME} ${API_SERVICE_PORT}' < /etc/nginx/conf.d/default.conf.template > /etc/nginx/conf.d/default.conf

# Execute nginx
exec nginx -g "daemon off;"