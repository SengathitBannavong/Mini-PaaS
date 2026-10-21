---
status: accepted
---

# Router and control plane run in one process

The router and the control plane run in the same process and share the in-memory routing table; each deployment replaces the table in a single atomic operation. This removes the need for a synchronisation protocol between two processes, guarantees that every request sees either the old table or the new one (no intermediate state during a blue-green switch), and keeps the time until a route takes effect low.

## Considered Options

- **Two separate processes** (the control plane pushes configuration to the router): better fault isolation, but requires designing a synchronisation protocol and handling the case where the two sides disagree on state; beyond the time available for the project.

## Consequences

- If the control plane crashes, the router stops with it. Acceptable for a single-node scope; stated as a limitation in the report.
