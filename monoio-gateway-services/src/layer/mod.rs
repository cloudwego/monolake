pub mod accept;
pub mod auth;
pub mod delay;
pub mod detect;
pub mod listen;
pub mod router;
pub mod timeout;
pub mod tls;
pub mod transfer;
/// monoio service layer

pub trait NewService<I> {
    type Service;

    fn new_svc(&self, inner: I) -> Self::Service;
}
