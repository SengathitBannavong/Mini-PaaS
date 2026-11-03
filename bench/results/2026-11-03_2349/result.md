# Bench result 2026-11-03_2349

failed = 5xx or broken connection. peak_p99_ms = worst per-second p99. route_effective_ms = switch command -> first response served by v2 (includes the test app's 100-200 ms delay).

## Per system (median / min / max)

| system | runs | stat | failed | peak_p99_ms | route_effective_ms |
|---|---|---|---|---|---|
| mine | 1 | median | 0 | 201.7 | 707.8 |
| mine | 1 | min | 0 | 201.7 | 707.8 |
| mine | 1 | max | 0 | 201.7 | 707.8 |
| nginx | 1 | median | 0 | 201.7 | 276.1 |
| nginx | 1 | min | 0 | 201.7 | 276.1 |
| nginx | 1 | max | 0 | 201.7 | 276.1 |

## Per run

| system | run | rate | requests | failed | p99_ms | peak_p99_ms | route_effective_ms |
|---|---|---|---|---|---|---|---|
| mine | 1 | 500 | 30000 | 0 | 200.8 | 201.7 | 707.8 |
| nginx | 1 | 500 | 30000 | 0 | 200.6 | 201.7 | 276.1 |

Per-second tables: `seconds/<system>-<run>.csv`.
