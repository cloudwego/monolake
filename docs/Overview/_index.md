---
title: "Overview"
linkTitle: "Overview"
weight: 1
keywords: ["Proxy", "Rust", "io-uring"]
description: "This doc covers architecture design, features and performance of Monolake."
---

## Monolake

Introducing **Monolake**: a **Rust** proxy built on the **io-uring** based runtime [Monoio](https://github.com/bytedance/monoio). Leveraging the efficiency and power of **IO-uring**, Monolake harnesses the full potential of modern asynchronous I/O operations for high-speed networking. Written in **Rust**, Monolake ensures memory safety, eliminating common memory-related bugs and providing a secure networking solution. Utilize Monolake to develop powerful proxies and load balancers for your high-throughput applications.

### Basic Features
| Feature                              | Status           |
|--------------------------------------|------------------|
| HTTP Proxy                            | Supported        |
| Routing based on Path                 | Supported        |
| TLS with Rustls and NativeTls         | Supported        |
| LKCF hardware openssl offload         | Supported        |
| Runtime configuration update          | Supported        |
| Frontend & Upstream conn keeplive/pool| Supported        |
| Openid integration                   | Supported        |
| TCP proxy                            | Supported        |
| HTTP2 downstream                      | Supported       |
| Header based routing                  | Planned          |
| Proxy protocol                       | Planned          |
| HTTP3 support                        | Planned          |
| GRPC over H2                          | Planned          |
| UDP proxy                             | Planned          |
| TLS/Quic          hardware offloading | Planned        |
| OAuth integration                     | Planned          |
| gzip (de)compression                  | Planned          |
| Client IP blacklist (Traffic Management) | Planned        |
| QAT hardware offloading               | Planned          |
| DPDK production ready                 | Planned          |

## Architecture 

## Performance

### Test environment

### Throughput performance

## Related Projects

- [monoio](https://github.com/bytedance/monoio): IO-uring based rust async runtime 
- [monoio-rs](https://github.com/monoio-rs): HTTP implementation

## Blogs