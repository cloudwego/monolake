---
title: "Config Reference"
linkTitle: "Config Reference"
weight: 4
date: 2023-07-3
description: "Config TOML file guide"

---

| Configuration Field | Field Type | Description |
|---------------------|-----------------|-------------|
| runtime.workers | Integer | Specifies the number of worker threads for the proxy. |
| runtime.entries | Integer | Specifies the number of entries for io-uring submission and completion queues |
| servers.serverX.name | String | The name of the server configuration. |
| servers.serverX.listener.type | "unix", "socket" | The type of listener for the server. |
| servers.serverX.listener.value | String | The value associated with the listener type (e.g., path to Unix domain socket or IP address and port for TCP socket). |
| servers.serverX.tls.chain | String (file path) | Path to the server certificate chain file for enabling TLS. |
| servers.serverX.tls.key | String (file path) | Path to the server private key file for enabling TLS. |
| servers.serverX.tls.stack | "rustls", "native_tls" | Specifies the TLS stack to use. |
| servers.serverX.routes.path | String | The URL path pattern to match for incoming requests. |
| servers.serverX.routes.upstreams.endpoint.type | "uri" | The type of endpoint for the upstream server. |
| servers.serverX.routes.upstreams.endpoint.value | String | The URI of the upstream server. |
