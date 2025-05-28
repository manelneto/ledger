use crate::kademlia::constants::{
    ALPHA, CRYPTO_KEY_LENGTH, ID_LENGTH, K, KEY_LENGTH, TIMEOUT, TRIES,
};
use crate::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use crate::kademlia::kademlia_proto::kademlia_server::KademliaServer;
use crate::kademlia::kademlia_proto::{
    FindNodeRequest, FindValueRequest, JoinRequest, Node as ProtoNode, PingRequest, StoreRequest,
};
use crate::kademlia::routing_table::RoutingTable;
use crate::kademlia::service::KademliaService;
use crate::ledger::block::Block;
use crate::ledger::blockchain::Blockchain;
use crate::ledger::transaction::{Transaction, TransactionType};
use crate::ledger::transaction_pool::TransactionPool;
use ed25519_dalek::{Keypair, PublicKey as DalekPublicKey, SecretKey as DalekSecretKey};
use futures::stream::{FuturesUnordered, StreamExt};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::time::{interval, timeout};
use tonic::transport::Server;
use tonic::{Request, Status};

// Constants for blockchain operations
const BLOCK_INTERVAL: Duration = Duration::from_secs(30);
const SYNC_INTERVAL: Duration = Duration::from_secs(60);
const MAX_TRANSACTIONS_PER_BLOCK: usize = 10;
const MAX_NODES_TO_SYNC: usize = 3;

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

// Message types for blockchain sync
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

