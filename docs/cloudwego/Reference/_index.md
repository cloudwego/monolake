---
title: "Config Reference"
linkTitle: "Config Reference"
weight: 4
date: 2024-11-05
description: "Config TOML file guide"

---

| Configuration Field | Field Type | Description |
|---------------------|-----------------|-------------|
| runtime.entries | Integer | Specifies the number of entries for io-uring submission and completion queues (default 32768) |
| runtime.runtime_type | "io_uring", "legacy" | Specifies the runtime type for the proxy. "io_uring" only supported in linux, and is the default value for linux|
| runtime.workers | Integer | Specifies the number of worker threads (default 1) for the proxy. |
| servers.serverX.http_opt_handlers.content_handler | Boolean | Use the content handler of the server configuration. |
| servers.serverX.http_timeout.server_keepalive_timeout_sec | Integer | The server keepalive timeout of the server configuration. |
| servers.serverX.http_timeout.server_read_header_timeout_sec | Integer | The server read header timeout of the server configuration. |
| servers.serverX.http_timeout.server_read_body_timeout_sec | Integer | The server read full body timeout of the server configuration. |
| servers.serverX.http_timeout.upstream_connect_timeout_sec | Integer | The upstream connect timeout of the server configuration. |
| servers.serverX.http_timeout.upstream_read_timeout_sec | Integer | The upstream response timeout of the server configuration. |
| servers.serverX.listener.type | "unix", "socket" | The type of listener for the server. |
| servers.serverX.listener.value | String | The value associated with the listener type (e.g., path to Unix domain socket or IP address and port for TCP socket). |
| servers.serverX.name | String | The name of the server configuration. |
| servers.serverX.proxy_type | "http", "thrift" | The proxy type. |
| servers.serverX.routes.path | String | The URL path pattern to match for incoming requests. |
| servers.serverX.routes.upstreams.endpoint.type | "uri" | The type of endpoint for the upstream server. |
| servers.serverX.routes.upstreams.endpoint.value | String | The URI of the upstream server. |
| servers.serverX.thrift_timeout.server_keepalive_timeout_sec | Integer | The thrift server keepalive timeout of the server configuration. |
| servers.serverX.thrift_timeout.server_message_timeout_sec | Integer | The thrift server read message timeout of the server configuration. |
| servers.serverX.tls.chain | String (file path) | Path to the server certificate chain file for enabling TLS. |
| servers.serverX.tls.key | String (file path) | Path to the server private key file for enabling TLS. |
| servers.serverX.tls.stack | "rustls", "native_tls" | Specifies the TLS stack to use. |
| servers.serverX.upstream_http_version | "auto", "http11", "http2" | The upstream HTTP version of the server configuration. The default is "auto" (find the version by request and upstream response) |
