#!/usr/bin/env bash
# Starts all example backends; Ctrl-C stops them all.
set -e
cd "$(dirname "$0")"

cargo build --example backend
bin=target/debug/examples/backend

trap 'kill 0' EXIT

"$bin" 8081 user-service-1 &
"$bin" 8083 user-service-2 &
"$bin" 8082 order-service-1 &
"$bin" 8084 order-service-2 &
"$bin" 8080 default-service &

wait