impl Node {
    pub fn new(address: SocketAddr) -> Self {
        let keypair = Keypair::generate(&mut OsRng);
        let hash = Sha256::digest(keypair.public.to_bytes());
        let id = hash[..ID_LENGTH]
            .try_into()
            .expect("SHA-256 hash length must be 160 bits (20 bytes)");

        Self {
            public_key: keypair.public.to_bytes(),
            private_key: keypair.secret.to_bytes(),
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
        let keypair = Keypair::generate(&mut OsRng);

        Self {
            public_key: keypair.public.to_bytes(),
            private_key: keypair.secret.to_bytes(),
            id,
            address,
            routing_table: Arc::new(RwLock::new(RoutingTable::new(id))),
            storage: Arc::new(RwLock::new(HashMap::new())),
            blockchain: Arc::new(RwLock::new(Blockchain::new())),
            transaction_pool: Arc::new(Mutex::new(TransactionPool::new())),
            is_mining: Arc::new(RwLock::new(false)),
        }
    }

    fn get_keypair(&self) -> Result<Keypair, &'static str> {
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

        let tx_data = crate::ledger::transaction::TransactionData {
            sender: sender.clone(),
            receiver,
            timestamp: crate::ledger::lib::now(),
            tx_type,
            amount,
            data,
            nonce,
            fee,
            valid_until: Some(crate::ledger::lib::now() + 3_600_000),
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

        self.broadcast_transaction(tx).await;
        Ok(())
    }

    async fn broadcast_transaction(&self, tx: Transaction) {
        let message = BlockchainMessage::NewTransaction {
            transaction: tx.clone(),
        };
        let data = serde_json::to_vec(&message).unwrap_or_default();

        let key = tx.tx_hash[..KEY_LENGTH]
            .try_into()
            .unwrap_or([0; KEY_LENGTH]);
        self.store(key, data).await.unwrap_or(());
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

        // Debug: Print what transactions we're trying to mine
        println!(
            "DEBUG: Retrieved {} transactions from pool for mining",
            transactions.len()
        );
        for (i, tx) in transactions.iter().enumerate() {
            println!(
                "  TX {}: {} (valid: {})",
                i + 1,
                hex::encode(&tx.tx_hash[..8]),
                tx.verify()
            );
        }

        let mut block = {
            let blockchain = self.blockchain.read().unwrap();
            blockchain.create_block(transactions)?
        };

        // Debug: Print what transactions are in the block before mining
        println!(
            "DEBUG: Block created with {} transactions",
            block.transactions.len()
        );

        {
            let blockchain = self.blockchain.read().unwrap();
            blockchain.mine_block(&mut block)?;
        }

        // Debug: Print what transactions are in the block after mining
        println!(
            "DEBUG: Block mined with {} transactions",
            block.transactions.len()
        );
        for (i, tx) in block.transactions.iter().enumerate() {
            println!("  Block TX {}: {}", i + 1, hex::encode(&tx.tx_hash[..8]));
        }

        // IMPORTANT: Only add the block if mining was successful
        {
            let mut blockchain = self.blockchain.write().unwrap();
            blockchain.add_block(block.clone())?;
        }

        // IMPORTANT: Only remove transactions from pool AFTER the block is successfully added
        {
            let mut pool = self.transaction_pool.lock().unwrap();
            pool.process_block(&block.transactions);
        }

        self.broadcast_block(block.clone()).await;

        Ok(block)
    }

    async fn broadcast_block(&self, block: Block) {
        println!("Broadcasting new block {} to network", block.index);
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
                    match timeout(
                        Duration::from_secs(5),
                        self.store_at(&node, key, data_clone),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            println!("Successfully broadcast block to {}", node.get_address())
                        }
                        Ok(Err(e)) => {
                            println!("Failed to broadcast block to {}: {}", node.get_address(), e)
                        }
                        Err(_) => println!("Timeout broadcasting block to {}", node.get_address()),
                    }
                });
            }
        }

        let mut completed = 0;
        while let Some(_) = broadcast_futures.next().await {
            completed += 1;
        }

        println!("Broadcasted block to {} nodes", completed);
    }

    pub async fn sync_blockchain(&self) {
        println!("Starting blockchain sync...");

        let current_height = {
            let blockchain = self.blockchain.read().unwrap();
            blockchain.get_block_height()
        };
        println!("Current blockchain height: {}", current_height);

        let nodes = {
            let routing_table = self.routing_table.read().unwrap();
            routing_table.find_closest_nodes(self.get_id(), K)
        };

        if nodes.is_empty() {
            println!("No nodes found for blockchain sync.");
            return;
        }

        println!("Found {} nodes for sync", nodes.len());

        let mut sync_futures = FuturesUnordered::new();

        for node in nodes.iter().take(MAX_NODES_TO_SYNC) {
            if node.get_id() != self.get_id() {
                println!("Requesting blockchain from {}", node.get_address());
                sync_futures.push(self.request_full_blockchain(node.clone()));
            }
        }

        let mut best_blockchain: Option<Blockchain> = None;
        let mut best_height = current_height;
        let mut successful_syncs = 0;

        while let Some(result) = sync_futures.next().await {
            match result {
                Ok(blockchain) => {
                    let height = blockchain.get_block_height();
                    println!("Successfully received blockchain with height: {}", height);
                    successful_syncs += 1;

                    // Accept blockchain if it's valid and has more blocks
                    if blockchain.is_chain_valid(None) {
                        if height > best_height {
                            println!("Found better blockchain with height {}", height);
                            best_height = height;
                            best_blockchain = Some(blockchain);
                        } else if height == best_height && best_blockchain.is_none() {
                            println!(
                                "Found blockchain with same height {} (better than current {})",
                                height, current_height
                            );
                            best_blockchain = Some(blockchain);
                        }
                    } else {
                        println!("Received invalid blockchain - rejecting");
                    }
                }
                Err(e) => {
                    println!("Error syncing from node: {}", e);
                }
            }
        }

        println!(
            "Sync completed: {} successful syncs out of {} attempts",
            successful_syncs, MAX_NODES_TO_SYNC
        );

        if let Some(blockchain) = best_blockchain {
            println!(
                "Updating blockchain from height {} to {}",
                current_height, best_height
            );
            let mut current_blockchain = self.blockchain.write().unwrap();
            *current_blockchain = blockchain;

            let mut pool = self.transaction_pool.lock().unwrap();
            pool.clear(); // Clear the transaction pool after sync

            println!("Successfully synced blockchain to height {}", best_height);
        } else if successful_syncs > 0 {
            println!(
                "Received {} blockchain(s) but none were better than current",
                successful_syncs
            );
        } else {
            println!("No valid blockchains received during sync - keeping current blockchain");
        }
    }

    async fn request_full_blockchain(
        &self,
        node: Node,
    ) -> Result<Blockchain, Box<dyn std::error::Error>> {
        println!("Requesting full blockchain from {}", node.get_address());

        // Create a unique key that includes both node IDs and timestamp to avoid collisions
        let request_key = {
            let mut hasher = Sha256::new();
            hasher.update(b"blockchain_request_v2"); // Version to avoid old keys
            hasher.update(&self.id);
            hasher.update(&node.get_id());
            hasher.update(&crate::ledger::lib::now().to_be_bytes());
            // Add some randomness
            hasher.update(&rand::random::<[u8; 8]>());
            let hash = hasher.finalize();
            hash[..KEY_LENGTH].try_into().unwrap_or([0; KEY_LENGTH])
        };

        // Create response key (where we expect the response)
        let response_key = {
            let mut hasher = Sha256::new();
            hasher.update(b"blockchain_response_v2");
            hasher.update(&request_key);
            let hash = hasher.finalize();
            hash[..KEY_LENGTH].try_into().unwrap_or([0; KEY_LENGTH])
        };

        println!("DEBUG: Using request key: {:02x?}", &request_key[..8]);
        println!("DEBUG: Expecting response key: {:02x?}", &response_key[..8]);

        // Store the request with the response key embedded
        let request_with_response_key = format!("REQUEST:{}", hex::encode(&response_key));
        self.store_at(&node, request_key, request_with_response_key.into_bytes())
            .await?;

        // Wait for response to be processed and stored
        tokio::time::sleep(Duration::from_millis(3000)).await;

        // Try to retrieve the response multiple times
        for attempt in 1..=3 {
            println!("DEBUG: Attempt {} to retrieve blockchain response", attempt);

            match self.find_value(node.clone(), response_key).await {
                Ok((Some(data), _)) => {
                    println!("DEBUG: Retrieved response data of {} bytes", data.len());

                    // Try to deserialize the response
                    match serde_json::from_slice::<BlockchainMessage>(&data) {
                        Ok(message) => match message {
                            BlockchainMessage::ResponseFullBlockchain { blockchain } => {
                                println!(
                                    "Successfully received full blockchain with {} blocks",
                                    blockchain.get_block_height()
                                );
                                return Ok(blockchain);
                            }
                            other => {
                                println!(
                                    "DEBUG: Unexpected message type in response: {:?}",
                                    std::mem::discriminant(&other)
                                );
                            }
                        },
                        Err(e) => {
                            println!(
                                "DEBUG: Failed to deserialize response (attempt {}): {}",
                                attempt, e
                            );
                            if data.len() < 200 {
                                println!(
                                    "DEBUG: Response data: {:?}",
                                    String::from_utf8_lossy(&data)
                                );
                            }
                        }
                    }
                }
                Ok((None, _)) => {
                    println!("DEBUG: No response found (attempt {})", attempt);
                }
                Err(e) => {
                    println!(
                        "DEBUG: Error retrieving response (attempt {}): {}",
                        attempt, e
                    );
                }
            }

            if attempt < 3 {
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
        }

        Err("Failed to receive valid blockchain response after 3 attempts".into())
    }

    pub async fn handle_blockchain_message(&self, data: &[u8]) -> Option<Vec<u8>> {
        // Check if this is a new-style request with response key
        if let Ok(text) = std::str::from_utf8(data) {
            if text.starts_with("REQUEST:") {
                let response_key_hex = &text[8..]; // Remove "REQUEST:" prefix
                if let Ok(response_key_bytes) = hex::decode(response_key_hex) {
                    if response_key_bytes.len() == KEY_LENGTH {
                        println!(
                            "Received new-style blockchain request, response key: {:02x?}",
                            &response_key_bytes[..8]
                        );

                        let blockchain = self.blockchain.read().unwrap().clone();

                        // Create simplified blockchain to avoid serialization issues
                        let safe_blockchain = Blockchain {
                            blocks: blockchain.blocks.clone(),
                            difficulty: blockchain.difficulty,
                            forks: HashMap::new(), // Clear forks to avoid serialization issues
                            balances: HashMap::new(), // Clear balances to avoid key serialization issues
                        };

                        let response = BlockchainMessage::ResponseFullBlockchain {
                            blockchain: safe_blockchain,
                        };

                        match serde_json::to_vec(&response) {
                            Ok(response_data) => {
                                println!(
                                    "Prepared blockchain response ({} bytes)",
                                    response_data.len()
                                );

                                // Store the response at the specified key
                                let response_key: [u8; KEY_LENGTH] =
                                    response_key_bytes.try_into().unwrap();
                                tokio::spawn({
                                    let node = self.clone();
                                    async move {
                                        let storage = node.get_storage();
                                        let mut storage_guard = storage.write().unwrap();
                                        storage_guard.insert(response_key, response_data);
                                        println!(
                                            "Stored blockchain response at key: {:02x?}",
                                            &response_key[..8]
                                        );
                                    }
                                });

                                return Some(b"OK".to_vec()); // Acknowledge the request
                            }
                            Err(e) => {
                                println!("Failed to serialize blockchain response: {}", e);
                            }
                        }
                    }
                }
            }
        }

        // Try to parse as a blockchain message (legacy handling)
        if let Ok(message) = serde_json::from_slice::<BlockchainMessage>(data) {
            println!(
                "DEBUG: Handling legacy blockchain message: {:?}",
                std::mem::discriminant(&message)
            );

            match message {
                BlockchainMessage::RequestFullBlockchain => {
                    println!("Received legacy blockchain request - preparing response");
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
                            println!(
                                "Legacy blockchain response prepared ({} bytes)",
                                response_data.len()
                            );
                            return Some(response_data);
                        }
                        Err(e) => {
                            println!("Failed to serialize legacy blockchain response: {}", e);
                        }
                    }
                }

                BlockchainMessage::NewBlock { block } => {
                    println!("Received new block: {}", block.index);
                    if let Err(e) = self.receive_new_block(block).await {
                        println!("Failed to process new block: {}", e);
                    }
                }

                BlockchainMessage::NewTransaction { transaction } => {
                    println!(
                        "Received new transaction: {}",
                        hex::encode(&transaction.tx_hash[..8])
                    );
                    if let Err(e) = self.receive_new_transaction(transaction).await {
                        println!("Failed to process new transaction: {}", e);
                    }
                }

                _ => {
                    println!("DEBUG: Other blockchain message type");
                }
            }
        } else {
            println!(
                "DEBUG: Could not parse as blockchain message - {} bytes",
                data.len()
            );
        }

        None
    }

    async fn receive_new_block(&self, block: Block) -> Result<(), &'static str> {
        let mut blockchain = self.blockchain.write().unwrap();

        // Validate and add the block
        match blockchain.receive_block(block.clone()) {
            Ok(_) => {
                println!("Successfully added new block {} to blockchain", block.index);

                // Remove transactions from pool that are now confirmed
                let mut pool = self.transaction_pool.lock().unwrap();
                pool.process_block(&block.transactions);

                Ok(())
            }
            Err(e) => {
                println!("Failed to add new block: {}", e);
                Err(e)
            }
        }
    }

    async fn receive_new_transaction(&self, transaction: Transaction) -> Result<(), &'static str> {
        if !transaction.verify() {
            return Err("Invalid transaction signature");
        }

        let mut pool = self.transaction_pool.lock().unwrap();
        match pool.add_transaction(transaction.clone()) {
            Ok(_) => {
                println!(
                    "Added new transaction to pool: {}",
                    hex::encode(&transaction.tx_hash[..8])
                );
                Ok(())
            }
            Err(e) => {
                println!("Failed to add transaction to pool: {}", e);
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
                    match node.mine_block().await {
                        Ok(block) => {
                            println!(
                                "Mined block {} with hash {}",
                                block.index,
                                hex::encode(&block.hash[0..8])
                            );
                        }
                        Err(e) => {
                            println!("Mining error: {}", e);
                        }
                    }
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

    // Get blockchain info
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

        println!(
            "BOOTSTRAP: sending FIND_NODE request to boostrap node ({})...",
            bootstrap_node
        );

        let request = Request::new(FindNodeRequest {
            sender: Some(self.to_send()),
            id: self.id.to_vec(),
        });

        let response = client.find_node(request).await?.into_inner();

        println!(
            "BOOTSTRAP: received FIND_NODE response from boostrap node ({})!",
            bootstrap_node
        );

        let mut routing_table = self
            .routing_table
            .write()
            .map_err(|_| Status::internal("BOOTSTRAP: failed to acquire lock on routing table"))?;

        for proto in response.nodes {
            if let Some(node) = Node::from_sender(&proto) {
                routing_table.update(node);
            }
        }

        println!("BOOTSTRAP: successfully updated routing table.");
        //println!("{}", routing_table);
        drop(routing_table);
        println!("BOOTSTRAP: syncing blockchain...");
        self.sync_blockchain().await;

        Ok(())
    }

    pub async fn ping(&self, target: &Node) -> Result<bool, Box<dyn std::error::Error>> {
        let mut client =
            KademliaClient::connect(format!("http://{}", target.get_address())).await?;

        for _ in 0..TRIES {
            println!("PING: sending PING request to {}...", target);

            let request = Request::new(PingRequest {
                sender: Some(self.to_send()),
            });

            let result = timeout(Duration::from_millis(TIMEOUT), client.ping(request)).await;

            if let Ok(Ok(response)) = result {
                println!("PING: received PING response from {}!", target);
                return Ok(response.into_inner().alive);
            }
        }

        println!("PING: did not receive PING response from {}!", target);
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

        println!("STORE: sending STORE request to {}...", target);

        let request = Request::new(StoreRequest {
            sender: Some(self.to_send()),
            key: key.to_vec(),
            value,
        });

        let response = client.store(request).await?.into_inner();

        println!("STORE: received STORE response from {}!", target);

        Ok(response.success)
    }

    pub async fn find_node(
        &self,
        target: Node,
        id: [u8; ID_LENGTH],
    ) -> Result<Vec<Node>, Box<dyn std::error::Error>> {
        let mut client =
            KademliaClient::connect(format!("http://{}", target.get_address())).await?;

        println!("FIND_NODE: sending FIND_NODE request to {}...", target);

        let request = Request::new(FindNodeRequest {
            sender: Some(self.to_send()),
            id: id.to_vec(),
        });

        let response = client.find_node(request).await?.into_inner();

        println!("FIND_NODE: received FIND_NODE response from {}!", target);

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

        println!("FIND_VALUE: sending FIND_VALUE request to {}...", target);

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

        println!("FIND_VALUE: received FIND_VALUE response from {}!", target);

        Ok((value, nodes))
    }

    pub async fn join(
        &self,
        bootstrap_node: Node,
        difficulty: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.ping(&bootstrap_node).await? {
            return Err("JOIN: error pinging boostrap node!".into());
        }

        let (nonce, pow_hash) = self.generate_pow(difficulty).await;

        println!(
            "JOIN: sending JOIN request to bootstrap node ({})...",
            bootstrap_node
        );

        let mut client =
            KademliaClient::connect(format!("http://{}", bootstrap_node.get_address())).await?;

        let request = Request::new(JoinRequest {
            sender: Some(self.to_send()),
            nonce: nonce.to_vec(),
            pow_hash: pow_hash.to_vec(),
        });

        let response = client.join(request).await?.into_inner();

        println!(
            "JOIN: received JOIN response from bootstrap node ({})!",
            bootstrap_node
        );

        if !response.accepted {
            return Err("JOIN: request rejected by bootstrap node!".into());
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
                                println!(
                                    "JOIN: successfully pinged {}, so updated routing table.",
                                    node
                                );
                            }
                            Err(_) => {
                                println!("JOIN: failed to acquire lock on routing table.");
                            }
                        },
                        _ => println!(
                            "JOIN: failed to ping {}, so did not update routing table.",
                            node
                        ),
                    }
                }
            });

        futures::future::join_all(ping_futures).await;
        self.sync_blockchain().await;
        println!("JOIN: successfully joined the network.");
        Ok(())
    }

    pub async fn iterative_find_node(&self, target: [u8; ID_LENGTH]) -> Vec<Node> {
        let mut closest: Vec<Node> = {
            let routing_table_lock = self.get_routing_table();
            let routing_table = routing_table_lock
                .read()
                .expect("ITERATIVE_FIND_NODE: failed to read routing table");
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
                .expect("ITERATIVE_FIND_VALUE: failed to read routing table");
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

        println!("START: started {}!", self);

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
