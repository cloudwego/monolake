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
| Name    | Required | Description           |
| ------- | -------- | --------------------- |
| runtime | true     | runtime configuration |
| servers | true     | server configuration  |


### Runtime
| Name         | Required | Default Value | Description           |
| ------------ | -------- | ------------- | --------------------- |
| workers      | true     | max cpu cores | num of worker threads |
| entries      | true     | 32768         | num of queue entries  |
| sqpoll_idle  | false    | None          | sqpoll idle entries   |
| runtime_type | true     | IoUring       | runtime type          |
| cpu_affinity | true     | true          | is CPU affinity       |

### Server
| Name             | Required | Default Value | Description      |
| ---------------- | -------- | ------------- | ---------------- |
| name             | true     |               | server name      |
| listener         | true     |               | listeners config |
| tls              | false    | None          | tls config       |
| routes           | true     |               | routes config    |
| keepalive_config | false    | None          | keepalive config |
### Listener
#### Socket Listener
| Name               | Required | Default Value | Description               |
| ------------------ | -------- | ------------- | ------------------------- |
| socket_addr        | true     |               | socket address            |
| transport_protocol | true     | Tcp           | transport protocol config |

#### UDS Listener
| Name               | Required | Default Value | Description               |
| ------------------ | -------- | ------------- | ------------------------- |
| uds_path           | true     |               | uds path                  |
| transport_protocol | true     | Tcp           | transport protocol config |


### Route
| Name      | Required | Description        |
| --------- | -------- | ------------------ |
| path      | true     | match request path |
| upstreams | true     | upstream endpoints |

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
