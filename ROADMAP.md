# Project Roadmap

This document outlines the development plan for Monolake. Our roadmap is divided into short-term and long-term goals, with specific milestones and features planned for each phase.

## Short-term Goals (Next 3-9 months)

### Version 1.1

- Config update notification support (replace existing polling approach)   
- Admin interface to allow users push configuration, observe internal state and control log level 
- Connection dispatcher: dispatching new connections to threads with better strategy
- Enhance observability features (logging, tracing, metrics)
- Load balancing with various algorithms
- TLS/SSL Intel QAT support

### Version 1.2

- IP Allow and Deny list
- Rate limiting
- Enhanced authentication/authorization
- Ingress Controller for Kubernetes
- TLS/SSL NIC/DPU acceleration

### Version 1.3

- Proxy protocol support
- WebSocket support
- HTTP/3 support
- Decompression and serialization DPU acceleration

## Long-term Goals (9-12 months)

### Version 2.0

- DPDK support

### Future Considerations

- Monolake-powered applications
- Community-driven feature requests (see Issues labeled 'C-feature-request')

## How to Contribute

We welcome contributions from the community! If you're interested in working on any of these items:

1. Check the issue tracker for related issues
2. Open a new issue to discuss your approach if none exists
3. Submit a pull request referencing the relevant issue

For more details, please see our CONTRIBUTING.md file.

## Disclaimer

This roadmap is subject to change based on community feedback, project priorities, and available resources. We'll update this document as plans evolve.

Last updated: [Nov.5, 2024]
