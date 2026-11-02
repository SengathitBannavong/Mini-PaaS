# Mini PaaS

A single-node Platform-as-a-Service: give it a Docker image and a domain name, and it runs the image as an app and routes HTTP traffic to it — including while switching to a new version, without dropping requests.

The core is an ingress **router written from scratch** in Rust (`tokio` + `hyper`). The goal is not to compete with real platforms but to understand how they work and to **measure** what happens during a version switch, compared with Nginx (`nginx -s reload`) and Traefik.

> **Status:** week 9 of 14. The router and the control plane work: deploy an image through the API and it is reachable through the router. Connection draining, the admin UI and the measurements are next. See [Roadmap](#roadmap).

This is a university project (Project III, IT3940, Hanoi University of Science and Technology). The full proposal is in [`docs/proposal.md`](docs/proposal.md).

## Quickstart

Requirements: Linux with Docker Engine (the router reaches containers by their IP, so Docker Desktop on Mac/Windows does not work — [ADR 0002](docs/adr/0002-route-to-container-ip-on-bridge.md)), Rust (edition 2024).

```sh
# 1. Start minipaas: router on 127.0.0.1:3000, control plane API on 127.0.0.1:3001
export MINIPAAS_API_KEY=$(openssl rand -hex 32)   # required, at least 32 chars
cd minipaas && cargo run --release
```

In a second terminal (export the same `MINIPAAS_API_KEY` there):

```sh
API=http://127.0.0.1:3001
AUTH="Authorization: Bearer $MINIPAAS_API_KEY"

# 2. Deploy an image for the domain app.localhost (answers when the deploy has finished)
curl -X POST "$API/apps/app.localhost/deploy" -H "$AUTH" -H 'Content-Type: application/json' \
     -d '{"image":"traefik/whoami:latest"}'
# {"digest":"traefik/whoami@sha256:…","replicas":["http://172.17.0.2:80"]}

# 3. Send a request through the router
curl -H 'Host: app.localhost' http://127.0.0.1:3000/

# 4. Deploy again with 2 Replicas: blue-green switch, requests are spread over both
curl -X POST "$API/apps/app.localhost/deploy" -H "$AUTH" -H 'Content-Type: application/json' \
     -d '{"image":"traefik/whoami:latest","replicas":2}'

# 5. See the routing table
curl -H "$AUTH" "$API/apps"
```

Deploy request body:

| Field | Required | Default |
|---|---|---|
| `image` | yes | — (must be in a registry: the Version is recorded by digest) |
| `port` | no | the image's single `EXPOSE` port |
| `health_path` | no | `/` |
| `replicas` | no | `1` |

A deploy that does not become Ready within 60 s is rolled back: the running Version keeps serving and the new containers are removed. Deploy history is kept in SQLite (`minipaas.db`, or the path in `MINIPAAS_DB`):

```sh
sqlite3 minipaas/minipaas.db 'select id, image, digest, status, error from deployments'
```

Stop with Ctrl-C. Deployed containers keep running; on the next start minipaas rebuilds the routes from them. To remove them: `docker ps -aq --filter label=minipaas.host | xargs -r docker rm -f`.

Run the tests:

```sh
cd minipaas && cargo test
cargo test -- --ignored   # also the tests that need Docker and network
```

Load test (needs [vegeta](https://github.com/tsenart/vegeta) and `python3`): see [`docs/benchmarking.md`](docs/benchmarking.md).

## Repository layout

```
minipaas/          Rust crate: router + control plane in one process
  src/             router/ and control/ modules, see ARCHITECTURE.md for a file-by-file map
  examples/        backend.rs: slow test app used as a Replica
  Dockerfile       builds the test app image (testapp:vN)
bench/             load-test scripts (vegeta + per-second report)
docs/
  CONTEXT.md       glossary — the words this project uses and what they mean
  proposal.md      project proposal (English); proposal.vi.md is the Vietnamese original
  benchmarking.md  how to run load tests and the M5 experiment
  adr/             architecture decision records
  images/          diagrams (SVG)
```

## Documentation

| Document | Read it for |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the system fits together, what is built and what is planned |
| [docs/CONTEXT.md](docs/CONTEXT.md) | Exact meaning of App, Version, Replica, Route, Draining, … |
| [docs/adr/](docs/adr/) | Why key design decisions were made |
| [docs/benchmarking.md](docs/benchmarking.md) | Load tests and the version-switch experiment |
| [docs/proposal.md](docs/proposal.md) | Goals, scope, evaluation method and schedule |

## Roadmap

Main goals (from the [proposal](docs/proposal.md#3-goals)):

- [x] **M1** — Router that routes by the `Host` header; routing table can change at runtime without restarting the process
- [x] **M2** — Control plane deploys an app from a Docker image and registers its route automatically *(weeks 8–9)*
- [x] **M3** — Active HTTP health checks and load balancing (round-robin) across Replicas
- [ ] **M4** — Zero-downtime blue-green deploy with connection draining *(weeks 10–11)*
- [ ] **M5** — Measure the version switch under load: failed requests, p99 latency, time until the route takes effect; compare with Nginx and Traefik *(week 12)*
- [ ] **M6** — Web admin UI: create, deploy and watch apps; live version-switch progress over SSE *(weeks 10–12)*
- [ ] **M7** — Minimal auth for the control plane: browser session cookie + API key for CI *(weeks 10–11)*

Stretch goals:

- [ ] **E1** — TLS via ACME using an existing library
- [ ] **E2** — Run the admin UI as an app on the platform itself

Known limitations by design: single node, HTTP/1.1 only, no WebSocket, router and control plane share one process (see [ADR 0001](docs/adr/0001-router-and-control-plane-in-one-process.md)).

## License

[MIT](LICENSE)
