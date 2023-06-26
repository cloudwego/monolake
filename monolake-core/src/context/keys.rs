use derive_more::{From, Into};

use crate::listener::AcceptedAddr;

#[derive(From, Into, Debug, Clone)]
pub struct PeerAddr(pub AcceptedAddr);

#[derive(From, Into, Debug, Clone)]
pub struct RemoteAddr(pub AcceptedAddr);
