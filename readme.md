# Oxide-Shield

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.94-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/EugSh1/oxide-shield/actions/workflows/ci.yml/badge.svg)](https://github.com/EugSh1/oxide-shield/actions/workflows/ci.yml)
[![Build & Publish](https://github.com/EugSh1/oxide-shield/actions/workflows/build.yml/badge.svg)](https://github.com/EugSh1/oxide-shield/actions/workflows/build.yml)

A **fast, lightweight, and safe L7 (HTTP) Reverse Proxy with Rate Limiting**, built with Rust,
Tokio, and Hyper. Oxide-Shield is designed to protect downstream backend services from DDoS attacks
and abuse by implementing a highly efficient Token Bucket algorithm.

## Features

- **High-Performance L7 Proxy** ⚡️
    - Built on top of `hyper` for asynchronous, zero-copy HTTP proxying.
    - Uses `Either<Full<Bytes>, Incoming>` to efficiently route bodies without unnecessary
      allocations.
- **Token Bucket Rate Limiting** 🪣
    - Fair and precise rate-limiting using fractional token math (`f64`).
    - Configurable bucket capacity, refill rates, and intervals.
- **Highly Concurrent State** 🗺️
    - Uses `DashMap` for sharded, lock-free-like concurrent IP tracking. Thousands of requests can
      be validated simultaneously without mutex contention.
- **Automated Garbage Collection** 🧹
    - A background worker automatically sweeps and removes stale client buckets, preventing memory
      leaks.
- **Header Management & Injection** 🛡️
    - Automatically injects `X-Forwarded-For` for the upstream server.
    - Exposes `X-RateLimit-Limit` and `X-RateLimit-Remaining` headers to the client.
- **Trust-Aware (Reverse Proxy Support)** 🕵️
    - Supports custom client IP extraction (e.g., `X-Real-IP`, `CF-Connecting-IP`) for deployments
      where Oxide-Shield sits behind another proxy or CDN.

## Technologies Used

**Core & Networking:**

- **Rust (2024 Edition)**: Memory safety, fearless concurrency, and performance.
- **Tokio**: Industry-standard asynchronous runtime.
- **Hyper**: Fast and correct HTTP/1 implementation for both Server and Client roles.
- **DashMap**: Blazing fast concurrent hash map.

**Observability & Utilities:**

- **Tracing / Tracing-Subscriber**: For structured, JSON-based logging.
- **Anyhow**: For flexible error handling.

![Made with](https://go-skill-icons.vercel.app/api/icons?i=rust,tokiors,docker&theme=dark)

## Configuration

Oxide-Shield is entirely configured via environment variables.

| Environment Variable            | Status       | Description                                                                                      |
| :------------------------------ | :----------- | :----------------------------------------------------------------------------------------------- |
| `OXIDE_SHIELD_BIND_ADDR`        | _(Required)_ | The address and port the proxy will listen on (e.g., `0.0.0.0:3000`).                            |
| `OXIDE_SHIELD_SERVER_URI`       | _(Required)_ | The upstream backend server to proxy requests to (e.g., `example.com:80`).                       |
| `OXIDE_SHIELD_BUCKETS_CAPACITY` | _(Required)_ | Maximum amount of tokens a bucket can hold (Burst capacity).                                     |
| `OXIDE_SHIELD_REFILL_RATE`      | _(Required)_ | How many tokens are added per `REFILL_INTERVAL`.                                                 |
| `OXIDE_SHIELD_REFILL_INTERVAL`  | _(Required)_ | Time interval (in seconds) at which tokens are refilled.                                         |
| `OXIDE_SHIELD_IP_HEADER_NAME`   | _(Optional)_ | Header to extract the real client IP from (e.g., `X-Real-IP`). Defaults to TCP socket remote IP. |
| `RUST_LOG`                      | `info`       | Log level for the tracing subscriber (e.g., `debug`, `info`, `warn`).                            |

## Quick Start (Development)

The repository includes a `Makefile` that spins up the proxy locally, targeting
`jsonplaceholder.typicode.com:80` with a strict rate limit (10 max requests, 1 request refilled per
second).

1. **Start the proxy:**

    ```bash
    make dev
    ```

2. **Test the proxy:** Send an HTTP request. You will see the standard response from
   `jsonplaceholder.typicode.com:80`, plus the rate-limiting headers:

    ```bash
    curl -i http://localhost:3000/todos/1
    ```

    **Example Output:**

    ```text
    HTTP/1.1 200 OK
    x-ratelimit-remaining: 9
    x-ratelimit-limit: 10
    ...
    ```

3. **Test the Rate Limiter:** Send multiple requests quickly to exhaust the token bucket:

    ```bash
    for i in {1..12}; do curl -i http://localhost:3000/todos/1; done
    ```

    **Example Output (Blocked):**

    ```text
    HTTP/1.1 429 Too Many Requests
    content-type: text/plain
    x-ratelimit-remaining: 0
    x-ratelimit-limit: 10

    429 Too Many Requests
    ```

## Architecture Flow

1. **Accept Loop**: The main thread listens for incoming HTTP connections using Tokio and Hyper.
2. **IP Extraction**: Determines the real client IP (either directly from the TCP socket or via
   configured headers like `X-Real-IP`).
3. **Rate Limiting**: Checks the client's IP against the `DashMap` state. The Token Bucket algorithm
   deducts a token or instantly returns a `429 Too Many Requests` response if the bucket is empty.
4. **Upstream Proxy**: If allowed, the request is forwarded to the target backend using the Hyper
   client. `X-Forwarded-For` is automatically injected.
5. **Response Mutation**: Injects `X-RateLimit-*` headers into the upstream response before
   streaming it back to the client.
6. **Garbage Collection**: A spawned Tokio task periodically sweeps the `DashMap` to remove inactive
   IPs, freeing up memory.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
