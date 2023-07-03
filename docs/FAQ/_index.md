---
title: "FAQ"
linkTitle: "FAQ"
date: 2023-07-03
weight: 5
keywords: ["Monolake", "HTTP", "Proxy", "Q&A"]
description: "Monolake Frequently Asked Questions and Answers."
---

## Monolake 

**Q1: Can you run monolake on Mac OS？**
* Yes, monolake will default to epoll instead of io-uring on Mac OS.

**Q2: Does Monolake support HTTP2？**
* Yes, monolake suppports HTTP2 on the downstream(client to proxy) connection 
* Monolake defaults to HTTP1_1 on the upstream(proxy to server) connection with future support for HTTP2 planned   
