# Monolake

A high performance reverse proxy based on [Monoio](http://github.com/bytedance/monoio).

## Basic Usage

```shell
# debug
cargo build
RUST_LOG=debug target/debug/monolake --config examples/config.toml

cargo build release
RUST_LOG=warn target/release/monolake --config examples/config.toml
```

## Configuration

### Example

``` toml
[runtime]
workers = 1
entries = 1024

# example server
[servers.example]
name = "gateway.example.com"
listener = { socket_addr = "0.0.0.0:8080" }

[[servers.example.routes]]
upstreams = [
    { endpoint = { uri = "https://www.wikipedia.org" } },
]
path = '/'
```
