# Example
1. Run `gen_cert.sh` to generate needed certificates.
2. Start monolake with `cargo run -- --config examples/config.toml`.
3. `curl --resolve gateway.monoio.rs:8082:127.0.0.1 --cacert examples/certs/rootCA.crt -vvv https://gateway.monoio.rs:8082`