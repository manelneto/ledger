use std::net::SocketAddr;

pub struct Node {
    id: [u8; 20],
    address: SocketAddr,
}

impl Node {
    pub fn new(id: [u8; 20], address: SocketAddr) -> Self {
        Self {
            id,
            address,
        }
    }

    pub fn get_id(&self) -> &[u8; 20] {
        &self.id
    }

    pub fn get_address(&self) -> SocketAddr {
        self.address
    }
}
