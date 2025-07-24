use monolake_core::context::{PeerAddr, RemoteAddr};

// This struct should be a app-defined struct.
// Framework should not bind it.
certain_map::certain_map! {
    #[derive(Clone)]
    #[full(FullContext)]
    pub struct Context {
        // Set by ContextService
        peer_addr: PeerAddr,
        // Set by ProxyProtocolService
        remote_addr: Option<RemoteAddr>,
    }
}

#[cfg(test)]
mod test {
    use std::net::SocketAddr;

    use certain_map::ParamSet;
    use monolake_core::listener::AcceptedAddr;
    use service_async::ParamRef;

    use super::*;

    #[test]
    pub fn test_add_entries_to_context() {
        let mut ctx = Context::new();
        let handler = ctx.handler();
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let peer_addr = PeerAddr::from(AcceptedAddr::from(addr));
        let handler = handler.param_set(peer_addr);
        match ParamRef::<PeerAddr>::param_ref(&handler).0 {
            AcceptedAddr::Tcp(socket_addr) => assert_eq!(addr, socket_addr),
            _ => unreachable!(),
        }
    }
}
