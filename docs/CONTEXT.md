# Mini PaaS — Glossary

A single-node PaaS: takes a Docker image, runs it as an application and routes HTTP traffic to it, including during a version switch.

Each term gives the English word used in code and docs, then the Vietnamese word used in the report (in parentheses). Words under _Avoid_ mean something else or are too vague; don't use them for this concept.

## Language

**App** (_Ứng dụng_):
A service deployed by the user, identified by a domain name (`Host`). An **App** has one or more **Versions**, of which at most one receives traffic.
_Avoid_: service, project

**Version** (_Phiên bản_):
A Docker image of an **App**, identified by its digest, not its tag, and running as one or more **Replicas**. Two deploys of the same tag with different digests are two different **Versions**.
_Avoid_: release, build

**Replica** (_Replica_):
One running container of a **Version**, reached by the **Router** at its IP on Docker's `bridge` network (see ADR 0002).
_Avoid_: backend, instance, upstream

**Deployment** (_Lần triển khai_):
One deploy command for an **App**: pull the image, start the new **Version**, switch traffic if it becomes **Ready**. Recorded as one row in SQLite with status `running`, `succeeded` or `failed`. Only one Deployment runs at a time across all Apps.
_Avoid_: release, rollout

**Ready** (_Sẵn sàng_):
State of a **Replica** that has answered an HTTP health check successfully. Only **Ready** Replicas are put into a **Route**.
_Avoid_: healthy, up, alive

**Route** (_Route_):
Mapping from one `Host` to the set of **Replicas** of the **Version** currently receiving traffic.
_Avoid_: rule, mapping

**Routing table** (_Bảng định tuyến_):
All **Routes** at one point in time; always replaced as a whole.

**Router** (_Bộ định tuyến_):
The component that receives HTTP requests and forwards each one to a **Replica** according to the **Routing table**.
_Avoid_: gateway, proxy, ingress

**Control plane** (_Control plane_):
The component that receives deploy commands, drives containers and replaces the **Routing table**.

**Version switch** (_Chuyển phiên bản_):
Moving an **App**'s traffic from the old **Version** (blue) to the new one (green). Ends when the old Version has finished its **In-flight requests**.
_Avoid_: deploy (when talking only about the traffic switch), switch

**Draining** (_Draining_):
Last stage of a **Version switch**: the old Version gets no new requests and finishes its **In-flight requests**.

**In-flight request** (_Request đang xử lý_):
A request the **Router** has forwarded to a **Replica** whose response has not been fully returned yet.
_Avoid_: pending

**Route-effective time** (_Thời gian route có hiệu lực_):
Time from the moment the **Control plane** receives a deploy command until the first request is served by the new **Version**.

**Failed request** (_Request lỗi_):
A request sent during a **Version switch** for which the client gets a 5xx status or a broken connection.

**Synthetic load** (_Tải tổng hợp_):
Automatically generated traffic used for measurement.

## Flagged ambiguities

- "Simulation" (_mô phỏng_) — this system does **not** simulate. Containers, HTTP connections and version switches are all real. Only the load used for measuring is generated; call it **Synthetic load**, not simulation.
- The code still uses `Backend` for **Replica** and `Gateway` for the **Router**'s state. The glossary terms win; rename when touching that code.
