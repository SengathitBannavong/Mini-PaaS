#!/usr/bin/env bash
# One Nginx baseline run on :8090: blue -> green at second 20 by editing the config
# and running `nginx -s reload`. Keep-alive to the upstream, like the router's pooled client.
# Usage: bench/nginx-run.sh [run] [rate] [duration]   e.g. bench/nginx-run.sh 1 500 60s
set -euo pipefail
cd "$(dirname "$0")"
source ./common.sh
parse_args "$@"
CONF=$RESULTS/nginx-conf

trap stop_all EXIT
stop_all
start_apps

# Mount the directory, not the file: sed -i replaces the file, a file mount would keep the old one
mkdir -p "$CONF"
cat >"$CONF/default.conf" <<NGINX
upstream app {
    server $BLUE;
    keepalive 64;
}
server {
    listen 8090;
    location / {
        proxy_pass http://app;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_set_header Host \$host;
    }
}
NGINX
# Host network: same path as the router, no extra Docker port-mapping hop. :Z for SELinux.
docker run -d --name bench-nginx --network host -v "$CONF:/etc/nginx/conf.d:Z" nginx:1.27 >/dev/null
wait_ready http://127.0.0.1:8090/

switch_nginx() {
    sed -i "s/server $BLUE;/server $GREEN;/" "$CONF/default.conf"
    docker exec bench-nginx nginx -s reload 2>/dev/null
}
attack nginx "$RUN" "$RATE" "$DURATION" http://127.0.0.1:8090/ "" switch_nginx
