// Module: blockchain
use super::*;
use std::vec;
use std::collections::{HashMap, HashSet};
use serde_json;
use crate::ledger::block::Block;
use crate::ledger::lib::{now, BHash};
use crate::ledger::transaction::{Transaction, PublicKey, TransactionType};
use crate::ledger::transaction_pool::SharedTransactionPool;
use std::time::{Duration, Instant};
use crate::ledger::merkle_tree::{MerkleTree, MerkleProof};
use ed25519_dalek::Keypair;

const DIFFICULTY_PREFIX: &str = "0000";
const MAX_BLOCK_TIME: u128 = 600_000; // 10 minutes
const MIN_BLOCK_TIME: u128 = 1_000; // 1 second (fix: U128 -> u128)
const MAX_MINING_TIME: Duration = Duration::from_secs(300);
const MAX_FORK_DEPTH: usize = 6;

pub struct Blockchain {
    pub blocks: Vec<Block>,
    pub uncofirmed_transactions: SharedTransactionPool,
    pub difficulty: usize,
    pub forks: HashMap<BHash, Vec<Block>>,
    balances: HashMap<Vec<u8>, u64>,
}

// Light client struct (was missing declaration)
pub struct LightClient {
    headers: Vec<crate::ledger::block::BlockHeader>,
}

impl Blockchain {
    pub fn new() -> Self {
        let mut chain = Blockchain {
            blocks: Vec::new(),
            uncofirmed_transactions: SharedTransactionPool::new(),
            difficulty: DIFFICULTY_PREFIX.len(),
            forks: HashMap::new(),
            balances: HashMap::new(),
        };

        chain.create_genesis_block();
        chain
    }

    fn create_genesis_block(&mut self) {
        let genesis_block = Block::genesis();

        // Calculate the hash of the genesis block
        let hash = genesis_block.hash();
        let mut genesis = genesis_block;
        genesis.hash = hash;

        self.blocks.push(genesis);
    }

    pub fn add_block(&mut self, block: Block) -> Result<(), &'static str> {
        let last_block = match self.blocks.last() {
            Some(block) => block,
            None => return Err("Blockchain has no blocks"),
        };

        if block.prev_hash != last_block.hash {
            return Err("Block has invalid previous hash");
        }

        if block.index != last_block.index + 1 {
            return Err("Block has invalid index");
        }

        let time_diff = block.timestamp.saturating_sub(last_block.timestamp);
        if time_diff < MIN_BLOCK_TIME {
            return Err("Block time is too short");
        }

        if time_diff > MAX_BLOCK_TIME {
            return Err("Block time is too long");
        }

        if !self.is_block_hash_valid(&block.hash()) {
            return Err("Block hash doesn't meet difficulty requirements");
        }

        self.validate_transactions(&block)?;

        self.proccess_block_transactions(&block)?;

