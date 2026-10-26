# Architecture

This document describes the **target** architecture of Mini PaaS. Each part is marked:

- ✅ **implemented** — exists in the code today
- 🚧 **planned** — designed but not written yet

Terms in **bold** (App, Version, Replica, Route, …) have exact meanings defined in [docs/CONTEXT.md](docs/CONTEXT.md).

## Bird's-eye view

![System overview](docs/images/system-overview.svg)

Mini PaaS runs on **one Linux host with Docker Engine** as **one process** that contains two parts sharing memory:

| Part | Job | Status |
|---|---|---|
| **Router** | Accepts HTTP requests on `:3000`, finds the **Route** for the request's `Host`, forwards it to a **Ready** **Replica** | ✅ |
| **Control plane** | HTTP API (`axum`) on `127.0.0.1:3001`: receives deploy commands, drives Docker, stores state in SQLite, replaces the **Routing table**, streams progress over SSE, serves the admin UI | 🚧 |

They share the **Routing table** in memory instead of talking over a protocol. Why: [ADR 0001](docs/adr/0001-router-and-control-plane-in-one-process.md).

External pieces:

| Piece | Used for | Status |
|---|---|---|
| Docker Engine (via `bollard`) | Pull images, run/remove containers on the default `bridge` network (no published ports, [ADR 0002](docs/adr/0002-route-to-container-ip-on-bridge.md)); container labels are the source of truth for running Replicas ([ADR 0003](docs/adr/0003-docker-labels-are-the-source-of-truth.md)) | 🚧 |
| SQLite | History of **Deployments** only (one `deployments` table) | 🚧 |
| Admin UI (Vite + React + TS) | Static files served by the control plane | 🚧 |
| Cloudflare Tunnel | Public URL for the demo (not used for measurements) | 🚧 |

## Request path ✅

What happens to one request, in order (`minipaas/src/main.rs`, `handle_request`):

1. **Accept** — a `tokio` task per TCP connection, served by `hyper` HTTP/1.1.
2. **Middleware (request side)** — each middleware may modify the request or answer early. Today: logging, add `X-Gateway` header.
3. **Find Route** — `Host` header, port stripped, lower-cased (`Users.Localhost:3000` → `users.localhost`). No Route → `404`.
4. **Pick Replica** — round-robin over Replicas currently marked Ready. None Ready → `503`.
5. **Forward** — rewrite the URI to the Replica's address (path and query unchanged), drop hop-by-hop headers (`Connection`, `Upgrade`, …), set `X-Forwarded-For`, keep all other client headers (`Cookie`, `Authorization`, …). Uses a pooled keep-alive client. Replica unreachable → `502`.
6. **Middleware (response side)** — in reverse order. Today: logging with latency, add `X-Powered-By` header.

## Routing table ✅ (partly)

```
Router
└── RwLock<HashMap<host, Arc<Route>>>
    └── Route { host, health_path, RwLock<Vec<Backend>>, round-robin counter }
```

- A request clones the `Arc<Route>` under a short read lock, then works on that snapshot. Replacing a host's Route therefore never shows a request a half-updated Route: it sees the old one or the new one.
- Today `Router::set_route` replaces **one host at a time**. 🚧 The design in CONTEXT.md says the whole table is replaced at once; this becomes needed if one deploy ever touches more than one host.
- 🚧 Routes are hard-coded in `main.rs` until the control plane exists. On start-up the control plane will rebuild the table from the labels (`minipaas.host`, `minipaas.port`, `minipaas.health_path`) of the containers still running; per host it keeps the Version of the newest container and removes the rest ([ADR 0003](docs/adr/0003-docker-labels-are-the-source-of-truth.md)).

## Health checks ✅

A background task (`minipaas/src/router/health.rs`) runs every 10 s. For each Replica it sends `GET <health_path>` (default `/`) and marks it Ready only on a `2xx` within 3 s. A plain TCP connect is not enough. Replicas that are not Ready are skipped in step 4 above.

## Deploy and version switch 🚧

![Blue-green version switch](docs/images/version-switch.svg)

