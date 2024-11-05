---
title: "Configuration"
linkTitle: "Configuration"
weight: 2
keywords: ["Monolake", "Rust", "Proxy", "Configuration"]
description: "This page provides a brief guide for Monolake configuration"
---
## Configuration

This chapter provides a brief guide for Monolake configuration.

The configuration file is in .toml format. Basic configuration includes the following sections: runtime, servers, and routes for servers. We can use examples to explain each section of the configuration. There is also an example configuration file examples/config.toml in the code.

### Runtime

This section starts with [runtime]. It defines global run time configurations, including worker threads number and max connection entries. Optionally it may include a field "runtime_type" and it can be assigned to "legacy" (this is default for non-linux OS). If the host OS is linux, it can be also assigned to "iouring" (this is default for linux OS). For example:

```markup
[runtime]
runtime_type = "legacy"
worker_threads = 2
entries = 1024
```

### Server Configuragtion

All server configuration section starts with [servers.xxxx]. Multiple servers can be defined in a configuration file. Currently monolake supports HTTP/HTTPS server and UDS server. Server configuration defines the proxy configuration and rules. It includes the server name, listener and routes. The name is a string. The listener contains a type and a value. Normally, the type is source connection type of the proxy and the value is the source end point of the proxy. The route contains proxy rules. There are also other optional fields and we will discuss later.

#### Basic Server

It is for http server. It has a name and we can specify http version. The type of listener is "socket" and the value contains ip and port of the proxy source. The content_handler of http_opt_handlers indicates the content has a handler. For example:

```markup
[servers.server_basic]
name = "monolake.cloudwego.io"
upstream_http_version = "http11"
http_opt_handlers = { content_handler = true }
listener = { type = "socket", value = "0.0.0.0:8080" }
```

#### TLS Support

TLS server configuration contains additional "tls" section which has "chain" and "key" fields which are TLS cert and key file names. Another supported field "stack" specifies TLS implementation method and can be "rustls" or "native_tls". With TLS, the server can support HTTPS. For example:

```markup
[servers.server_tls]
tls = { chain = "examples/certs/server.crt", key = "examples/certs/server.key", stack = "rustls" }
name = "tls.monolake.cloudwego.io"
listener = { type = "socket", value = "0.0.0.0:8081" }
```

#### UDS (Unix Domain Socket) Server

UDS server configuration contains special listener with type "unix" and value containing the system file name of the proxy source. For example:

```markup
[servers.server_uds]
name = "uds.monolake.cloudwego.io"
listener = { type = "unix", value = "/tmp/monolake.sock" }
```

### Other Server Configuration

#### Thrift support

In server section, user can define proxy type to "thrift" to enable Apache thrift support. For example:

```markup
[servers.thrift_proxy]
name = "thrift_proxy"
proxy_type = "thrift"
listener = { type = "socket", value = "0.0.0.0:8081" }
```

#### Timeout Support

In server section, user can configure http timeout or thrift timeout. server_keepalive_timeout_sec is for server keepalive timeout. upstream_connect_timeout_sec is for connection timeout. upstream_read_timeout_sec and server_message_timeout_sec are for read/response timeout. server_read_header_timeout_sec is for http header read timeout. server_read_body_timeout_sec is for http body full read timeout. For example:

```markup
[servers.proxy]
name = "proxy"
http_timeout = { server_keepalive_timeout_sec = 60, upstream_connect_timeout_sec = 2, upstream_read_timeout_sec = 2 , server_read_header_timeout_sec = 2, server_read_body_timeout_sec = 5 }
thrift_timeout = { server_keepalive_timeout_sec = 60, server_message_timeout_sec = 2 }
listener = { type = "socket", value = "0.0.0.0:8081" }
```

### Routes Configuration

Router configuration is part of the server configuration. Thus the section starts with [[servers.xxx.routes]]. It defines route rules which proxies source paths to destination paths. The major fields are "path" and "upstreams". The field path defines the proxy source path. The field upstreams defines the proxy/mapped destination. Upstreams contains endpoint, which has a type and its value. Multiple routers can be defined for one server. Each different path can be mapped to an upstreams section.

For example:

```markup
[[servers.server_basic.routes]]
path = '/'
upstreams = [{ endpoint = { type = "uri", value = "http://127.0.0.1:9080" } }]
```

That proxies "http://127.0.0.1:8080/" to "http://127.0.0.1:9080/" if previous servers.server_basic was used.

```markup
[[servers.server_basic.routes]]
path = '/*p'
upstreams = [{ endpoint = { type = "uri", value = "http://127.0.0.1:9080" } }]
```

That proxies "http://127.0.0.1:8080/*p" to "http://127.0.0.1:9080/*p" if previous servers.server_basic was used.

#### Weight Configuration

Naturally proxy support load balacing. User can define multiple upstreams in upstreams section and use "weight" field to distribute load for each upstreams. Weight field is optional. A higher weight means the upstream is more likely to be chosen when distributing requests. If not specified, it defaults to 1. For example:

```markup
[[servers.server_uds.routes]]
upstreams = [
    { weight = 10, endpoint = { type = "uri", value = "http://127.0.0.1:9080" } },
    { weight = 20, endpoint = { type = "uri", value = "http://127.0.0.1:10080" } },
]
path = '/*p'
```

### Parameter Values

Please check [Config Reference](../Reference/_index.md) for more details.
