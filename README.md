# Project Archival Notice
This project is no longer recommended for use and has been officially archived. The decision to archive is driven by the following key factors:
- Insufficient engineering resource investment: We currently lack the dedicated engineering manpower required to support ongoing development, bug fixes, and feature enhancements.
- Challenges in sustaining open-source maintenance and growth: Due to the above constraints, it has become impractical to continue maintaining the project at a standard that meets open-source community expectations, nor to drive its long-term development.

We sincerely appreciate the interest and any contributions this project has received from the community. For users seeking similar functionality, we recommend exploring alternative open-source solutions that are actively maintained and aligned with current technical standards.

## CloudWeGo-Monolake

English | [简体中文](README_zh.md)

[![License](https://img.shields.io/github/license/cloudwego/monolake)](https://github.com/cloudwego/monolake/blob/main/LICENSE)
[![OpenIssue](https://img.shields.io/github/issues/cloudwego/monolake)](https://github.com/cloudwego/monolake/issues)
[![ClosedIssue](https://img.shields.io/github/issues-closed/cloudwego/monolake)](https://github.com/cloudwego/monolake/issues?q=is%3Aissue+is%3Aclosed)
![Stars](https://img.shields.io/github/stars/cloudwego/monolake)
![Forks](https://img.shields.io/github/forks/cloudwego/monolake)

## Monolake Framework

Monolake is a framework for developing high-performance network services like proxies and gateways in **Rust**. It is built from the ground up as a blank slate design, starting with a async runtime called [Monoio](https://docs.rs/crate/monoio/latest) that has first-class support for **io_uring** .

While the most widely used Rust async runtime is [Tokio](https://docs.rs/tokio/latest/tokio/), which is an excellent and high-performance epoll/kqueue-based runtime, Monolake takes a different approach. The monoio runtime developed by Bytedance is designed with a thread-per-core model in mind, allowing Monolake to extract maximum performance from io_uring's highly efficient asynchronous I/O operations.

By building Monolake on this novel runtime foundation, the team was able to incorporate new first-class support for io_uring throughout the ecosystem. This includes io_uring specific IO traits and a unique service architecture that differs from the popular Tower implementation. Monolake also includes io_uring optimized implementations for Thrift and HTTP.

The Monolake framework has been used to build various high-performance proxies and gateways, and it is **actively deployed in production at [ByteDance](https://www.bytedance.com/)**. Its use cases are wide-ranging and include:

- Application Gateways: For protocol conversion, such as HTTP to Thrift
- Security Gateways: Providing pseudonymization for gRPC and Thrift RPCs

## Monolake Proxy

[Monolake Proxy](https://github.com/cloudwego/monolake/tree/main/monolake) is a reference implementation that leverages the various components within the Monolake framework to build a high-performance HTTP and Thrift proxy. This project serves as a showcase for the unique features and capabilities of the Monolake ecosystem. By utilizing the efficient networking capabilities of the [monoio-transports](https://docs.rs/monoio-transports/latest/monoio_transports/) crate, the modular service composition of [service-async](https://docs.rs/service-async/0.2.4/service_async/index.html), and the type-safe context management provided by [certain-map](https://docs.rs/certain-map/latest/certain_map/), Monolake Proxy demonstrates the practical application of the Monolake framework. Additionally, this reference implementation allows for the collection of benchmarks, enabling comparisons against other popular proxy solutions like Nginx and Envoy.

## Basic Features

- **io_uring-based Async Runtime (Monoio)**: Monolake is built on top of the Monoio runtime, which leverages the advanced capabilities of the io_uring Linux kernel feature to provide a highly efficient and performant asynchronous I/O foundation.

- **Thread-per-Core Model**: Monoio, the async runtime used by Monolake, follows a thread-per-core architecture, which simplifies concurrent programming and avoids the complexities associated with shared data across multiple threads.

- **Improved Service Trait and Lifecycle Management**: Monolake introduces an enhanced `Service` trait with improved borrowing semantics and a sophisticated service lifecycle management system, enabling seamless service versioning, rolling updates, and state preservation.

- **Modular and Composable Connector Architecture**: The `monoio-transports` crate provides a flexible and composable connector system, allowing developers to easily build complex network communication solutions by stacking various connectors (e.g., TCP, TLS, HTTP) on top of each other.

- **Context Management with `certain_map`**: Monolake utilizes the `certain_map` crate to provide a typed and compile-time guaranteed context management system, simplifying the handling of indirect data dependencies between services.

- **Optimized Protocol Implementations**: The Monolake framework includes io_uring-optimized implementations for protocols like HTTP and Thrift, taking full advantage of the underlying runtime's capabilities.

- **Modular and Extensible Design**: The Monolake framework is designed to be modular and extensible, allowing developers to easily integrate custom components or adapt existing ones to their specific needs.

## Performance

### Test environment

- AWS instance: c6a.8xlarge
- CPU: AMD EPYC 7R13 Processo, 16 cores, 32 threads
- Memory: 64GB
- OS: 6.1.94-99.176.amzn2023.x86_64, Amazon Linux 2023.5.20240805
- Nginx: 1.24.0

<p align="center">
  <img src=".github/images/https_req_per_sec_vs_body_size.png" alt="Requests per Second vs Body Size (HTTPS)" width="45%" style="margin-right: 10px;">
  <img src=".github/images/http_req_per_sec_vs_body_size.png" alt="HTTP Requests per Second vs Body Size (HTTP)" width="45%">
</p>

<p align="center">
  <img src=".github/images/https_req_per_sec_vs_worker_threads.png" alt="HTTPS Requests per Second vs Worker Threads (HTTPS)" width="45%" style="margin-right: 10px;">
  <img src=".github/images/http_req_per_sec_vs_worker_threads.png" alt="HTTP Requests per Second vs Worker Threads (HTTP)" width="45%">
</p>

## Documentation

- [**Getting Started**](./docs/getting-started/)

- [**Architecture**](./docs/architecture/)

- [**Developer guide**](./docs/tutorial/)

- [**Config guide**](./docs/config-guide/)

## Related Crates

| Crate | Description |
|-------|-------------|
| [monoio-transports](https://crates.io/crates/monoio-transports) | A foundational crate that provides high-performance, modular networking capabilities, including connectors and utilities for efficient network communications |
| [service-async](https://crates.io/crates/service-async) | A foundational crate that introduces a refined Service trait with efficient borrowing and zero-cost abstractions, as well as utilities for service composition and state management |
| [certain-map](https://crates.io/crates/certain-map) | A foundational crate that provides a typed map data structure, ensuring the existence of specific items at compile-time, useful for managing data dependencies between services |
| [monoio-thrift](https://crates.io/crates/monoio-thrift) | Monoio native, io_uring compatible thrift implementation |
| [monoio-http](https://crates.io/crates/monoio-http) | Monoio native, io_uring compatible HTTP/1.1 and HTTP/2 implementation |
| [monoio-nativetls](https://crates.io/crates/monoio-native-tls) | The native-tls implementation compatible with monoio |
| [monoio-rustls](https://crates.io/crates/monoio-rustls) | The rustls implementation compatible with monoio |

## Contributing

Contributor guide: [Contributing](https://github.com/cloudwego/monolake/blob/main/CONTRIBUTING.md).

## License

Monolake is licensed under the MIT license or Apache license.

## Community
- Email: [conduct@cloudwego.io](conduct@cloudwego.io)
- How to become a member: [COMMUNITY MEMBERSHIP](https://github.com/cloudwego/community/blob/main/COMMUNITY_MEMBERSHIP.md)
- Issues: [Issues](https://github.com/cloudwego/monolake/issues)
- Discord: Join community with [Discord Channel](https://discord.gg/NuYubnJ8fV). 

## Landscapes

<p align="center">
<img src="https://landscape.cncf.io/images/cncf-landscape-horizontal-color.svg" width="150"/>&nbsp;&nbsp;<img src="https://www.cncf.io/wp-content/uploads/2023/04/cncf-main-site-logo.svg" width="200"/>
<br/><br/>
CloudWeGo enriches the <a href="https://landscape.cncf.io/">CNCF CLOUD NATIVE Landscape</a>.
</p>
