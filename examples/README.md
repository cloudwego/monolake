# Getting Started

For detailed information on how to get started with the Monolake framework, please refer to the [Getting Started](https://www.cloudwego.io/docs/monolake/getting-started/) guide.

## HTTP Example

1. Run `gen_cert.sh` to generate needed certificates.
2. Start monolake with `cargo run -- --config examples/config.toml`.
3. `curl --resolve gateway.monoio.rs:8081:127.0.0.1 --cacert examples/certs/rootCA.crt -vvv https://gateway.monoio.rs:8081`

> Note: Except for the `--cacert path_to_ca`, you can also use `--insecure` to skip the certificate verification.

## Thrift Example

1. Start monolake with `cargo run -- --config examples/thrift.toml`.
2. Use your client request to `:8081`(will be forwarded to `127.0.0.1:9969`) or `/tmp/thrift_proxy_monolake.sock`(will be forwarded to `/tmp/thrift_server_monolake.sock`)
