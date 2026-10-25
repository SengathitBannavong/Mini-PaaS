---
status: accepted
---

# Route to the container IP on the default bridge network

The router sends traffic straight to each Replica's IP on Docker's default `bridge` network (`172.17.x.x:<container port>`). Containers publish no ports. Each container has its own IP, so ports never clash between Apps or between two Versions of one App, and Apps can't be reached from outside the host except through the router.

Publishing a random host port and routing to `127.0.0.1:<host port>` makes every request pass through `docker-proxy`, an extra userland hop (checked on Docker Engine 29.8.2: one `docker-proxy` process per published port). Traefik's Docker provider also routes to the container IP, so this keeps the M5 comparison fair.

## Considered Options

- **Published random host port** (the original proposal): works on Docker Desktop (Mac/Windows), but adds the `docker-proxy` hop and needs port bindings plus reading the port back.
- **A dedicated network (`minipaas`)**: isolates Apps from other containers on the host. Not needed for one admin; it can be added later by setting `network_mode` in `HostConfig`.

## Consequences

- Linux only: the router must run on the Docker host itself (or on the same Docker network). On Docker Desktop the container IP can't be reached from the host.
- Apps on `bridge` can reach each other by IP. Acceptable for a single-admin scope; stated as a limitation in the report.
- Replica addresses are built in one function (`replica_url`), so moving to published ports (Mac/Windows) or a remote Docker host changes only that function.
