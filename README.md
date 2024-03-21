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

## code coverage test

### install llvm-cov
cargo install cargo-llvm-cov

### It is already done: adding llvm-cov to Cargo.toml dependencies foelds:
[dependencies]
cargo-llvm-cov = "0.6.8"

### run code coverage test for all unit tests (in the code)
cargo llvm-cov

### run code coverage test for monolake functions
cargo llvm-cov --html run -- -c examples/config-2.toml

curl http://localhost:8402 # ip/port depends on config
curl -k https://localhost:6442 # ip/port depends on config
./wrk 'http://localhost:8402' -d 10s -c 10 -t 1 # ip/port depends on config
./wrk 'https://localhost:6442' -d 10s -c 10 -t 1  # ip/port depends on config

ps -A | grep monolake # find pid-of-monolake
kill -15 <pid-of-monolake> # send SIGTERM to quit monolake

open target/llvm-cov/html/index.html # show code coverage in browser page

cargo llvm-cov clean --workspace # clean the result for the next run

## Limitations

1. On Linux 5.6+, both uring and epoll are supported
2. On Linux 2.6+, only epoll is supported
3. On macOS, kqueue is used
4. Other platforms are currently not supported

## Call for help

Monoio is a subproject of [CloudWeGo](https://www.cloudwego.io).

Due to the limited resources, any help to make the monolake more mature, reporting issues or  requesting features are welcome. Refer the [Contributing](./CONTRIBUTING.md) documents for the guidelines.

## Dependencies

- [monoio](https://github.com/bytedance/monoio), Rust runtime
- [monoio-codec](https://github.com/monoio-rs/monoio-codec), framed reader or writer
- [monoio-tls](https://github.com/monoio-rs/monoio-tls), tls wrapper for monoio
- [monoio-http](https://github.com/monoio-rs/monoio-http), http protocols implementation base monoio

## License

Monoio is licensed under the MIT license or Apache license.