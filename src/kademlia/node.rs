use ed25519_dalek::Keypair;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

pub struct Node {
    public_key: [u8; 32],
    private_key: [u8; 32],
    id: [u8; 20],
    address: SocketAddr,
    storage: Arc<RwLock<HashMap<[u8; 20], Vec<u8>>>>,
}

impl Node {
    pub fn new(address: SocketAddr) -> Self {
        let keypair = Keypair::generate(&mut OsRng);

        let hash = Sha256::digest(keypair.public.to_bytes());

        Self {
            public_key: keypair.public.to_bytes(),
            private_key: keypair.secret.to_bytes(),
            id: hash[..20].try_into().expect("SHA-256 hash length must be 160 bits (20 bytes)"),
            address,
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_public_key(&self) -> &[u8; 32] {
        &self.public_key
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
