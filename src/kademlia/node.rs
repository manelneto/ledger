use crate::blockchain::block::Block;
use crate::blockchain::blockchain::Blockchain;
use crate::blockchain::transaction::{Transaction, TransactionType};
use crate::blockchain::transaction_pool::TransactionPool;
use crate::constants::{ALPHA, BLOCK_INTERVAL, CRYPTO_KEY_LENGTH, ID_LENGTH, K, KEY_LENGTH, MAX_NODES_TO_SYNC, MAX_TRANSACTIONS_PER_BLOCK, SYNC_INTERVAL, TIMEOUT, TRIES};
use crate::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use crate::kademlia::kademlia_proto::kademlia_server::KademliaServer;
use crate::kademlia::kademlia_proto::{
    FindNodeRequest, FindValueRequest, JoinRequest, Node as ProtoNode, PingRequest, StoreRequest,
};
use crate::kademlia::routing_table::RoutingTable;
use crate::kademlia::service::KademliaService;
use ed25519_dalek::{Keypair, PublicKey as DalekPublicKey, SecretKey as DalekSecretKey};
use futures::stream::{FuturesUnordered, StreamExt};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use std::{fmt, fs};
use tokio::time::{interval, timeout};
use tonic::transport::Server;
use tonic::{Request, Status};

#[derive(Clone)]
pub struct Node {
    public_key: [u8; CRYPTO_KEY_LENGTH],
    private_key: [u8; CRYPTO_KEY_LENGTH],
    id: [u8; ID_LENGTH],
    address: SocketAddr,
    routing_table: Arc<RwLock<RoutingTable>>,
    storage: Arc<RwLock<HashMap<[u8; KEY_LENGTH], Vec<u8>>>>,
    blockchain: Arc<RwLock<Blockchain>>,
    transaction_pool: Arc<Mutex<TransactionPool>>,
    is_mining: Arc<RwLock<bool>>,
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node ID = {} @ {}", hex::encode(self.id), self.address)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum BlockchainMessage {
    RequestFullBlockchain,
    ResponseFullBlockchain { blockchain: Blockchain },
    ResponseBlocks { blocks: Vec<Block> },
    NewBlock { block: Block },
    NewTransaction { transaction: Transaction },
    RequestTransactionPool,
    ResponseTransactionPool { transactions: Vec<Transaction> },
}

#[derive(Serialize, Deserialize)]
struct StoredKeyData {
    public_key: [u8; CRYPTO_KEY_LENGTH],
    private_key: [u8; CRYPTO_KEY_LENGTH],
}

impl Node {
    pub fn new(address: SocketAddr) -> Self {
        let (public_key, private_key) = Self::get_or_create_keypair(address);
        let hash = Sha256::digest(public_key);
        let id = hash[..ID_LENGTH]
            .try_into()
            .expect("SHA-256 hash length must be 160 bits (20 bytes)");

        Self {
            public_key,
            private_key,
            id,
            address,
            routing_table: Arc::new(RwLock::new(RoutingTable::new(id))),
            storage: Arc::new(RwLock::new(HashMap::new())),
            blockchain: Arc::new(RwLock::new(Blockchain::new())),
            transaction_pool: Arc::new(Mutex::new(TransactionPool::new())),
            is_mining: Arc::new(RwLock::new(false)),
        }
    }

    pub fn new_with_id(address: SocketAddr, id: [u8; ID_LENGTH]) -> Self {
        let (public_key, private_key) = Self::get_or_create_keypair(address);

        Self {
            public_key,
            private_key,
            id,
            address,
            routing_table: Arc::new(RwLock::new(RoutingTable::new(id))),
            storage: Arc::new(RwLock::new(HashMap::new())),
            blockchain: Arc::new(RwLock::new(Blockchain::new())),
            transaction_pool: Arc::new(Mutex::new(TransactionPool::new())),
            is_mining: Arc::new(RwLock::new(false)),
        }
    }

    fn get_or_create_keypair(address: SocketAddr) -> ([u8; CRYPTO_KEY_LENGTH], [u8; CRYPTO_KEY_LENGTH]) {
        let ip_str = address.ip().to_string().replace(":", "_");
        let key_file_path = format!("keys/{}_{}.json", ip_str, address.port());

        if let Ok(existing_keys) = Self::load_keypair_from_file(&key_file_path) {
            return existing_keys;
        }

        let keypair = Keypair::generate(&mut OsRng);
        let public_key = keypair.public.to_bytes();
        let private_key = keypair.secret.to_bytes();

        let _ = Self::save_keypair_to_file(&key_file_path, &public_key, &private_key);

        (public_key, private_key)
    }

