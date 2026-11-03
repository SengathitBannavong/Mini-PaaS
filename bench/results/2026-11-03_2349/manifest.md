# Bench manifest 2026-11-03_2349

## Software

| item | value |
|---|---|
| git commit | 2c287f4 |
| testapp:v1 | sha256:910879d5715b9b8c14c05a5dfe87e8f9179184ed86c24eec5ead19608c157c34 |
| testapp:v2 | sha256:6c966dfc15b5210e4642734c55e8ea642112d98eb252b6a23b79c0a9e4077a64 |
| vegeta | Version: v12.12.0 |

## Machine (captured when saving)

| item | value |
|---|---|
| OS | Fedora release 43 (Forty Three) |
| Kernel | 7.2.13-100.fc43.x86_64 |
| CPU | AMD Ryzen 7 6800HS with Radeon Graphics (16 threads) |
| RAM | 14.9 GiB |
| Docker | 29.8.2 |
| nginx:1.27 image | sha256:6784fb0834aa7dbbe12e3d7471e69c290df3e6ba810dc38b34ae33d3c1c05f7d |

## Runs

| system | run | rate | duration | url | host | switch | commit | command |
|---|---|---|---|---|---|---|---|---|
| mine | 1 | 500 | 60s | http://127.0.0.1:3000/ | app.localhost | +20.00s | 2c287f4 | bench/mine-run.sh 1 500 60s |
| nginx | 1 | 500 | 60s | http://127.0.0.1:8090/ | - | +20.00s | 2c287f4 | bench/nginx-run.sh 1 500 60s |

Raw vegeta results and per-run manifests: `raw.tar.gz` (not committed).
