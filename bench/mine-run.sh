#!/usr/bin/env bash
# One run against the project's router on :3000 (Host app.localhost -> blue).
# No version switch yet: the router gets a deploy API with the control plane.
# Usage: bench/mine-run.sh [run] [rate] [duration]    e.g. bench/mine-run.sh 1 500 60s
set -euo pipefail
cd "$(dirname "$0")"
source ./common.sh
parse_args "$@"

trap stop_all EXIT
stop_all
start_apps

# Build first and run the binary directly: killing `cargo run` would leave the router running
cargo build --release --quiet --manifest-path ../router/Cargo.toml
../router/target/release/router >"$RESULTS/router.log" 2>&1 &
ROUTER_PID=$!
wait_ready http://127.0.0.1:3000/ app.localhost

attack mine "$RUN" "$RATE" "$DURATION" http://127.0.0.1:3000/ app.localhost
