#!/usr/bin/env python3
"""Summarise bench runs from /tmp/bench.
Usage: bench/report.py                     CSV: one line per run, then median/min/max per system
       bench/report.py --seconds FILE.bin  per-second table and summary line for one run
       bench/report.py --save              write bench/results/<date_time>/ (see save())
failed = 5xx or broken connection; route_effective_ms = switch command -> first response
served by v2 (includes the test app's 100-200 ms delay).
"""
import base64, csv, glob, io, json, os, platform, statistics, subprocess, sys, tarfile, time
from collections import defaultdict

RESULTS = "/tmp/bench"
SAVE_ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "results")
FIELDS = ["system", "run", "rate", "requests", "failed", "p99_ms", "peak_p99_ms", "route_effective_ms"]
AGG_FIELDS = ["system", "runs", "stat", "failed", "peak_p99_ms", "route_effective_ms"]
SECOND_FIELDS = ["sec", "requests", "failed", "p99_ms", "v1", "v2"]


def load(bin_path):
    meta = json.load(open(bin_path[:-4] + ".json"))
    with open(bin_path, "rb") as f:
        out = subprocess.run(["vegeta", "encode", "--to", "csv"], stdin=f,
                             capture_output=True, check=True).stdout.decode()
    rows = [(int(ts), int(code), int(lat), base64.b64decode(body).decode().strip())
            for ts, code, lat, _o, _i, _e, body, *_ in csv.reader(out.splitlines())]
    return meta, rows


def is_failed(code):
    return code == 0 or code >= 500  # 0 = connection error


def p99(lats_ns):
    lats = sorted(lats_ns)
    return lats[min(len(lats) - 1, int(len(lats) * 0.99))] / 1e6