    fn load_keypair_from_file(file_path: &str) -> Result<([u8; CRYPTO_KEY_LENGTH], [u8; CRYPTO_KEY_LENGTH]), Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(file_path)?;
        let stored_data: StoredKeyData = serde_json::from_str(&contents)?;
        Ok((stored_data.public_key, stored_data.private_key))
    }

    fn save_keypair_to_file(
        file_path: &str,
        public_key: &[u8; CRYPTO_KEY_LENGTH],
        private_key: &[u8; CRYPTO_KEY_LENGTH],
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = Path::new(file_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let stored_data = StoredKeyData {
            public_key: *public_key,
            private_key: *private_key,
        };

        let json_data = serde_json::to_string_pretty(&stored_data)?;
        fs::write(file_path, json_data)?;

        Ok(())
    }

    pub fn get_keypair(&self) -> Result<Keypair, &'static str> {
        let secret =
            DalekSecretKey::from_bytes(&self.private_key).map_err(|_| "Invalid private key")?;
        let public =
            DalekPublicKey::from_bytes(&self.public_key).map_err(|_| "Invalid public key")?;
        Ok(Keypair { secret, public })
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

    pub fn get_blockchain(&self) -> Arc<RwLock<Blockchain>> {
        self.blockchain.clone()
    }

    pub fn get_transaction_pool(&self) -> Arc<Mutex<TransactionPool>> {
        self.transaction_pool.clone()
    }

    pub async fn create_transaction(
        &self,
        receiver: Option<Vec<u8>>,
        tx_type: TransactionType,
        amount: Option<u64>,
        data: Option<String>,
    ) -> Result<Transaction, &'static str> {
        let blockchain = self.blockchain.read().unwrap();
        let sender = self.public_key.to_vec();
        let nonce = blockchain.get_next_nonce(&sender);

        let fee = match tx_type {
            TransactionType::Transfer => 1000,
            TransactionType::Data => 500,
        };

        let tx_data = crate::blockchain::transaction::TransactionData {
            sender: sender.clone(),
            receiver,
            timestamp: crate::blockchain::lib::now(),
            tx_type,
            amount,
            data,
            nonce,
            fee,
            valid_until: Some(crate::blockchain::lib::now() + 3_600_000),
        };

        let keypair = self.get_keypair()?;
        let tx = Transaction::create_signed(tx_data, &keypair);
        Ok(tx)
    }

    pub async fn submit_transaction(&self, tx: Transaction) -> Result<(), &'static str> {
        if !tx.verify() {
            return Err("Invalid transaction signature");
        }

        {
            let mut pool = self.transaction_pool.lock().unwrap();
            pool.add_transaction(tx.clone())?;
        }

        Ok(())
    }

    pub async fn mine_block(&self) -> Result<Block, &'static str> {
        {
            let mut mining = self.is_mining.write().unwrap();
            if *mining {
                return Err("Already Mining");
            }
            *mining = true;
        }

        let result = self.mine_pow_block().await;

        {
            let mut mining = self.is_mining.write().unwrap();
            *mining = false;
        }

        result
    }

    async fn mine_pow_block(&self) -> Result<Block, &'static str> {
        let transactions = {
            let pool = self.transaction_pool.lock().unwrap();
            pool.get_transactions_4_block(MAX_TRANSACTIONS_PER_BLOCK)
        };

        let mut block = {
            let blockchain = self.blockchain.read().unwrap();
            blockchain.create_block(transactions)?
        };

        {
            let blockchain = self.blockchain.read().unwrap();
            blockchain.mine_block(&mut block)?;
        }

        {
            let mut blockchain = self.blockchain.write().unwrap();
            blockchain.add_block(block.clone())?;
        }

        {
            let mut pool = self.transaction_pool.lock().unwrap();
            pool.process_block(&block.transactions);
        }

        self.broadcast_block(block.clone()).await;

        Ok(block)
    }

    async fn broadcast_block(&self, block: Block) {
        let message = BlockchainMessage::NewBlock {
            block: block.clone(),
        };
        let data = serde_json::to_vec(&message).unwrap_or_default();

        let nodes = {
            let routing_table = self.routing_table.read().unwrap();
            routing_table.find_closest_nodes(self.get_id(), K)
        };

        let mut broadcast_futures = FuturesUnordered::new();

        for node in nodes {
            if node.get_id() != self.get_id() {
                let data_clone = data.clone();
                let key = block.hash[..KEY_LENGTH]
                    .try_into()
                    .unwrap_or([0; KEY_LENGTH]);

                broadcast_futures.push(async move {
                    let _ = timeout(
                        Duration::from_secs(5),
                        self.store_at(&node, key, data_clone),
                    )
                        .await;
                });
            }
        }

        while let Some(_) = broadcast_futures.next().await {}
    }

    pub async fn sync_blockchain(&self) {
        let current_height = {
            let blockchain = self.blockchain.read().unwrap();
            blockchain.get_block_height()
        };

        let nodes = {
            let routing_table = self.routing_table.read().unwrap();
            routing_table.find_closest_nodes(self.get_id(), K)
        };

        if nodes.is_empty() {
            return;
        }

        let mut sync_futures = FuturesUnordered::new();

        for node in nodes.iter().take(MAX_NODES_TO_SYNC) {
            if node.get_id() != self.get_id() {
                sync_futures.push(self.request_full_blockchain(node.clone()));
            }
        }

        let mut best_blockchain: Option<Blockchain> = None;
        let mut best_height = current_height;

        while let Some(result) = sync_futures.next().await {
            if let Ok(blockchain) = result {
                let height = blockchain.get_block_height();

                if blockchain.is_chain_valid(None) {
                    if height > best_height {
                        best_height = height;
                        best_blockchain = Some(blockchain);
                    } else if height == best_height && best_blockchain.is_none() {
                        best_blockchain = Some(blockchain);
                    }
                }
            }
        }

        if let Some(blockchain) = best_blockchain {
            let mut current_blockchain = self.blockchain.write().unwrap();
            *current_blockchain = blockchain;

            let mut pool = self.transaction_pool.lock().unwrap();
            pool.clear();
        }
    }

    async fn request_full_blockchain(
        &self,
        node: Node,
    ) -> Result<Blockchain, Box<dyn std::error::Error>> {
        let request_key = {
            let mut hasher = Sha256::new();
            hasher.update(b"blockchain_request_v2");
            hasher.update(&self.id);
            hasher.update(&node.get_id());
            hasher.update(&crate::blockchain::lib::now().to_be_bytes());
            hasher.update(&rand::random::<[u8; 8]>());
            let hash = hasher.finalize();
            hash[..KEY_LENGTH].try_into().unwrap_or([0; KEY_LENGTH])
        };

        let response_key = {
            let mut hasher = Sha256::new();
            hasher.update(b"blockchain_response_v2");
            hasher.update(&request_key);
            let hash = hasher.finalize();
            hash[..KEY_LENGTH].try_into().unwrap_or([0; KEY_LENGTH])
        };

        let request_with_response_key = format!("REQUEST:{}", hex::encode(&response_key));
        self.store_at(&node, request_key, request_with_response_key.into_bytes())
            .await?;

        tokio::time::sleep(Duration::from_millis(3000)).await;

        for _ in 1..=3 {
            match self.find_value(node.clone(), response_key).await {
                Ok((Some(data), _)) => {
                    match serde_json::from_slice::<BlockchainMessage>(&data) {
                        Ok(message) => match message {
                            BlockchainMessage::ResponseFullBlockchain { blockchain } => {
                                return Ok(blockchain);
                            }
                            _ => {}
                        },
                        Err(_) => {}
                    }
                }
                _ => {}
            }

            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        Err("Failed to receive valid blockchain response after 3 attempts".into())
    }

    pub async fn handle_blockchain_message(&self, data: &[u8]) -> Option<Vec<u8>> {
        if let Ok(text) = std::str::from_utf8(data) {
            if text.starts_with("REQUEST:") {
                let response_key_hex = &text[8..];
                if let Ok(response_key_bytes) = hex::decode(response_key_hex) {
                    if response_key_bytes.len() == KEY_LENGTH {
                        let blockchain = self.blockchain.read().unwrap().clone();

                        let safe_blockchain = Blockchain {
                            blocks: blockchain.blocks.clone(),
                            difficulty: blockchain.difficulty,
                            forks: HashMap::new(),
                            balances: HashMap::new(),
                        };

                        let response = BlockchainMessage::ResponseFullBlockchain {
                            blockchain: safe_blockchain,
                        };

                        match serde_json::to_vec(&response) {
                            Ok(response_data) => {
                                let response_key: [u8; KEY_LENGTH] =
                                    response_key_bytes.try_into().unwrap();
                                tokio::spawn({
                                    let node = self.clone();
                                    async move {
                                        let storage = node.get_storage();
                                        let mut storage_guard = storage.write().unwrap();
                                        storage_guard.insert(response_key, response_data);
                                    }
                                });

                                return Some(b"OK".to_vec());
                            }
                            Err(_) => {}
                        }
                    }
                }
            }
        }

        if let Ok(message) = serde_json::from_slice::<BlockchainMessage>(data) {
            match message {
                BlockchainMessage::RequestFullBlockchain => {
                    let blockchain = self.blockchain.read().unwrap().clone();

                    let safe_blockchain = Blockchain {
                        blocks: blockchain.blocks.clone(),
                        difficulty: blockchain.difficulty,
                        forks: HashMap::new(),
                        balances: HashMap::new(),
                    };

                    let response = BlockchainMessage::ResponseFullBlockchain {
                        blockchain: safe_blockchain,
                    };

                    match serde_json::to_vec(&response) {
                        Ok(response_data) => {
                            return Some(response_data);
                        }
                        Err(_) => {}
                    }
                }

                BlockchainMessage::NewBlock { block } => {
                    let _ = self.receive_new_block(block).await;
                }
                _ => {}
            }
        }

        None
    }

    async fn receive_new_block(&self, block: Block) -> Result<(), &'static str> {
        println!("\n\nReceived block {}", block.index);

        let mut blockchain = self.blockchain.write().unwrap();
        match blockchain.receive_block(block.clone()) {
            Ok(_) => {
                let mut pool = self.transaction_pool.lock().unwrap();
                pool.process_block(&block.transactions);
                println!("Successfully added block {} to blockchain\n", block.index);
                Ok(())
            }
            Err(e) => {
                println!("Failed to add block {} to blockchain\n", e);
                Err(e)
            }
        }
    }

    pub async fn start_mining(&self) {
        let node = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(BLOCK_INTERVAL);
            loop {
                interval.tick().await;

                if !*node.is_mining.read().unwrap() {
                    let _ = node.mine_block().await;
                }
            }
        });
    }

    pub async fn start_syncing(&self) {
        let node = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(SYNC_INTERVAL);
            loop {
                interval.tick().await;
                node.sync_blockchain().await;
            }
        });
    }

    pub fn get_blockchain_info(&self) -> (usize, Option<String>) {
        let blockchain = self.blockchain.read().unwrap();
        let height = blockchain.get_block_height();
        let last_hash = blockchain.get_last_block().map(|b| hex::encode(&b.hash));
        (height, last_hash)
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
            blockchain: Arc::new(RwLock::new(Blockchain::new())),
            transaction_pool: Arc::new(Mutex::new(TransactionPool::new())),
            is_mining: Arc::new(RwLock::new(false)),
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
        let mut client =
            KademliaClient::connect(format!("http://{}", bootstrap_node.get_address())).await?;

        let request = Request::new(FindNodeRequest {
            sender: Some(self.to_send()),
            id: self.id.to_vec(),
        });

        let response = client.find_node(request).await?.into_inner();

        let mut routing_table = self
            .routing_table
            .write()
            .map_err(|_| Status::internal("failed to acquire lock on routing table"))?;

        for proto in response.nodes {
            if let Some(node) = Node::from_sender(&proto) {
                routing_table.update(node);
            }
        }

        drop(routing_table);
        self.sync_blockchain().await;

        Ok(())
    }

    pub async fn ping(&self, target: &Node) -> Result<bool, Box<dyn std::error::Error>> {
        let mut client =
            KademliaClient::connect(format!("http://{}", target.get_address())).await?;

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

    pub async fn store(
        &self,
        key: [u8; KEY_LENGTH],
        value: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    pub async fn store_at(
        &self,
        target: &Node,
        key: [u8; KEY_LENGTH],
        value: Vec<u8>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let mut client =
            KademliaClient::connect(format!("http://{}", target.get_address())).await?;

        let request = Request::new(StoreRequest {
            sender: Some(self.to_send()),
            key: key.to_vec(),
            value,
        });

        let response = client.store(request).await?.into_inner();

        Ok(response.success)
    }

    pub async fn find_node(
        &self,
        target: Node,
        id: [u8; ID_LENGTH],
    ) -> Result<Vec<Node>, Box<dyn std::error::Error>> {
        let mut client =
            KademliaClient::connect(format!("http://{}", target.get_address())).await?;

        let request = Request::new(FindNodeRequest {
            sender: Some(self.to_send()),
            id: id.to_vec(),
        });

        let response = client.find_node(request).await?.into_inner();

        let nodes = response
            .nodes
            .into_iter()
            .filter_map(|proto| Node::from_sender(&proto))
            .collect();

        Ok(nodes)
    }

    pub async fn find_value(
        &self,
        target: Node,
        key: [u8; KEY_LENGTH],
    ) -> Result<(Option<Vec<u8>>, Vec<Node>), Box<dyn std::error::Error>> {
        let mut client =
            KademliaClient::connect(format!("http://{}", target.get_address())).await?;

        let request = Request::new(FindValueRequest {
            sender: Some(self.to_send()),
            key: key.to_vec(),
        });

        let response = client.find_value(request).await?.into_inner();

        let value = response.value;
        let nodes = response
            .nodes
            .into_iter()
            .filter_map(|proto| Node::from_sender(&proto))
            .collect();

        Ok((value, nodes))
    }

    pub async fn join(
        &self,
        bootstrap_node: Node,
        difficulty: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.ping(&bootstrap_node).await? {
            return Err("error pinging boostrap node!".into());
        }

        let (nonce, pow_hash) = self.generate_pow(difficulty).await;

        let mut client =
            KademliaClient::connect(format!("http://{}", bootstrap_node.get_address())).await?;

        let request = Request::new(JoinRequest {
            sender: Some(self.to_send()),
            nonce: nonce.to_vec(),
            pow_hash: pow_hash.to_vec(),
        });

        let response = client.join(request).await?.into_inner();

        if !response.accepted {
            return Err("request rejected by bootstrap node!".into());
        }

        let routing_table_lock = self.routing_table.clone();
        let ping_futures = response
            .closest_nodes
            .into_iter()
            .filter_map(|proto| {
                let node = Node::from_sender(&proto)?;
                (node.get_id() != self.get_id()).then_some(node)
            })
            .map(move |node| {
                let routing_table_lock = routing_table_lock.clone();
                async move {
                    match tokio::time::timeout(Duration::from_secs(5), self.ping(&node)).await {
                        Ok(Ok(true)) => match routing_table_lock.write() {
                            Ok(mut routing_table) => {
                                routing_table.update(node.clone());
                            }
                            Err(_) => {}
                        },
                        _ => {}
                    }
                }
            });

        futures::future::join_all(ping_futures).await;
        self.sync_blockchain().await;
        Ok(())
    }

    pub async fn iterative_find_node(&self, target: [u8; ID_LENGTH]) -> Vec<Node> {
        let mut closest: Vec<Node> = {
            let routing_table_lock = self.get_routing_table();
            let routing_table = routing_table_lock
                .read()
                .expect("failed to read routing table");
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
                            timeout(Duration::from_millis(TIMEOUT), self.find_node(node, target))
                                .await
                        });
                    }
                }
            }

            while let Some(Ok(Ok(nodes))) = parallel_requests.next().await {
                for node in nodes {
                    let it = node.get_id().to_vec();
                    if !queried.contains(&it)
                        && !candidates.iter().any(|n| n.get_id() == node.get_id())
                    {
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
            let routing_table = routing_table_lock
                .read()
                .expect("failed to read routing table");
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
                            timeout(Duration::from_millis(TIMEOUT), self.find_value(node, key))
                                .await
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
                    if !queried.contains(&id)
                        && !candidates.iter().any(|n| n.get_id() == node.get_id())
                    {
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

    pub fn verify_pow(
        &self,
        node_id: &[u8],
        nonce: &[u8],
        pow_hash: &[u8],
        difficulty: usize,
    ) -> bool {
        let mut input = Vec::new();
        input.extend_from_slice(node_id);
        input.extend_from_slice(nonce);

        let mut hasher = Sha256::new();
        hasher.update(&input);
        let computed_hash = hasher.finalize();

        computed_hash[..difficulty] == vec![0u8; difficulty][..]
            && computed_hash.as_slice() == pow_hash
    }
}
