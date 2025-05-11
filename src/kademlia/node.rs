use crate::kademlia::constants::{ALPHA, CRYPTO_KEY_LENGTH, ID_LENGTH, KEY_LENGTH, K, TIMEOUT, TRIES};
use crate::kademlia::kademlia_proto::{FindNodeRequest, FindValueRequest, Node as ProtoNode, PingRequest, StoreRequest, JoinRequest};
use crate::kademlia::routing_table::RoutingTable;
use ed25519_dalek::Keypair;
use futures::stream::{FuturesUnordered, StreamExt};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::timeout;
use tonic::{Request, Status};
use tonic::transport::Server;
use crate::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use crate::kademlia::kademlia_proto::kademlia_server::KademliaServer;
use crate::kademlia::service::KademliaService;

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

    pub fn to_send(&self) -> ProtoNode {
        ProtoNode {
            id: self.id.to_vec(),
            ip: self.address.ip().to_string(),
            port: self.address.port() as u32,
            public_key: self.public_key.to_vec(),
        }
    }

    pub async fn bootstrap(&self, bootstrap_node: Node) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = KademliaClient::connect(format!("http://{}", bootstrap_node.get_address())).await?;

        let request = Request::new(FindNodeRequest {
            sender: Some(self.to_send()),
            id: self.id.to_vec(),
        });

        let response = client.find_node(request).await?.into_inner();

        let mut routing_table = self.routing_table.write().map_err(|_| {
            Status::internal("BOOTSTRAP: failed to acquire lock on routing table")
        })?;

        for proto in response.nodes {
            if let Some(node) = Node::from_sender(&proto) {
                routing_table.update(node);
            }
        }

        println!("BOOTSTRAP: OK");

        Ok(())
    }

    pub async fn ping(&self, target: &Node) -> Result<bool , Box<dyn std::error::Error>> {
        let mut client = KademliaClient::connect(format!("http://{}", target.get_address())).await?;

        for _ in 0..TRIES {
            let request = Request::new(PingRequest {
                sender: Some(self.to_send()),
            });

            let result = timeout(Duration::from_millis(TIMEOUT), client.ping(request)).await;

            if let Ok(Ok(response)) = result {
                return Ok(response.into_inner().alive);
            }
        }

        Ok(false)
    }

    pub async fn store(&self, key: [u8; KEY_LENGTH], value: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        {
            let storage_lock = self.get_storage();
            let mut storage = storage_lock.write().unwrap();
            storage.insert(key, value.clone());
        }

        let closest_nodes = self.iterative_find_node(key).await;

        for node in closest_nodes {
            let _ = self.store_at(&node, key, value.clone()).await;
        }

        Ok(())
    }

    pub async fn store_at(&self, target: &Node, key: [u8; KEY_LENGTH], value: Vec<u8>) -> Result<bool, Box<dyn std::error::Error>> {
        let mut client = KademliaClient::connect(format!("http://{}", target.get_address())).await?;

        let request = Request::new(StoreRequest {
            sender: Some(self.to_send()),
            key: key.to_vec(),
            value,
        });

        let response = client.store(request).await?.into_inner();

        Ok(response.success)
    }


    pub async fn find_node(&self, target: Node, id: [u8; ID_LENGTH]) -> Result<Vec<Node>, Box<dyn std::error::Error>> {
        let mut client = KademliaClient::connect(format!("http://{}", target.get_address())).await?;

        let request = Request::new(FindNodeRequest {
            sender: Some(self.to_send()),
            id: id.to_vec(),
        });

        let response = client.find_node(request).await?.into_inner();

        let nodes = response.nodes
            .into_iter()
            .filter_map(|proto| Node::from_sender(&proto))
            .collect();

        Ok(nodes)
    }

    pub async fn find_value(&self, target: Node, key: [u8; KEY_LENGTH]) -> Result<(Option<Vec<u8>>, Vec<Node>), Box<dyn std::error::Error>> {
        let mut client = KademliaClient::connect(format!("http://{}", target.get_address())).await?;

        let request = Request::new(FindValueRequest {
            sender: Some(self.to_send()),
            key: key.to_vec(),
        });

        let response = client.find_value(request).await?.into_inner();

        let value = response.value;
        let nodes = response.nodes
            .into_iter()
            .filter_map(|proto| Node::from_sender(&proto))
            .collect();

        Ok((value, nodes))
    }

    pub async fn iterative_find_node(&self, target: [u8; ID_LENGTH]) -> Vec<Node> {
        let mut closest: Vec<Node> = {
            let routing_table_lock = self.get_routing_table();
            let routing_table = routing_table_lock.read().expect("ITERATIVE_FIND_NODE: failed to read routing table");
            routing_table.find_closest_nodes(&target, K)
        };

        let mut queried = HashSet::new();
        let mut candidates: VecDeque<Node> = VecDeque::from(closest.clone());

        while !candidates.is_empty() {
            let mut parallel_requests = FuturesUnordered::new();

            for _ in 0..ALPHA {
                if let Some(node) = candidates.pop_front() {
                    if queried.insert(node.get_id().to_vec()) {
                        parallel_requests.push(async move {
                            timeout(Duration::from_millis(TIMEOUT), self.find_node(node, target)).await
                        });
                    }
                }
            }

            while let Some(Ok(Ok(nodes))) = parallel_requests.next().await {
                for node in nodes {
                    let it = node.get_id().to_vec();
                    if !queried.contains(&it) && !candidates.iter().any(|n| n.get_id() == node.get_id()) {
                        candidates.push_back(node.clone());
                    }
                    closest.push(node);
                }
            }

            closest.sort_by_key(|n| RoutingTable::xor_distance(n.get_id(), &target));
            closest.dedup_by_key(|n| n.get_id().to_vec());
            closest.truncate(K);
        }

        closest
    }

    pub async fn iterative_find_value(&self, key: [u8; KEY_LENGTH]) -> Option<Vec<u8>> {
        let mut closest: Vec<Node> = {
            let routing_table_lock = self.get_routing_table();
            let routing_table = routing_table_lock.read().expect("ITERATIVE_FIND_VALUE: failed to read routing table");
            routing_table.find_closest_nodes(&key, K)
        };

        let mut queried = HashSet::new();
        let mut candidates: VecDeque<Node> = VecDeque::from(closest.clone());

        while !candidates.is_empty() {
            let mut parallel_requests = FuturesUnordered::new();

            for _ in 0..ALPHA {
                if let Some(node) = candidates.pop_front() {
                    if queried.insert(node.get_id().to_vec()) {
                        parallel_requests.push(async move {
                            timeout(Duration::from_millis(TIMEOUT), self.find_value(node, key)).await
                        });
                    }
                }
            }

            while let Some(Ok(Ok((value_opt, nodes)))) = parallel_requests.next().await {
                if let Some(value) = value_opt {
                    closest.push(self.clone());
                    closest.sort_by_key(|n| RoutingTable::xor_distance(n.get_id(), &key));
                    closest.dedup_by_key(|n| n.get_id().to_vec());

                    let top: Vec<_> = closest.into_iter().take(K).collect();
                    if top.iter().any(|n| n.get_id() == self.get_id()) {
                        if let Ok(mut storage) = self.get_storage().write() {
                            storage.insert(key, value.clone());
                        }
                    }

                    return Some(value);
                }

                for node in nodes {
                    let id = node.get_id().to_vec();
                    if !queried.contains(&id) && !candidates.iter().any(|n| n.get_id() == node.get_id()) {
                        candidates.push_back(node.clone());
                    }
                    closest.push(node);
                }

                closest.sort_by_key(|n| RoutingTable::xor_distance(n.get_id(), &key));
                closest.dedup_by_key(|n| n.get_id().to_vec());
                closest.truncate(K);
            }
        }

        None
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        Server::builder()
            .add_service(KademliaServer::new(KademliaService::new(self.clone())))
            .serve(self.address)
            .await?;

        println!("NODE START: {}", self.address);

        Ok(())
    }

    pub fn new_with_id(address: SocketAddr, id: [u8; ID_LENGTH]) -> Self {
        let keypair = Keypair::generate(&mut OsRng);

        Self {
            public_key: keypair.public.to_bytes(),
            private_key: keypair.secret.to_bytes(),
            id,
            address,
            routing_table: Arc::new(RwLock::new(RoutingTable::new(id))),
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn join_with_pow(&self, bootstrap_node: Node, difficulty: usize) -> Result<(), Box<dyn std::error::Error>> {

        if !self.ping(&bootstrap_node).await? {
            return Err("Could not ping bootstrap node".into());
        }
    
        let (nonce, pow_hash) = self.generate_pow(difficulty).await;
    
        let mut client = KademliaClient::connect(format!("http://{}", bootstrap_node.get_address())).await?;
        let response = client.join(Request::new(JoinRequest {
            sender: Some(self.to_send()),
            nonce: nonce.to_vec(),
            pow_hash: pow_hash.to_vec(),
        })).await?.into_inner();
    
        if !response.accepted {
            return Err("Join request rejected by bootstrap node".into());
        }
    
        let mut routing_table = self.routing_table.write().map_err(|_| {
            Status::internal("JOIN: failed to acquire lock on routing table")
        })?;
    
        let ping_futures = response.closest_nodes
            .into_iter()
            .filter_map(|proto| {
                let node = Node::from_sender(&proto)?;
                // Skip self and invalid nodes
                (node.get_id() != self.get_id()).then(|| {
                    routing_table.update(node.clone());
                    node
                })
            })
            .map(|node| async move {
                match tokio::time::timeout(
                    Duration::from_secs(5),
                    self.ping(&node)
                ).await {
                    Ok(Ok(true)) => println!(" Successfully pinged {}", node.get_address()),
                    Ok(Ok(false)) => println!(" Ping failed to {}", node.get_address()),
                    Ok(Err(e)) => println!(" Ping error to {}: {}", node.get_address(), e),
                    Err(_) => println!(" Ping timeout to {}", node.get_address()),
                }
            });
    
        futures::future::join_all(ping_futures).await;
    
        println!(" JOIN: Successfully joined the network");
        Ok(())
    }

    async fn generate_pow(&self, difficulty: usize) -> ([u8; 8], [u8; 32]) {
    
        let mut nonce: u64 = 0;
        let target_prefix = vec![0u8; difficulty];
    
        loop {
            let mut input = Vec::new();
            input.extend_from_slice(self.get_id());
            input.extend_from_slice(&nonce.to_be_bytes());
    
            let mut hasher = Sha256::new();
            hasher.update(&input);
            let result = hasher.finalize();
    
            if result[..difficulty] == target_prefix[..] {
                return (nonce.to_be_bytes(), result.into());
            }
    
            nonce = nonce.wrapping_add(1); 
        }
    }

    pub fn verify_pow(&self, node_id: &[u8], nonce: &[u8], pow_hash: &[u8], difficulty: usize) -> bool {

        let mut input = Vec::new();
        input.extend_from_slice(node_id);
        input.extend_from_slice(nonce);
        
        let mut hasher = Sha256::new();
        hasher.update(&input);
        let computed_hash = hasher.finalize();
        
        computed_hash[..difficulty] == vec![0u8; difficulty][..] && 
        computed_hash.as_slice() == pow_hash
    }

}
