#!/usr/bin/env bash
# One run against minipaas on :3000 (Host app.localhost): deploy v1 through the API,
# then deploy v2 SWITCH_AT seconds into the attack (blue-green, fixed wait until M4).
# Usage: bench/mine-run.sh [run] [rate] [duration]    e.g. bench/mine-run.sh 1 500 60s
set -euo pipefail
cd "$(dirname "$0")"
source ./common.sh
parse_args "$@"

export MINIPAAS_API_KEY=$(openssl rand -hex 32) MINIPAAS_DB=$RESULTS/mine.db
API=http://127.0.0.1:3001

# deploy IMAGE: blocks until the Deployment ends (fails the run if it fails)
deploy() {
    curl -sf -X POST "$API/apps/app.localhost/deploy" \
        -H "Authorization: Bearer $MINIPAAS_API_KEY" -H 'Content-Type: application/json' \
        -d "{\"image\":\"$1\"}" >>"$RESULTS/mine-deploy.log"
    echo >>"$RESULTS/mine-deploy.log"
}
switch_to_green() { deploy "$REGISTRY/testapp:v2"; }

trap stop_all EXIT
stop_all
push_images
rm -f "$MINIPAAS_DB" "$RESULTS/mine-deploy.log"

# Build first and run the binary directly: killing `cargo run` would leave the router running
cargo build --release --quiet --manifest-path ../minipaas/Cargo.toml
../minipaas/target/release/minipaas >"$RESULTS/router.log" 2>&1 &
ROUTER_PID=$!
for _ in $(seq 50); do curl -s -o /dev/null "$API/" && break; sleep 0.2; done  # any answer = API is up

deploy "$REGISTRY/testapp:v1"   # blue
wait_ready http://127.0.0.1:3000/ app.localhost

attack mine "$RUN" "$RATE" "$DURATION" http://127.0.0.1:3000/ app.localhost switch_to_green