        self.blocks.push(block);
        Ok(())
    }

    fn validate_transactions(&self, block: &Block) -> Result<(), &'static str> {
        if block.index == 0 || block.transactions.is_empty() {
            return Ok(());
        }

        // Check for duplicate transactions in block
        let mut tx_hashes = HashSet::new();
        for tx in &block.transactions {
            if !tx_hashes.insert(tx.tx_hash.clone()) {
                return Err("Duplicate transaction in block");
            }

            // Verify transaction signature
            if !tx.verify() {
                return Err("Block contains invalid transaction signature");
            }

            // Check balance for transfers
            if let Some(amount) = tx.data.amount {
                let sender_balance = self.balances.get(&tx.data.sender).unwrap_or(&0);
                let total_cost = amount + tx.data.fee;
                
                if *sender_balance < total_cost {
                    return Err("Insufficient balance for transaction");
                }
            }
        }

        Ok(())
    }

    fn proccess_block_transactions(&mut self, block: &Block) -> Result<(), &'static str> {
        if block.index == 0 || block.transactions.is_empty() {
            return Ok(());
        }

        // Process each transaction in the block
        for tx in &block.transactions {
            if !tx.verify() {
                return Err("Block contains invalid transaction");
            }

            // Update balances
            match tx.data.tx_type {
                TransactionType::Transfer => {
                    if let Some(amount) = tx.data.amount {
                        // Deduct from sender
                        let sender_balance = self.balances.entry(tx.data.sender.clone())
                            .or_insert(0);
                        *sender_balance = sender_balance.saturating_sub(amount + tx.data.fee);
                        
                        // Add to receiver
                        if let Some(receiver) = &tx.data.receiver {
                            let receiver_balance = self.balances.entry(receiver.clone())
                                .or_insert(0);
                            *receiver_balance += amount;
                        }
                    }
                },
                TransactionType::Data => {
                    // Just deduct fee for data transactions
                    let sender_balance = self.balances.entry(tx.data.sender.clone())
                        .or_insert(0);
                    *sender_balance = sender_balance.saturating_sub(tx.data.fee);
                }
            }

            // Remove from transaction pool
            self.uncofirmed_transactions.remove_transaction(&tx.tx_hash);
        }

        Ok(())
    }

    pub fn add_transaction(&mut self, tx: Transaction) -> Result<(), &'static str> {
        // Check if transaction is already in a block
        for block in &self.blocks {
            if self.is_transaction_in_block(block, &tx.tx_hash) {
                return Err("Transaction already in blockchain");
            }
        }
        
        // Verify transaction signature
        if !tx.verify() {
            return Err("Invalid transaction signature");
        }
        
        // Check balance for transfers
        if let Some(amount) = tx.data.amount {
            let sender_balance = self.balances.get(&tx.data.sender).unwrap_or(&0);
            let total_cost = amount + tx.data.fee;
            
            if *sender_balance < total_cost {
                return Err("Insufficient balance");
            }
        }
        
        self.uncofirmed_transactions.add_transaction(tx)
    }

    // Get the next nonce for a sender
    pub fn get_next_nonce(&self, sender: &PublicKey) -> u64 {
        // Check transaction pool first
        let pending_txs = self.uncofirmed_transactions.get_pending_by_sender(sender);
        let pool_max_nonce = pending_txs.iter().map(|tx| tx.data.nonce).max();
        
        // Check blockchain for last confirmed nonce
        let mut confirmed_nonce = 0;
        for block in &self.blocks {
            for tx in &block.transactions {
                if tx.data.sender == *sender && tx.data.nonce > confirmed_nonce {
                    confirmed_nonce = tx.data.nonce;
                }
            }
        }
        
        // Return the highest of confirmed or pending nonce + 1
        std::cmp::max(confirmed_nonce, pool_max_nonce.unwrap_or(0)) + 1
    }

    // Get balance for an address
    pub fn get_balance(&self, address: &PublicKey) -> u64 {
        *self.balances.get(address).unwrap_or(&0)
    }

    // Proof of Work: Mining
    pub fn mine_block(&mut self, max_transactions: usize) -> Result<Block, &'static str> {
        let last_block = match self.blocks.last() {
            Some(block) => block,
            None => return Err("Blockchain has no blocks"),
        };

        // Use the secure method with gas limits
        let transactions = self.uncofirmed_transactions.get_transactions_for_block(
            max_transactions * 500, // Assuming 500 bytes per tx on average
            1000000 // Gas limit
        );

        if transactions.is_empty() && max_transactions > 0 {
            return Err("No valid transactions available");
        }

        // Create a new block with the transactions
        let mut new_block = Block::new(
            last_block.index + 1,
            now(),
            last_block.hash.clone(),
            0,
            transactions.clone(),
        );

        // Find a valid hash through proof of work
        self.proof_of_work(&mut new_block)?;

        // Add to blockchain
        self.add_block(new_block.clone())?;

        // Process the block to update nonces
        self.uncofirmed_transactions.process_block(&transactions);

        Ok(new_block)
    }

    // Mine an empty block
    pub fn mine_empty_block(&mut self) -> Result<Block, &'static str> {
        let last_block = match self.blocks.last() {
            Some(block) => block,
            None => return Err("Blockchain has no blocks"),
        };

        let mut new_block = Block::new(
            last_block.index + 1,
            now(),
            last_block.hash.clone(),
            0,
            Vec::new(), // Empty transactions
        );

        self.proof_of_work(&mut new_block)?;
        self.add_block(new_block.clone())?;

        Ok(new_block)
    }

    pub fn proof_of_work(&self, block: &mut Block) -> Result<(), &'static str> {
        println!("Mining block {:?}", block);

        let start_time = Instant::now();
        let target = "0".repeat(self.difficulty);

        loop {
            // Timeout protection
            if start_time.elapsed() > MAX_MINING_TIME {
                return Err("Mining timed out - adjust difficulty");
            }

            block.hash = block.hash();
            let hash_str = hex::encode(&block.hash);

            if hash_str.starts_with(&target) {
                println!("Found valid hash: {} in {:?}", hash_str, start_time.elapsed());
                return Ok(());
            }

            block.nonce += 1;

            if block.nonce == u64::MAX {
                return Err("Nonce overflow - difficulty too high");
            }
        }
    }

    fn is_block_hash_valid(&self, hash: &[u8]) -> bool {
        let hash_string = hex::encode(hash);
        let target = "0".repeat(self.difficulty);
        hash_string.starts_with(&target)
    }

    pub fn is_chain_valid(&self, chain: Option<&Vec<Block>>) -> bool {
        let chain_to_validate = chain.unwrap_or(&self.blocks);

        for i in 0..chain_to_validate.len() {
            let current_block = &chain_to_validate[i];

            // Verify block hash
            if current_block.hash != current_block.hash() {
                println!("Current block hash is invalid");
                return false;
            }

            // Verify Merkle root
            if !self.verify_block_merkle_root(current_block) {
                println!("Block Merkle root is invalid");
                return false;
            }

            // Check links between blocks
            if i > 0 {
                let prev_block = &chain_to_validate[i - 1];
                
                if current_block.prev_hash != prev_block.hash {
                    println!("Current block previous hash is invalid");
                    return false;
                }

                if !self.is_block_hash_valid(&current_block.hash()) {
                    println!("Current block hash doesn't meet difficulty requirements");
                    return false;
                }
            }
        }
        true
    }

    pub fn get_last_block(&self) -> Option<&Block> {
        self.blocks.last()
    }

    pub fn receive_block(&mut self, block: Block) -> Result<(), &'static str> {
        if !self.is_block_hash_valid(&block.hash()) {
            return Err("Block hash doesn't meet difficulty requirements");
        }

        let current_time = now();
        if block.timestamp > current_time + 7_200_000 {
            return Err("Block timestamp is too far in the future");
        }

        if let Some(last_block) = self.blocks.last() {
            if block.prev_hash == last_block.hash && block.index == last_block.index + 1 {
                return self.add_block(block);
            }
        }

        for (i, existing_block) in self.blocks.iter().enumerate() {
            if block.prev_hash == existing_block.hash {
                if (self.blocks.len() - i) > MAX_FORK_DEPTH {
                    return Err("Fork depth exceeded");
                }

                let mut fork_chain = self.blocks[0..=i].to_vec();

                if !self.validate_fork_chain(&fork_chain, &block) {
                    return Err("Invalid fork chain");
                }

                fork_chain.push(block.clone());
                self.forks.insert(existing_block.hash.clone(), fork_chain);
                println!("Fork created from block index {}", i);

                self.resolve_forks();
                return Ok(());
            }
        }

        let fork_key_to_update = self
            .forks
            .iter()
            .find_map(|(key, fork_chain)| {
                fork_chain.last().and_then(|last_block| {
                    if block.prev_hash == last_block.hash && block.index == last_block.index + 1 {
                        Some(key.clone())
                    } else {
                        None
                    }
                })
            });

        if let Some(key) = fork_key_to_update {
            let is_valid = {
                if let Some(fork_chain) = self.forks.get(&key) {
                    self.validate_block_for_fork(fork_chain, &block)
                } else {
                    false
                }
            };

            if is_valid {
                if let Some(fork_chain) = self.forks.get_mut(&key) {
                    fork_chain.push(block.clone());
                    println!("Block added to existing fork");
                    self.resolve_forks();
                    return Ok(());
                }
            }
        }

        Err("Block doesn't fit in any chain")
    }


    // Enhanced fork chain validation
    fn validate_fork_chain(&self, fork_chain: &Vec<Block>, new_block: &Block) -> bool {
        // Validate entire fork chain
        for i in 1..fork_chain.len() {
            let current = &fork_chain[i];
            let prev = &fork_chain[i-1];
            
            if current.prev_hash != prev.hash {
                return false;
            }
            
            if !self.is_block_hash_valid(&current.hash()) {
                return false;
            }
        }
        
        // Validate new block against last fork block
        let last_fork = fork_chain.last().unwrap();
        new_block.prev_hash == last_fork.hash && 
        new_block.index == last_fork.index + 1 &&
        self.is_block_hash_valid(&new_block.hash())
    }

    // Validate block for existing fork
    fn validate_block_for_fork(&self, fork_chain: &Vec<Block>, block: &Block) -> bool {
        // Check timestamp against fork chain
        if let Some(last_fork_block) = fork_chain.last() {
            let time_diff = block.timestamp.saturating_sub(last_fork_block.timestamp);
            if time_diff < MIN_BLOCK_TIME || time_diff > MAX_BLOCK_TIME {
                return false;
            }
        }
        
        // Verify block hash
        self.is_block_hash_valid(&block.hash())
    }

    fn get_cumulative_work(&self, chain: &Vec<Block>) -> u128 {
        chain.iter().map(|block| {
            let leading_zeros = block.hash.iter()
                .take_while(|&&byte| byte == 0)
                .count() * 8 + 
                block.hash.iter()
                .find(|&&byte| byte != 0)
                .map_or(0, |&byte| byte.leading_zeros() as usize);
            2u128.pow(leading_zeros as u32)
        }).sum()
    }

    pub fn resolve_forks(&mut self) {
        let main_chain_length = self.blocks.len();
        let mut forks_to_remove = Vec::new();
        let fork_keys: Vec<_> = self.forks.keys().cloned().collect(); // evita empréstimos diretos

        for fork_key in fork_keys {
            let fork_chain_cloned = self.forks.get(&fork_key).cloned();

            if let Some(fork_chain) = fork_chain_cloned {
                if fork_chain.len() > main_chain_length + MAX_FORK_DEPTH {
                    println!("Rejecting excessively long fork");
                    forks_to_remove.push(fork_key.clone());
                    continue;
                }

                if self.is_chain_valid(Some(&fork_chain)) && fork_chain.len() > main_chain_length {
                    if self.get_cumulative_work(&fork_chain) > self.get_cumulative_work(&self.blocks) {
                        println!("Found longer valid fork (length: {})", fork_chain.len());

                        self.revert_to_fork_state(&fork_chain);
                        self.blocks = fork_chain.clone();

                        forks_to_remove.extend(self.forks.keys().cloned());
                        break;
                    }
                }

                if !self.is_chain_valid(Some(&fork_chain)) {
                    forks_to_remove.push(fork_key.clone());
                }
            }
        }

        for key in forks_to_remove {
            self.forks.remove(&key);
        }
    }

    fn revert_to_fork_state(&mut self, fork_chain: &Vec<Block>) {
        // Reset balances
        self.balances.clear();
        
        // Replay transactions from fork chain
        for block in fork_chain {
            let _ = self.proccess_block_transactions(block);
        }
    }

    fn is_transaction_in_block(&self, block: &Block, tx_hash: &Vec<u8>) -> bool {
        block.transactions.iter().any(|tx| tx.tx_hash == *tx_hash)
    }

    // Simulate fork creation for testing
    pub fn simulate_fork(&mut self, transactions: Vec<Transaction>) -> Result<Block, &'static str> {
        if self.blocks.len() < 2 {
            return Err("Need at least 2 blocks to create a fork");
        }

        let fork_base_index = self.blocks.len() - 2;
        let fork_base = &self.blocks[fork_base_index];

        let mut fork_block = Block::new(
            fork_base.index + 1,
            now(),
            fork_base.hash.clone(),
            0,
            transactions,
        );

        self.proof_of_work(&mut fork_block)?;

        match self.receive_block(fork_block.clone()) {
            Ok(_) => Ok(fork_block),
            Err(e) => Err(e),
        }
    }

    // Get transaction inclusion proof
    pub fn get_transaction_proof(&self, block_index: usize, tx_hash: &[u8]) -> Option<MerkleProof> {
        if let Some(block) = self.blocks.get(block_index) {
            block.generate_inclusion_proof(tx_hash)
        } else {
            None
        }
    }

    // Verify transaction inclusion across the chain
    pub fn verify_transaction_in_chain(&self, tx_hash: &[u8], proof: &MerkleProof, block_index: usize) -> bool {
        if let Some(block) = self.blocks.get(block_index) {
            block.verify_transaction_inclusion(tx_hash, proof)
        } else {
            false
        }
    }

    // Get block headers only (for light clients)
    pub fn get_block_headers(&self) -> Vec<crate::ledger::block::BlockHeader> {
        self.blocks.iter().map(|block| block.get_header()).collect()
    }

    // Get block header by index
    pub fn get_block_header(&self, index: usize) -> Option<crate::ledger::block::BlockHeader> {
        self.blocks.get(index).map(|block| block.get_header())
    }

    // Verify Merkle root matches transactions
    pub fn verify_block_merkle_root(&self, block: &Block) -> bool {
        let merkle_tree = MerkleTree::new(&block.transactions);
        if let Some(calculated_root) = merkle_tree.get_root_hash() {
            calculated_root == block.merkle_root
        } else {
            block.merkle_root == vec![0; 32] && block.transactions.is_empty()
        }
    }
}

