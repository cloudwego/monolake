# Monolake, a proxy framework base on Rust and Iouring

Earlier 2023, Cloudflare released a blog to introduce their Oxy, a Rust-based modern proxy framework. Volcano Engine, a public cloud from Bytedance Inc. has similar requirements, and we start Monolake, a layer 7 proxy framework base on Rust and Iouring.

# Architecture of Monolake

There are 3 major categories in monolake, the runtime and transport category, the tls category and the http category. Monolake currently supoprt Iouring and epoll runtime which are benefit from monoio(a thread-per-core rust runtime). The layer 4 proxy is implemented in monolake. The tls category currently support both rustls and native-tls, user can switch between these two solutions in case there is critical security defect in one of them. For the http category, monolake support http/1.1 and h2, we are currently working on the thrift protocol support, the grpc and h3 protocol support is planned. 

```
+-----------+     +-----------+  +-----------+     +-----------+ +------------+ +-----------+
|    HTTP   |     | HTTP/1.1  |  |    H2     |     |monoio-http| |monoio-codec| |  monolake |
+-----------+     +-----------+  +-----------+     +-----------+ +------------+ +-----------+

+-----------+     +-----------+  +-----------+     +-----------+
|    TLS    |     |  rustls   |  |native-tls |     |monoio-tls |
+-----------+     +-----------+  +-----------+     +-----------+

+-----------+     +-----------+  +-----------+     +-----------+ +-----------+
|Runtime/   |     |  Iouring  |  |   epoll   |     |  monoio   | | monolake  |
|Transport  |     |           |  |           |     |           | |           |
+-----------+     +-----------+  +-----------+     +-----------+ +-----------+
```

Besides multi-protocols and proxy features, monolake provides the ability to update the handler chains at runtime. Combine with the linux SO_REUSEPORT socket option, users can upgrade monolake binary at runtime. 

# Why Thread-per-Core
- tpc can reduce the tail latency
- no synchronization requirements between different threads for tpc
- cpu binding make sure each thread bind to a cpu core to reduce context switch
- compare to process model, tpc still be able to share state between different thread

# Runtime handler chain update


# Next steps
- H3/Quic support
- More protocols support
- Scaffolding to create a proxy

# Conclusion
 
