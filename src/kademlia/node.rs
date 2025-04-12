use crate::kademlia::constants::{ID_LENGTH, CRYPTO_KEY_LENGTH, KEY_LENGTH};
use crate::kademlia::kademlia_proto::Node as ProtoNode;
use crate::kademlia::routing_table::RoutingTable;
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct Node {
    public_key: [u8; CRYPTO_KEY_LENGTH],
    private_key: [u8; CRYPTO_KEY_LENGTH],
    id: [u8; ID_LENGTH],
    address: SocketAddr,
    routing_table: Arc<RwLock<RoutingTable>>,
    storage: Arc<RwLock<HashMap<[u8; KEY_LENGTH], Vec<u8>>>>,
}

impl Node {
    pub fn new(address: SocketAddr) -> Self {
        let keypair = Keypair::generate(&mut OsRng);
        let hash = Sha256::digest(keypair.public.to_bytes());
        let id = hash[..ID_LENGTH].try_into().expect("SHA-256 hash length must be 160 bits (20 bytes)");

        Self {
            public_key: keypair.public.to_bytes(),
            private_key: keypair.secret.to_bytes(),
            id,
            address,
            routing_table: Arc::new(RwLock::new(RoutingTable::new(id))),
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn from_sender(sender: &ProtoNode) -> Option<Self> {
        let id: [u8; ID_LENGTH] = sender.id.as_slice().try_into().ok()?;

        Some(Self {
            public_key: sender.public_key.as_slice().try_into().ok()?,
            private_key: [0; CRYPTO_KEY_LENGTH],
            id,
            address: SocketAddr::new(sender.ip.parse().ok()?, sender.port as u16),
            routing_table: Arc::new(RwLock::new(RoutingTable::new(id))),
            storage: Arc::new(Default::default()),
        })
    }

    pub fn get_public_key(&self) -> &[u8; CRYPTO_KEY_LENGTH] {
        &self.public_key
    }

    pub fn get_id(&self) -> &[u8; ID_LENGTH] {
        &self.id
    }

    pub fn get_address(&self) -> SocketAddr {
        self.address
    }

    pub fn get_routing_table(&self) -> Arc<RwLock<RoutingTable>> {
        self.routing_table.clone()
    }

    pub fn get_storage(&self) -> Arc<RwLock<HashMap<[u8; KEY_LENGTH], Vec<u8>>>> {
        self.storage.clone()
    }

    pub fn to_send(&self) -> ProtoNode {
        ProtoNode {
            id: self.id.to_vec(),
            ip: self.address.ip().to_string(),
            port: self.address.port() as u32,
            public_key: self.public_key.to_vec(),
        }
    }
}
