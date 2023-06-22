use std::{fmt::Debug, net::SocketAddr, path::PathBuf};

use bytes::Bytes;

#[derive(Debug, Clone)]
pub enum ValueType {
    Usize(usize),
    U32(u32),
    I32(i32),
    F32(f32),
    SocketAddr(SocketAddr),
    Path(PathBuf),
    String(String),
    Bytes(Bytes),
}
