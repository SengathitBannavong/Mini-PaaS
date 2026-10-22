# Shared by every <system>-run.sh. Each run sets everything up, attacks, tears it down and
# leaves two files in /tmp/bench: <system>-<run>.bin (vegeta results) and <system>-<run>.json
# (manifest: settings, switch time, git commit, image ids). report.py reads both.

RESULTS=/tmp/bench
BLUE=127.0.0.1:18081     # testapp:v1
GREEN=127.0.0.1:18082    # testapp:v2
SWITCH_AT=20             # seconds into the attack
mkdir -p "$RESULTS"

# parse_args "$@": sets RUN RATE DURATION, prints the script's usage on -h/--help or bad input
parse_args() {
    local usage
    usage=$(grep '^# Usage:' "$(basename "$0")" | sed 's/^# //')  # scripts cd into bench/ first
    case "${1:-}" in -h|--help) echo "$usage"; exit 0 ;; esac
    RUN=${1:-1} RATE=${2:-500} DURATION=${3:-60s}
    [[ $RUN =~ ^[0-9]+$ && $RATE =~ ^[0-9]+$ && $DURATION =~ ^[0-9]+[smh]$ ]] \
        || { echo "bad arguments: $*" >&2; echo "$usage" >&2; exit 1; }
}

start_apps() {
    docker run -d --name bench-blue  -p 18081:8080 testapp:v1 >/dev/null
    docker run -d --name bench-green -p 18082:8080 testapp:v2 >/dev/null
}

stop_all() {
    if [ -n "${ROUTER_PID:-}" ]; then kill "$ROUTER_PID" 2>/dev/null || true; fi
    docker rm -f bench-blue bench-green bench-nginx >/dev/null 2>&1 || true
}

# wait_ready URL [HOST]
wait_ready() {
    for _ in $(seq 50); do
        curl -sf -o /dev/null ${2:+-H "Host: $2"} "$1" && return 0
        sleep 0.2
    done
    echo "$1 not ready" >&2
    exit 1
}

# attack SYSTEM RUN RATE DURATION URL HOST [SWITCH_FN]
# HOST may be empty. SWITCH_FN, if given, is called SWITCH_AT seconds into the attack.
attack() {
    local system=$1 run=$2 rate=$3 duration=$4 url=$5 host=$6 switch_fn=${7:-}
    local out=$RESULTS/$system-$run switch_ns=null start_ns pid
    local header=()
    [ -n "$host" ] && header=(-header "Host: $host")

    echo "$system run $run: $rate req/s for $duration${switch_fn:+, switch at second $SWITCH_AT}"
    start_ns=$(date +%s%N)
    echo "GET $url" | vegeta attack -rate="$rate" -duration="$duration" "${header[@]}" >"$out.bin" &
    pid=$!
    if [ -n "$switch_fn" ]; then
        sleep "$SWITCH_AT"
        switch_ns=$(date +%s%N)
        "$switch_fn"
    fi
    wait "$pid"

    cat >"$out.json" <<JSON
{
  "system": "$system",
  "run": $run,
  "rate": $rate,
  "duration": "$duration",
  "url": "$url",
  "host": "$host",
  "start_ns": $start_ns,
  "switch_ns": $switch_ns,
  "git_commit": "$(git rev-parse --short HEAD)$(git diff --quiet || echo -dirty)",
  "testapp_v1": "$(docker image inspect -f '{{.Id}}' testapp:v1)",
  "testapp_v2": "$(docker image inspect -f '{{.Id}}' testapp:v2)",
  "vegeta": "$(vegeta -version | head -1)"
}
JSON
    ./report.py --seconds "$out.bin"
}
