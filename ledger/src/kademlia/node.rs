use crate::routing::RoutingTable;
use crate::storage::DHT;
use std::net::SocketAddr;

pub struct Node {
    id: u64,
    address: SocketAddr,
    routing_table: RoutingTable,
    dht: DHT,
}

impl Node {
    pub fn new(id: u64, address: SocketAddr) -> Self {
        Self {
            id,
            address,
            routing_table: RoutingTable::new(),
            dht: DHT::new(),
        }
    }

    pub fn get_id(&self) -> u64 {
        self.id
    }

    pub fn get_address(&self) -> SocketAddr {
        self.address
    }

    pub fn get_routing_table(&self) -> RoutingTable {
        &self.routing_table
    }

    pub fn get_dht(&self) -> DHT {
        &self.dht;
    }

    pub fn store(&mut self, key: u64, value: String) {
        self.dht.insert(key, value);
    }
}