def per_second(rows):
    t0 = min(r[0] for r in rows)
    sec = defaultdict(list)
    for r in rows:
        sec[(r[0] - t0) // 10**9].append(r)
    return dict(sorted(sec.items()))


def seconds_table(rows):
    table = []
    for s, reqs in per_second(rows).items():
        bodies = [r[3] for r in reqs]
        table.append([s, len(reqs), sum(is_failed(r[1]) for r in reqs),
                      f"{p99([r[2] for r in reqs]):.1f}", bodies.count("v1"), bodies.count("v2")])
    return table


def summary(meta, rows):
    sw = meta["switch_ns"]
    v2_done = [ts + lat for ts, _, lat, body in rows if body == "v2"]
    if sw is None:
        effective = ""
    elif v2_done:
        effective = f"{(min(v2_done) - sw) / 1e6:.1f}"
    else:
        effective = "never"
    return [meta["system"], meta["run"], meta["rate"], len(rows),
            sum(is_failed(r[1]) for r in rows),
            f"{p99([r[2] for r in rows]):.1f}",
            f"{max(p99([r[2] for r in s]) for s in per_second(rows).values()):.1f}",
            effective]


def aggregate(summaries):
    by_system = defaultdict(list)
    for s in summaries:
        by_system[s[0]].append(dict(zip(FIELDS, s)))
    table = []
    for system, ds in by_system.items():
        for name, fn in [("median", statistics.median), ("min", min), ("max", max)]:
            row = [system, len(ds), name]
            for k in AGG_FIELDS[3:]:
                vals = [float(d[k]) for d in ds if d[k] not in ("", "never")]
                row.append(f"{fn(vals):g}" if vals else "")
            table.append(row)
    return table


def to_csv(header, table):
    buf = io.StringIO()
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(header)
    w.writerows(table)
    return buf.getvalue()


def to_md(header, table):
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    lines += ["| " + " | ".join(str(c) for c in row) + " |" for row in table]
    return "\n".join(lines) + "\n"


def sh(*cmd):
    try:
        return subprocess.run(cmd, capture_output=True, text=True).stdout.strip()
    except OSError:
        return ""


def machine():
    cpu = next((l.split(":", 1)[1].strip() for l in open("/proc/cpuinfo") if l.startswith("model name")), "")
    mem_kb = int(next(l.split()[1] for l in open("/proc/meminfo") if l.startswith("MemTotal")))
    return [
        ("OS", sh("cat", "/etc/fedora-release") or platform.platform()),
        ("Kernel", platform.release()),
        ("CPU", f"{cpu} ({os.cpu_count()} threads)"),
        ("RAM", f"{mem_kb / 1024**2:.1f} GiB"),
        ("Docker", sh("docker", "version", "--format", "{{.Server.Version}}")),
        ("nginx:1.27 image", sh("docker", "image", "inspect", "-f", "{{.Id}}", "nginx:1.27")),
    ]


def save(runs):
    """bench/results/<date_time>/: manifest.md, result.md, summary.csv, seconds/*.csv, raw.tar.gz"""
    out = os.path.join(SAVE_ROOT, time.strftime("%Y-%m-%d_%H%M"))
    os.makedirs(os.path.join(out, "seconds"), exist_ok=False)
    summaries = []
    for bin_path, meta, rows in runs:
        summaries.append(summary(meta, rows))
        name = f"{meta['system']}-{meta['run']}.csv"
        open(os.path.join(out, "seconds", name), "w").write(to_csv(SECOND_FIELDS, seconds_table(rows)))
    agg = aggregate(summaries)

    open(os.path.join(out, "summary.csv"), "w").write(
        to_csv(FIELDS, summaries) + "\n" + to_csv(AGG_FIELDS, agg))
    open(os.path.join(out, "result.md"), "w").write(
        f"# Bench result {os.path.basename(out)}\n\n"
        "failed = 5xx or broken connection. peak_p99_ms = worst per-second p99. "
        "route_effective_ms = switch command -> first response served by v2 "
        "(includes the test app's 100-200 ms delay).\n\n"
        "## Per system (median / min / max)\n\n" + to_md(AGG_FIELDS, agg) +
        "\n## Per run\n\n" + to_md(FIELDS, summaries) +
        "\nPer-second tables: `seconds/<system>-<run>.csv`.\n")

    metas = [m for _, m, _ in runs]
    commits = sorted({m["git_commit"] for m in metas})
    warn = []
    if len(commits) > 1:
        warn.append(f"runs come from different commits: {', '.join(commits)}")
    if any(c.endswith("-dirty") for c in commits):
        warn.append("some runs had uncommitted changes (-dirty)")
    first = metas[0]
    run_table = [[m["system"], m["run"], m["rate"], m["duration"], m["url"], m["host"] or "-",
                  "-" if m["switch_ns"] is None else f"+{(m['switch_ns'] - m['start_ns']) / 1e9:.2f}s",
                  m["git_commit"], f"bench/{m['system']}-run.sh {m['run']} {m['rate']} {m['duration']}"]
                 for m in metas]
    open(os.path.join(out, "manifest.md"), "w").write(
        f"# Bench manifest {os.path.basename(out)}\n\n" +
        "".join(f"> **Warning:** {w}\n>\n" for w in warn) + ("\n" if warn else "") +
        "## Software\n\n" + to_md(["item", "value"], [
            ["git commit", ", ".join(commits)],
            ["testapp:v1", first["testapp_v1"]],
            ["testapp:v2", first["testapp_v2"]],
            ["vegeta", first["vegeta"]],
        ]) +
        "\n## Machine (captured when saving)\n\n" + to_md(["item", "value"], machine()) +
        "\n## Runs\n\n" + to_md(["system", "run", "rate", "duration", "url", "host", "switch",
                                 "commit", "command"], run_table) +
        "\nRaw vegeta results and per-run manifests: `raw.tar.gz` (not committed).\n")

    with tarfile.open(os.path.join(out, "raw.tar.gz"), "w:gz") as tar:
        for bin_path, _, _ in runs:
            tar.add(bin_path, arcname=os.path.basename(bin_path))
            tar.add(bin_path[:-4] + ".json", arcname=os.path.basename(bin_path)[:-4] + ".json")
    return out


if sys.argv[1:2] == ["--seconds"]:
    meta, rows = load(sys.argv[2])
    print(to_csv(SECOND_FIELDS, seconds_table(rows)))
    print(to_csv(FIELDS, [summary(meta, rows)]), end="")
    sys.exit()

paths = []
for p in sorted(glob.glob(f"{RESULTS}/*.bin")):
    if os.path.exists(p[:-4] + ".json"):
        paths.append(p)
    else:  # an aborted run leaves results without a manifest
        print(f"skipping {p}: no manifest {os.path.basename(p)[:-4]}.json", file=sys.stderr)
if not paths:
    sys.exit(f"no runs in {RESULTS}")
runs = sorted(((p, *load(p)) for p in paths), key=lambda r: (r[1]["system"], r[1]["run"]))

if sys.argv[1:2] == ["--save"]:
    print(f"saved to {save(runs)}")
    sys.exit()

summaries = [summary(m, r) for _, m, r in runs]
print(to_csv(FIELDS, summaries))
print(to_csv(AGG_FIELDS, aggregate(summaries)), end="")
