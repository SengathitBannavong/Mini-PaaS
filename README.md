# Mini PaaS

A single-node Platform-as-a-Service: give it a Docker image and a domain name, and it runs the image as an app and routes HTTP traffic to it — including while switching to a new version, without dropping requests.

The core is an ingress **router written from scratch** in Rust (`tokio` + `hyper`). The goal is not to compete with real platforms but to understand how they work and to **measure** what happens during a version switch, compared with Nginx (`nginx -s reload`) and Traefik.

> **Status:** early development (week 7 of 14). The router works; the control plane, Docker integration, blue-green deploys and admin UI are planned. See [Roadmap](#roadmap).

This is a university project (Project III, IT3940, Hanoi University of Science and Technology). The full proposal is in [`docs/proposal.md`](docs/proposal.md).

## Quickstart

Requirements: Rust (edition 2024), Docker.

```sh
# 1. Build and start the test app (answers in 100–200 ms with its version name)
docker build -t testapp:v1 --build-arg VERSION=v1 minipaas
docker run -d --name app-v1 -p 18081:8080 testapp:v1

# 2. Start the router on 127.0.0.1:3000
#    (the route app.localhost -> 127.0.0.1:18081 is hard-coded in minipaas/src/main.rs for now)
cd minipaas && cargo run --release

# 3. Send a request through the router
curl -H 'Host: app.localhost' http://127.0.0.1:3000/
# v1
```

Run the tests:

```sh
cd minipaas && cargo test
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
- [ ] **M2** — Control plane deploys an app from a Docker image and registers its route automatically *(weeks 8–9)*
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
