# Monolake

Monolake is a Rust-based high performance Layer 4/7 proxy framework which is built on the [Monoio](https://github.com/bytedance/monoio) runtime.

## Quick Start

The following guide is trying to use monolake with the basic proxy features.

### Preparation

```bash
# install rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# clone repo
git clone https://github.com/cloudwego/monolake.git
cd monolake

# generate certs
sh -c "cd examples && ./gen_cert.sh"
```

### Build

```bash
# build dev binary
cargo build

# build release binary
cargo build --release

# build lto release binary
cargo build --profile=release-lto
```

### Run examples

```bash
# run example with debug version
cargo run --package monolake -- -c examples/config.toml

# enable debug logging level
RUST_LOG=debug cargo run --package monolake -- -c examples/config.toml

# send https request
curl -vvv --cacert examples/certs/rootCA.crt --resolve "gateway.monolake.rs:8082:127.0.0.1"  https://gateway.monolake.rs:8082/
```

## Configurations

Please check [Configuration](docs/cloudwego/Getting%20Started/_config.md) for more details.

## Limitations

1. On Linux 5.6+, both uring and epoll are supported
2. On Linux 2.6+, only epoll is supported
3. On macOS, kqueue is used
4. Other platforms are currently not supported

## Call for help

Monolake is a subproject of [CloudWeGo](https://www.cloudwego.io).

Due to the limited resources, any help to make the monolake more mature, reporting issues or  requesting features are welcome. Refer the [Contributing](./CONTRIBUTING.md) documents for the guidelines.

## Dependencies

- [monoio](https://github.com/bytedance/monoio), Rust runtime
- [monoio-codec](https://github.com/monoio-rs/monoio-codec), framed reader or writer
- [monoio-tls](https://github.com/monoio-rs/monoio-tls), tls wrapper for monoio
- [monoio-http](https://github.com/monoio-rs/monoio-http), http protocols implementation base monoio

## License

Monolake is licensed under the MIT license or Apache license.
