# PROJECT III PROPOSAL AND PLAN (IT3940)

*English translation. The original Vietnamese version submitted to the supervisor is [proposal.vi.md](proposal.vi.md); if the two differ, the Vietnamese version is authoritative.*

| | |
|---|---|
| **Student** | Seng Athit Bannavong |
| **Student ID** | 20239717 |
| **Class** | IT1-04 — Computer Science K68 |
| **Supervisor** | Dr. Nguyễn Đức Anh |
| **Semester** | 2026.1 |

## 1. Title

**Building a mini PaaS with a custom ingress router and zero-downtime deployment** (Mini PaaS)

## 2. Short description

A PaaS (Platform as a Service) lets developers put applications on the Internet without configuring servers themselves. This project builds a small PaaS running on one server, whose core component is a custom-built ingress router.

Users work through a web admin UI or through the control plane API (from the command line or CI): create apps, deploy new versions and follow version-switch progress in real time.

## 3. Goals

The aim is not to compete with commercial products, but to understand the basic mechanisms of cloud infrastructure and to quantitatively evaluate the system's behaviour during a version switch, compared with Nginx and Traefik.

**Main goals**

| ID | Goal |
|:---|:----------------------------------------------------------------|
| M1 | HTTP router that routes by the `Host` header; routing table updated dynamically without restarting the process |
| M2 | Control plane that deploys apps from Docker images and registers routes automatically |
| M3 | Active health checks and load balancing across multiple replicas |
| M4 | Zero-downtime deployment using blue-green combined with connection draining |
| M5 | Quantitative evaluation of the version switch under load: failed requests, p99 latency and time from the deploy command until the route takes effect; compared with `nginx -s reload` and Traefik (Docker provider) |
| M6 | Web admin UI to create, deploy and monitor apps, showing version-switch progress in real time (SSE) |
| M7 | Minimal authentication protecting the control plane: login session for browsers and API key for automated processes |

**Stretch goals (if time allows)**

| ID | Goal |
|:---|:----------------------------------------------------------------|
| E1 | TLS via ACME using an existing library |
| E2 | Deploy the admin UI as an app on the system itself |

## 4. System overview

![System overview](images/system-overview.svg)

*Note: running the admin UI as an app on the Docker host belongs to stretch goal E2; in the main scope the UI is served by the control plane.*

## 5. Scope

**In scope**

- One node.
- HTTP/1.1.
- Web admin UI, served by the control plane as static files.
- Minimal authentication for one admin account.
- Measurements run on the local machine, not through Cloudflare Tunnel.

**Deployment input**

- Pre-built Docker images pulled from a registry (Docker Hub / GHCR).
- Building images from a Dockerfile is done by external CI (GitHub Actions) and is outside the system's scope.
- For each deploy the user provides: image name, domain (`Host`), container port and health-check path.
- Container port and health-check path are optional: if empty, the system takes the port from the image's `EXPOSE` declaration and uses the path `/`.
- The host-side port is assigned automatically by Docker for each container, so ports never clash between apps or between two versions of the same app.
- The image is pulled again on every deploy and the version is recorded by digest.
- Health checks send an HTTP request, not just a TCP connect.
- The routing table changes only when the new version is ready. If the new version does not pass the health check within 60 seconds, the old version keeps serving, the new container is removed and the deploy is recorded as failed.
- Connection draining: the router counts in-flight requests per replica; after the route switches, the old version receives no new requests and is removed only when the counter reaches 0 or after at most 30 seconds. Requests cut by the timeout count as failed requests.

**Authentication**

- One admin account, password hashed with `argon2`.
- Session stored in an `HttpOnly` cookie.
- Separate API key for CI and command-line tools.
- The API key is not embedded in browser code.

**Out of scope**

- Building images from source code.
- Private registries (would require storing registry credentials).
- Automatic rollback after a version switch.
- WebSocket and other protocol-upgrade connections.
- Multi-node clusters.
- Network isolation between tenants.
- Paid public service.
- Path-prefix routing.
- Multiple users, role-based access, third-party login.

## 6. Evaluation method

The main experiment measures the system's behaviour during a version switch under load (M5).

| Item | Setting |
|:-----------------|:----------------------------------------------|
| Load | Fixed rate, about 500 requests/second for 60 seconds |
| Switch time | Second 20 of each run |
| Test application | Deliberately slow responses (100–200 ms) so there are always in-flight requests at the switch; the response tells which version served it |
| Repetitions | 10 runs per system; report median and min–max range |
| Metrics | Failed requests (5xx or broken connection), p99 latency per second, route-effective time |
| Systems compared | This project's router; Nginx (edit config then `nginx -s reload`); Traefik (Docker provider) |
| Environment | Same machine, same set of containers, measured locally without Cloudflare Tunnel |

Load and repetition numbers are starting values and will be adjusted to the capacity of the test machine. Results are reported exactly as measured, even if a reference system performs equally well or better.

## 7. Technology

| Item | Choice |
|:-----------------|:----------------------------------------------|
| Language | Rust (`tokio`, `hyper`, `axum`) |
| Containers | Docker Engine API (`bollard`) |
| State storage | SQLite |
| Admin UI | Vite + React + TypeScript |
| Measurement baselines | Nginx, Traefik |
| Infrastructure | Personal machine exposed publicly via Cloudflare Tunnel |

## 8. Schedule

| Phase | Weeks | Work | Result |
|:--------|:------|:-------------------------------------|:------------------------|
| 1 | 1–6 (done) | Survey reverse-proxy architecture, self-hosted PaaS platforms and zero-downtime deployment methods. Reverse-proxy prototype: path-prefix routing, round-robin load balancing, TCP health check | Prototype running with multiple backends |
| 2 | 7 | Fix prototype bugs (health check, header forwarding), switch to `Host`-header routing, routing table updatable at runtime, basic load-testing tool. Install Docker Engine | Dynamic router, baseline numbers (M1, M3) |
| 3 | 8–9 | Control plane API (`axum`), container control via Docker Engine API (`bollard`), state in SQLite, API key | Deploy apps from Docker images via API, routes registered automatically (M2) |
| 4 | 10–11 | Blue-green deploy with connection draining, measure failed requests during the switch, SSE status stream. In parallel: admin UI on top of the existing API, cookie-session login (M4 takes priority if short on time) | Version switch under load without dropped requests (M4), login and app list in the UI (M7) |
| 5 | 12 | Benchmark against `nginx -s reload` and Traefik, finish UI (deploy button, version-switch status bar), expose the system via Cloudflare Tunnel | Measurement dataset, demo on a public URL (M5, M6) |
| 6 | 13–14 | Analyse results, finish report and slides | Report, defence slides |
