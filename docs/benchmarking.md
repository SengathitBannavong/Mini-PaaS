# Benchmarking

How to load-test the router, and how the main experiment (goal **M5**) is set up.

## Requirements

- Docker
- Rust toolchain
- [vegeta](https://github.com/tsenart/vegeta)
- `python3` (standard library only)

## Steady-load test (available now)

Measures the router under constant load, **without** a version switch. This gives the baseline numbers.

```sh
docker build -t testapp:v1 --build-arg VERSION=v1 minipaas
bench/vegeta-test.sh 500 60s     # rate (req/s), duration
```

The script:

1. starts `testapp:v1` as container `bench-blue` on `127.0.0.1:18081`
2. builds and starts the router (release build) on `127.0.0.1:3000`, log in `/tmp/router.log`
3. waits until `Host: app.localhost` answers
4. attacks at a fixed rate and saves the raw results to `/tmp/vegeta-<timestamp>.bin`
5. prints vegeta's summary, then a per-second table from `bench/report-vegeta.py`
6. stops the router and removes the container on exit

Per-second table columns:

| Column | Meaning |
|---|---|
| `sec` | Second since the first request |
| `requests` | Requests sent in that second |
| `errors` | **Failed requests**: status ≥ 500 or connection error (vegeta code `0`) |
| `p99_ms` | 99th-percentile latency in ms |
| `v1`, `v2` | How many responses came from each Version (the test app answers with its version name) |

The test app always takes 100–200 ms, so latencies below 100 ms mean something is wrong.

## M5 experiment: version switch under load (planned)

| Item | Setting |
|---|---|
| Load | Fixed rate, ~500 req/s for 60 s |
| Switch | At second 20 of each run |
| Test app | `testapp:v1` → `testapp:v2` (slow on purpose so requests are in flight during the switch) |
| Runs | 10 per system; report median and min–max |
| Metrics | Failed requests, p99 per second, **Route-effective time** |
| Systems | This router; Nginx (edit config, `nginx -s reload`); Traefik (Docker provider) |
| Environment | Same machine, same containers, local only (not through Cloudflare Tunnel) |

Load and run counts are starting values and may change to fit the test machine. Results are reported as measured, even if Nginx or Traefik do as well or better.

Still to build: the switch step in the script (start `v2`, trigger the switch at second 20) and the Nginx/Traefik variants.

## Results

None recorded yet. Each result should note: date, commit hash, machine (CPU, RAM, OS), rate and duration.
