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

### make sure you have newer version of gcc/g++ and ld
gcc/g++ and ld must have newer version than 8.3/2.38. Otherwise build will report error of "undefined hidden symbol `__ehdr_start'". We tested with gcc/g++ 11.4.1 and ld 2.39-6. and these work. 

### make sure you have identity.pfx (used for test)
openssl pkcs12 -export -out examples/certs/identity.pfx -inkey examples/certs/key.pem -in examples/certs/cert.pem

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
RUST_LOG=debug target/debug/code-coverage-monolake -c examples/config.toml &
#### or for llvm-cov run "cargo llvm-cov --html run -- --bin code-coverage-monolake -c examples/config-2.toml"
cargo llvm-cov --html run -- --bin code-coverage-monolake -c examples/config.toml &

### integration test (you must setup the servers before running it)

#### 
curl -k https://localhost:8082; 
curl -k https://localhost:8083

### manually kill the monolake process
kill -15 $(ps aux | grep 'code-coverage-monolake' | awk '{print $2}')

#### more integration tests
RUST_LOG=debug target/debug/code-coverage-monolake -c examples/config-2.toml &

#### 
curl http://localhost:8402; 
curl http://localhost:8403; 
curl http://localhost:8405; 
curl -k https://localhost:6442; 
curl -k https://localhost:6445; 
curl -k -v https://localhost:8082; 
curl -k -v https://localhost:8083; 
curl -k -v http://localhost:8083; 
curl -k -v https://localhost:6442/server2; 
curl -k -v https://localhost:6442/server2/1

#### 
cd ../wrk; 
./wrk 'http://localhost:8402' -d 1m -c 10 -t 1 -R40000 --latency; 
./wrk 'http://localhost:8403' -d 15s -c 10 -t 1 -R40000 --latency; 
./wrk 'http://localhost:8404' -d 15s -c 10 -t 10 -R40000 --latency; 
./wrk 'http://localhost:8405' -d 35s -c 80 -t 5 -R40000 --latency; 
./wrk 'https://localhost:6442/' -d 15s -c 10 -t 1 -R40000 --latency; 
./wrk 'https://localhost:6443/' -d 15s -c 10 -t 1 -R40000 --latency; 
./wrk 'https://localhost:6444/' -d 25s -c 80 -t 10 -R40000 --latency; 
./wrk 'https://localhost:6445/' -d 15s -c 10 -t 1 -R40000 --latency; 

### manually kill the monolake process
kill -15 $(ps aux | grep 'code-coverage-monolake' | awk '{print $2}')

#### more integration tests
RUST_LOG=debug target/debug/code-coverage-monolake -c examples/config-3.toml &

### manually kill the monolake process
kill -15 $(ps aux | grep 'code-coverage-monolake' | awk '{print $2}')

#### more integration tests
RUST_LOG=debug target/debug/code-coverage-monolake -c examples/config-4.toml &

#### 
curl -v http://127.0.0.1:8080; 
curl -v http://127.0.0.1:8080/p; 
curl -v http://127.0.0.1:8080/p2; 
curl -k -v https://127.0.0.1:8081; 
curl -k -v https://127.0.0.1:8081/p; 
curl -k -v https://127.0.0.1:8081/p2; 
curl -X GET --unix-socket /tmp/monolake.sock http://localhost:10082/; 
curl -X GET --unix-socket /tmp/monolake.sock http://localhost:9080/; 
curl -X GET --unix-socket /tmp/monolake.sock http://localhost:9081/; 
curl -X GET --unix-socket /tmp/monolake.sock http://localhost:9080/p; 
curl -X GET --unix-socket /tmp/monolake.sock http://localhost:9080/p2; 
curl -X GET --unix-socket /tmp/monolake.sock http://localhost:9081/p; 

### manually kill the monolake process
kill -15 $(ps aux | grep 'code-coverage-monolake' | awk '{print $2}')

### thrift config
RUST_LOG=debug target/debug/code-coverage-monolake -c examples/thrift.toml &

### run thrift test
curl http://localhost:8081

### manually kill the monolake process
kill -15 $(ps aux | grep 'code-coverage-monolake' | awk '{print $2}')

### grcov only: merge code coverage report
grcov . -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing  --ignore '../*' --ignore "/*" --ignore "monolake/src/main.rs" -o ./target/debug/coverage/

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