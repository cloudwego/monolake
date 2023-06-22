use fnv::FnvHashMap;
pub use known_keys::{ALPN_PROTOCOL, PEER_ADDR, REMOTE_ADDR};

pub use self::value::ValueType;

mod known_keys;
mod value;

#[derive(Debug, Clone)]
pub struct Environments {
    inner: FnvHashMap<String, ValueType>,
}

impl Environments {
    pub fn new(capacity: usize) -> Self {
        Environments {
            inner: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
        }
    }
    pub fn insert(&mut self, key: String, value: ValueType) {
        self.inner.insert(key, value);
    }

    pub fn get(&self, key: &String) -> Option<&ValueType> {
        self.inner.get(key)
    }
}

#[cfg(test)]
mod test {
    use super::Environments;
    use crate::environments::{ValueType, PEER_ADDR};

    #[test]
    pub fn test_add_entries_to_environment() {
        let mut environments = Environments::new(64);
        environments.insert("test".to_string(), ValueType::U32(8));
        environments.insert(
            PEER_ADDR.to_string(),
            ValueType::SocketAddr("127.0.0.1:8080".parse().unwrap()),
        );
        println!("{:?}", environments.get(&"abc".to_string()));
        println!("{:?}", environments.get(&"test".to_string()));
    }
}
