use super::*;
use std::net::{IpAddr, SocketAddr};
use std::collections::{VecDeque, HashMap, HashSet};
use std::time::{Duration, Instant};
use ed25519_dalek::{Keypair, PublicKey, SecretKey, SecretKeyBytes};
use rand::rngs::OsRng;
use sha2::{Sha256, Digest};
use rand::{rngs::OsRng, RngCore};
use serde::{Serialize, Deserialize};

//TODO: derive distances
//TODO: create method to insert value at certain kbucket
//TODO: implement ping, store, find_node, find_value and join


const K_SIZE: usize = 20;
const NODE_TIMEOUT: Duration = Duration::from_secs(3600);
const BUCKET_COUNT: usize = 128;

#[derive(Serialize, Deserialize, Clone)]
struct Node{
    pub id: u128,
    pub address: IpAddr,
    pub port: u16,
    pub routing_table: RoutingTable,
    pub public_key: Vec<u8>,
    private_key: Vec<u8>,
    //pub storage:
}

impl Node{
    pub fn new(address: IpAddr,port: u16)-> Self{

        let (pub_key, priv_key) = Self::generate_keys();
        let id = Self::generate_id_from_public_key(pub_key);
        let routing_table = RoutingTable::new(id, address, port, pub_key);

        Node{
            id: id,
            address: address,
            port: port,
            routing_table: routing_table,
            public_key: pub_key,
            private_key: priv_key,
        }
    }

    pub fn generate_keys() -> (Vec<u8>,Vec<u8>){
        let mut csprng = OsRng;
        let keypair: Keypair = Keypair::generate(&mut csprng);
        let public_key = keypair.public.to_bytes().to_vec();
        let private_key = keypair.secret.to_bytes().to_vec();

        (public_key,private_key)
    }

    pub fn generate_id_from_public_key(public_key: Vec<u8>) -> u128 {
        let mut hasher = Sha256::new();
        hasher.update(public_key);
        let result = hasher.finalize();
        
        let mut id_bytes = [0u8; 16];
        id_bytes.copy_from_slice(&result[0..16]);
        u128::from_be_bytes(id_bytes)
    }
}


#[derive(Clone)]
struct RoutingTable{
    pub node_info: (u128, IpAddr, u16, [u8; 32]),
    pub kbuckets: Vec<Bucket>,
}

impl RoutingTable{
    pub fn new(id: u128, address: IpAddr, port: u16, pub_key:[u8; 32]){
        let node_info = (id,address,port,pub_key);
        let kbuckets = vec![Bucket::new(); BUCKET_COUNT];

        RoutingTable{
            node_info,
            kbuckets,
        }
    }
}


#[derive(Clone)]
struct Bucket{
    pub node_inst: Vec<(u64, IpAddr, u16, [u8; 32])>,
}

impl Bucket {
    pub fn new() -> Self {
        let node_inst = vec![(0, IpAddr::V4("0.0.0.0".parse().unwrap()), 0, [0u8; 32]); 20];
        Bucket {
            node_inst,
        }
    }

    pub fn insert(&mut self, id: u64, address: IpAddr, port: u16, public_key: [u8; 32]) {
        // Check if we already have the node or if we need to add it
        if self.node_inst.len() < 20 {
            self.node_inst.push((id, address, port, public_key));
        } else {
            // If the bucket is full, you could either remove the oldest or 
            // implement some replacement strategy. Here we'll just remove the first item.
            self.node_inst.remove(0);
            self.node_inst.push((id, address, port, public_key));
        }
    }
}



