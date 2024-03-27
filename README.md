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

## manual code coverage test

When PR is merged into main branch, unit test code coverage will be automatically run. But to get more code coverage rate, we need to run code coverage test manually.

### install coverage tool
cargo install grcov # or "cargo install cargo-llvm-cov" for llvm-cov

### adding coverage tool to Cargo.toml dependencies foelds:
cargo add grcov --package monolake 
#### or for llvm-cov: add 'cargo-llvm-cov = "0.6.8"' to [dependencies] 
[dependencies]
cargo-llvm-cov = "0.6.8"

### grcov only: setup code coverage test for all unit tests 
export RUSTFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="<your_name>-%p-%m.profraw"
cargo build

### run code coverage test for all unit tests (in the code)
cargo test 
#### or for llvm-cov run "cargo llvm-cov"
cargo llvm-cov

### run code coverage test for integration test
RUST_LOG=info target/debug/code-coverage-monolake -c examples/config-2.toml & 
#### or for llvm-cov run "cargo llvm-cov --html run -- --bin code-coverage-monolake -c examples/config-2.toml"
cargo llvm-cov --html run -- --bin code-coverage-monolake -c examples/config-2.toml

### integration test
curl http://localhost:8402 # ip/port depends on config
####
curl -k https://localhost:6442 # ip/port depends on config
####
./wrk 'http://localhost:8402' -d 10s -c 10 -t 1 # ip/port depends on config
####
./wrk 'https://localhost:6442' -d 10s -c 10 -t 1  # ip/port depends on config

### manually kill the monolake process
kill -15 $(ps aux | grep 'code-coverage-monolake' | awk '{print $2}')

### grcov only: merge code coverage report
grcov . -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./target/debug/coverage/

### browse code coverage report
open target/debug/coverage/index.html
#### or for llvm-cov "open target/llvm-cov/index.html"
open target/llvm-cov/index.html

### if it is not the first run, use target/debug/coverage/html/index.html
open target/debug/coverage/html/index.html
#### or for llvm-cov "open target/llvm-cov/html/index.html"
open target/llvm-cov/html/index.html

### clean coverage report result for the next run
rm *.profraw */*.profraw 
#### or for llvm-cov "cargo llvm-cov clean --workspace" 
cargo llvm-cov clean --workspace

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