// Light client implementation
impl LightClient {
    pub fn new() -> Self {
        LightClient {
            headers: Vec::new(),
        }
    }

    // Add a block header
    pub fn add_header(&mut self, header: crate::ledger::block::BlockHeader) -> Result<(), &'static str> {
        if let Some(last_header) = self.headers.last() {
            if header.prev_hash != last_header.hash {
                return Err("Invalid header chain");
            }
        }
        
        self.headers.push(header);
        Ok(())
    }

    // Verify transaction proof against known headers
    pub fn verify_transaction(&self, tx_hash: &[u8], proof: &MerkleProof, block_index: usize) -> bool {
        if let Some(header) = self.headers.get(block_index) {
            MerkleTree::verify_proof(&header.merkle_root, tx_hash, proof)
        } else {
            false
        }
    }

    // Get latest block height
    pub fn get_height(&self) -> usize {
        self.headers.len()
    }
}

// Helper functions for creating secure transactions
pub fn create_secure_transfer(
    blockchain: &Blockchain,
    key_pair: &Keypair,
    receiver: PublicKey,
    amount: u64,
    fee: u64,
) -> Result<Transaction, &'static str> {
    let sender = Transaction::get_public_key(key_pair);
    
    // Get next valid nonce
    let nonce = blockchain.get_next_nonce(&sender);
    
    // Check balance
    let balance = blockchain.get_balance(&sender);
    let total_needed = amount + fee;
    
    if balance < total_needed {
        return Err("Insufficient balance");
    }
    
    // Create transaction
    Transaction::create_transfer(key_pair, receiver, amount, nonce, fee)
}

pub fn create_secure_data_tx(
    blockchain: &Blockchain,
    key_pair: &Keypair,
    data: String,
    fee: u64,
) -> Result<Transaction, &'static str> {
    let sender = Transaction::get_public_key(key_pair);
    
    // Get next valid nonce
    let nonce = blockchain.get_next_nonce(&sender);
    
    // Check balance
    let balance = blockchain.get_balance(&sender);
    
    if balance < fee {
        return Err("Insufficient balance for fee");
    }
    
    // Create transaction
    Transaction::create_data_tx(key_pair, data, nonce, fee)
}