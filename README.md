# CloudWeGo-Monolake

[![WebSite](https://img.shields.io/website?up_message=cloudwego&url=https%3A%2F%2Fwww.cloudwego.io%2F)](https://www.cloudwego.io/)
[![License](https://img.shields.io/github/license/cloudwego/monolake)](https://github.com/cloudwego/monolake/blob/main/LICENSE)
[![OpenIssue](https://img.shields.io/github/issues/cloudwego/kitex)](https://github.com/cloudwego/monolake/issues)
[![ClosedIssue](https://img.shields.io/github/issues-closed/cloudwego/monolake)](https://github.com/cloudwego/monolake/issues?q=is%3Aissue+is%3Aclosed)
![Stars](https://img.shields.io/github/stars/cloudwego/monolake)
![Forks](https://img.shields.io/github/forks/cloudwego/monolake)

## Monolake Framework

Monolake is an open-source framework for developing high-performance network services like proxies and gateways. It is built from the ground up as a blank slate design, starting with a custom async runtime called [Monoio](https://docs.rs/crate/monoio/latest) that has first-class support for the io_uring Linux kernel feature.

While the most widely used Rust async runtime is [Tokio](https://docs.rs/tokio/latest/tokio/), which is an excellent and high-performance epoll/kqueue-based runtime, Monolake takes a different approach. The monoio runtime developed by Bytedance is designed with a thread-per-core model in mind, allowing Monolake to extract maximum performance from io_uring's highly efficient asynchronous I/O operations.

By building Monolake on this novel runtime foundation, the team was able to incorporate new first-class support for io_uring throughout the ecosystem. This includes io_uring-specific IO traits and a unique service architecture that differs from the popular Tower implementation. Monolake also includes io_uring-optimized implementations for protocols like Thrift and HTTP.

The Monolake team has used this framework to build a variety of high-performance network components, including:
- HTTP and Thrift proxies
- Application gateways (HTTP-to-Thrift)
- gRPC proxies

By focusing on cutting-edge Rust and io_uring, Monolake aims to provide developers with a powerful toolkit for building 

## Monolake Proxy

[Monolake Proxy](https://github.com/cloudwego/monolake/tree/main/monolake) is a reference implementation that leverages the various components within the Monolake framework to build a high-performance HTTP and Thrift proxy. This project serves as a showcase for the unique features and capabilities of the Monolake ecosystem. By utilizing the efficient networking capabilities of the monoio-transports crate, the modular service composition of service-async, and the type-safe context management provided by certain-map, Monolake Proxy demonstrates the practical application of the Monolake framework. Additionally, this reference implementation allows for the collection of benchmarks, enabling comparisons against other popular proxy solutions like Nginx and Envoy.

### Basic Features

- **io_uring-based Async Runtime (Monoio)**: Monolake is built on top of the Monoio runtime, which leverages the advanced capabilities of the io_uring Linux kernel feature to provide a highly efficient and performant asynchronous I/O foundation.

- **Thread-per-Core Model**: Monoio, the async runtime used by Monolake, follows a thread-per-core architecture, which simplifies concurrent programming and avoids the complexities associated with shared data across multiple threads.

- **Improved Service Trait and Lifecycle Management**: Monolake introduces an enhanced `Service` trait with improved borrowing semantics and a sophisticated service lifecycle management system, enabling seamless service versioning, rolling updates, and state preservation.

- **Modular and Composable Connector Architecture**: The `monoio-transports` crate provides a flexible and composable connector system, allowing developers to easily build complex network communication solutions by stacking various connectors (e.g., TCP, TLS, HTTP) on top of each other.

- **Context Management with `certain_map`**: Monolake utilizes the `certain_map` crate to provide a typed and compile-time guaranteed context management system, simplifying the handling of indirect data dependencies between services.

- **Optimized Protocol Implementations**: The Monolake framework includes io_uring-optimized implementations for protocols like HTTP and Thrift, taking full advantage of the underlying runtime's capabilities.

- **Modular and Extensible Design**: The Monolake framework is designed to be modular and extensible, allowing developers to easily integrate custom components or adapt existing ones to their specific needs.

## Documentation

- [**Getting Started**](https://www.cloudwego.io/docs/monolake/getting-started/)

- [**Architecture**](https://www.cloudwego.io/docs/monolake/architecture/)

- [**Developer guide**](https://www.cloudwego.io/docs/monolake/tutorial/)

- [**Config guide**](https://www.cloudwego.io/docs/monolake/config-guid/)

## Performance
TODO

## Related Projects

- [Monoio](https://github.com/bytedance/monoio): A high-performance thread-per-core io_uring based async runtime
- [monoio-transports](https://github.com/monoio-rs/monoio-transports)
- [service-async](https://github.com/ihciah/service-async)
- [certain-map](https://github.com/ihciah/certain-map)
- [monoio-thrift](https://github.com/monoio-rs/monoio-thrift)
- [monoio-http](https://github.com/monoio-rs/monoio-http)
- [monoio-nativetls](https://github.com/monoio-rs/monoio-tls)

## Blogs
- [Monolake: How ByteDance Developed Its Own Rust Proxy to Save Hundreds of Thousands of CPU Cores](TODO)

## Contributing

Contributor guide: [Contributing](https://github.com/cloudwego/monolake/blob/develop/CONTRIBUTING.md).

## License

Monolake is licensed under the MIT license or Apache license.

## Community
- Email: [conduct@cloudwego.io](conduct@cloudwego.io)
- How to become a member: [COMMUNITY MEMBERSHIP](https://github.com/cloudwego/community/blob/main/COMMUNITY_MEMBERSHIP.md)
- Issues: [Issues](https://github.com/cloudwego/monoalke/issues)

TODO: Add more community information

## Landscapes

<p align="center">
<img src="https://landscape.cncf.io/images/cncf-landscape-horizontal-color.svg" width="150"/>&nbsp;&nbsp;<img src="https://www.cncf.io/wp-content/uploads/2023/04/cncf-main-site-logo.svg" width="200"/>
<br/><br/>
CloudWeGo enriches the <a href="https://landscape.cncf.io/">CNCF CLOUD NATIVE Landscape</a>.
</p>
