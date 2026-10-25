---
status: accepted
---

# Docker labels are the source of truth for running Replicas; SQLite keeps history

Each container the Control plane creates carries the labels `minipaas.host`, `minipaas.port` and `minipaas.health_path`. When the process starts, it lists containers by the `minipaas.host` label and rebuilds the Routing table from them. SQLite stores only the history of Deployments (one row per deploy command: image, digest, status, error, times).

The running containers are the real state. If the Routing table were rebuilt from SQLite, a row could say "running" for a container that has already died, and the two would have to be kept in sync by hand.

## Considered Options

- **Everything in SQLite** (tables for Apps, Versions and Replicas): one place to query, but it duplicates what Docker already knows and can drift after a crash or a manual `docker rm`.

## Consequences

- If the process dies in the middle of a Version switch, both blue and green may still be running. On start, for each `Host`, the Control plane keeps the Version (the container's `image`, which is the digest) of the newest container and removes the others. Green may get traffic before it is Ready; the router's health check takes it out within one interval.
- On start, Deployments still marked `running` in SQLite are marked `failed` with error `interrupted`.
- Removing a container by hand (`docker rm`) also removes it from the Routing table after the next restart, without touching SQLite.