A deploy of an App that is already running (blue) to a new Version (green):

1. **Pull** the image again and record its **digest** — the digest, not the tag, identifies the Version.
2. **Start green** next to blue: `replicas` containers (default 1) on the `bridge` network, labelled with host, port and health path. The router reaches each one at `http://<container IP>:<container port>`, so the two Versions never clash. Container port and health path default to the image's `EXPOSE` and `/`.
3. **Wait for Ready** — check every green Replica every 100 ms; all must be Ready within 60 s.
   - Fails → blue keeps serving, green is removed, the deploy is recorded as failed, the Routing table is **not** touched.
4. **Swap the Route** atomically to green.
5. **Drain blue** — blue gets no new requests; the router counts **In-flight requests** per Replica. Blue is removed when its count reaches 0 or after 30 s; requests cut by the timeout count as **Failed requests**. Until M4, blue is removed after a fixed 5 s wait.

Rules:
- One Deployment at a time across all Apps (one global lock).
- The deploy runs in its own task (`tokio::spawn`), so a client that disconnects does not stop it half-way.
- Each Deployment is one row in SQLite: `running` → `succeeded` / `failed`. Rows still `running` at start-up become `failed` (`interrupted`).
- `POST /apps/{host}/deploy` answers when the Deployment ends. With SSE (M6) it will return an id at once and push progress instead.

## Control plane API and auth 🚧

- API key for CI and CLI (weeks 8–9): read from env `MINIPAAS_API_KEY` (the process refuses to start without it), sent as `Authorization: Bearer <key>`, compared in constant time. The key is never shipped in browser code.
- One admin account, password hashed with `argon2`, browser session in an `HttpOnly` cookie (weeks 10–11).

## Code map

Only files that exist today. Paths are relative to `minipaas/`.

| File | What it does |
|---|---|
| `src/main.rs` | Start-up (hard-coded routes, health checks, middleware), accept loop, `handle_request`; end-to-end test that routes by Host and swaps a Route at runtime |
| `src/router/mod.rs` | `Router` (routing table), `Route` (Replicas + round-robin), `Backend` (one Replica) |
| `src/router/proxy.rs` | Pooled HTTP client and `forward_request` (URI rewrite, hop-by-hop headers, `X-Forwarded-For`, `502`) |
| `src/router/health.rs` | Periodic HTTP health checks |
| `src/router/middleware/mod.rs` | `Middleware` trait and `MiddlewarePipeline` |
| `src/router/middleware/logging.rs` | Per-request log lines with latency |
| `src/router/middleware/headers.rs` | Add fixed request/response headers |
| `examples/backend.rs` | Test app: answers with its name after 100–200 ms, so requests are always in flight during a switch |
| `Dockerfile` | Packs `examples/backend.rs` as `testapp:<VERSION>` |
| `../bench/` | vegeta load test and per-second report, see [docs/benchmarking.md](docs/benchmarking.md) |

Naming note: the code says `Backend` where the docs say **Replica**, and `Gateway` for the **Router** state. The docs' words are the ones to keep.

## Invariants

These must stay true; break one only with a new ADR.

- A request sees either the old Route or the new one, never a mix.
- The Routing table changes only after the new Version is **Ready**.
- A failed deploy never affects the Version currently serving.
- The router never strips a path prefix or rewrites the path.
- The router never forwards hop-by-hop headers.

## Limitations

- Single node; if the process crashes, both router and control plane go down ([ADR 0001](docs/adr/0001-router-and-control-plane-in-one-process.md)).
- Linux only: the router reaches containers by IP, which Docker Desktop (Mac/Windows) does not allow ([ADR 0002](docs/adr/0002-route-to-container-ip-on-bridge.md)).
- Apps share Docker's `bridge` network and can reach each other by IP.
- HTTP/1.1 only, no WebSocket or other protocol upgrades.
- Requests longer than the 30 s drain window are cut during a version switch.
- Out of scope: building images from source, private registries, automatic rollback after a switch, multi-tenant network isolation, path-prefix routing, multiple users.
