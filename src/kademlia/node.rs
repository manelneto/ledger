use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

pub struct Node {
    id: [u8; 20],
    address: SocketAddr,
    storage: Arc<RwLock<HashMap<[u8; 20], Vec<u8>>>>,
}

impl Node {
    pub fn new(id: [u8; 20], address: SocketAddr) -> Self {
        Self {
            id,
            address,
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_id(&self) -> &[u8; 20] {
        &self.id
    }

    pub fn get_address(&self) -> SocketAddr {
        self.address
    }

    pub fn get_storage(&self) -> Arc<RwLock<HashMap<[u8; 20], Vec<u8>>>> {
        self.storage.clone()
    }
}
