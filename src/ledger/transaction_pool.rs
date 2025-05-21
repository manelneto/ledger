// src/ledger/transaction_pool.rs
use std::collections::{HashMap, BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};
use crate::ledger::transaction::{Transaction, TxHash, PublicKey, TransactionType, NonceTracker};

// Constants for security limits
const MAX_POOL_SIZE: usize = 10000;
const MAX_TXS_PER_SENDER: usize = 50;
const TX_EXPIRY_TIME: Duration = Duration::from_secs(3600); // 1 hour
const MIN_FEE_RATE: u64 = 100; // Minimum fee per transaction

// Transaction with metadata for the pool
#[derive(Clone)]
pub struct PoolTransaction {
    pub transaction: Transaction,
    pub added_at: Instant,
    pub fee_per_byte: u64,
}

// Main transaction pool
pub struct TransactionPool {
    transactions: HashMap<TxHash, PoolTransaction>,
    by_sender: HashMap<PublicKey, BTreeMap<u64, TxHash>>, // Sender -> nonce -> tx_hash
    nonce_tracker: NonceTracker,
    
    // Rate limiting
    sender_counts: HashMap<PublicKey, usize>,
    total_size: usize,
}

impl TransactionPool {
    pub fn new() -> Self {
        TransactionPool {
            transactions: HashMap::new(),
            by_sender: HashMap::new(),
            nonce_tracker: NonceTracker::new(),
            sender_counts: HashMap::new(),
            total_size: 0,
        }
    }

    // Add transaction to the pool with security checks
    pub fn add_transaction(&mut self, tx: Transaction) -> Result<(), &'static str> {
        // Expire old transactions first
        self.expire_transactions();

        // Check pool size limits
        if self.transactions.len() >= MAX_POOL_SIZE {
            self.remove_lowest_fee_transaction()?;
        }

        // Verify transaction signature and validity
        if !tx.verify() {
            return Err("Transaction signature is invalid");
        }

        // Check for duplicates
        if self.transactions.contains_key(&tx.tx_hash) {
            return Err("Transaction already exists in the pool");
        }

        // Check sender limits
        let sender = &tx.data.sender;
        let sender_count = self.sender_counts.get(sender).unwrap_or(&0);
        if *sender_count >= MAX_TXS_PER_SENDER {
            return Err("Too many transactions from sender");
        }

        // Validate nonce sequence
        if let Some(sender_txs) = self.by_sender.get(sender) {
            let expected_nonce = self.nonce_tracker.get_nonce(sender) + 1;
            
            // Check for gaps in nonce sequence
            if let Some((&highest_nonce, _)) = sender_txs.last_key_value() {
                if tx.data.nonce > highest_nonce + 1 {
                    return Err("Nonce gap - missing previous transactions");
                }
            }
            
            // Check for duplicate nonce
            if sender_txs.contains_key(&tx.data.nonce) {
                return Err("Duplicate nonce - transaction already exists");
            }
        }

        // Check minimum fee rate
        let tx_size = self.estimate_transaction_size(&tx);
        let fee_per_byte = tx.data.fee / tx_size;
        if fee_per_byte < MIN_FEE_RATE {
            return Err("Transaction fee too low");
        }

        // Add to pool
        let pool_tx = PoolTransaction {
            transaction: tx.clone(),
            added_at: Instant::now(),
            fee_per_byte,
        };

        self.transactions.insert(tx.tx_hash.clone(), pool_tx);
        
        // Update by_sender index
        self.by_sender.entry(sender.clone())
            .or_insert_with(BTreeMap::new)
            .insert(tx.data.nonce, tx.tx_hash.clone());
        
        // Update sender count
        *self.sender_counts.entry(sender.clone()).or_insert(0) += 1;
        
        self.total_size += tx_size as usize;
        
        Ok(())
    }

    // Remove transaction from pool
    pub fn remove_transaction(&mut self, tx_hash: &TxHash) -> Option<Transaction> {
        if let Some(pool_tx) = self.transactions.remove(tx_hash) {
            let tx = &pool_tx.transaction;
            let sender = &tx.data.sender;
            
            // Update by_sender index
            if let Some(sender_txs) = self.by_sender.get_mut(sender) {
                sender_txs.remove(&tx.data.nonce);
                if sender_txs.is_empty() {
                    self.by_sender.remove(sender);
                }
            }
            
            // Update sender count
            if let Some(count) = self.sender_counts.get_mut(sender) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.sender_counts.remove(sender);
                }
            }
            
            self.total_size = self.total_size.saturating_sub(self.estimate_transaction_size(&tx) as usize);
            
            return Some(pool_tx.transaction);
        }
        None
    }

    // Get transaction by hash
    pub fn get_transaction(&self, tx_hash: &TxHash) -> Option<&Transaction> {
        self.transactions.get(tx_hash).map(|pool_tx| &pool_tx.transaction)
    }

    // Get all transactions in the pool
    pub fn get_all_transactions(&self) -> Vec<&Transaction> {
        self.transactions.values().map(|pool_tx| &pool_tx.transaction).collect()
    }

    // Get size of the pool
    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    // Clear the pool
    pub fn clear(&mut self) {
        self.transactions.clear();
        self.by_sender.clear();
        self.sender_counts.clear();
        self.total_size = 0;
    }

    // Get best transactions for a block
    pub fn get_transactions_for_block(&self, max_size: usize, max_gas: u64) -> Vec<Transaction> {
        let mut selected = Vec::new();
        let mut total_size = 0;
        let mut total_gas = 0;
        
        // Sort by fee per byte (highest first), then by nonce for same sender
        let mut pool_txs: Vec<_> = self.transactions.values().collect();
        pool_txs.sort_by(|a, b| {
            b.fee_per_byte.cmp(&a.fee_per_byte)
                .then_with(|| a.transaction.data.nonce.cmp(&b.transaction.data.nonce))
        });
        
        // Track nonces for each sender to maintain sequence
        let mut sender_nonces: HashMap<PublicKey, u64> = HashMap::new();
        
        for pool_tx in pool_txs {
            let tx = &pool_tx.transaction;
            let tx_size = self.estimate_transaction_size(tx);
            let tx_gas = self.estimate_gas_cost(tx);
            
            // Check size and gas limits
            if total_size + tx_size > max_size as u64 || total_gas + tx_gas > max_gas {
                continue;
            }
            
            // Check nonce sequence for sender
            let sender = &tx.data.sender;
            let default_nonce = self.nonce_tracker.get_nonce(sender);
            let expected_nonce = sender_nonces.get(sender).unwrap_or(&default_nonce);
            
            if tx.data.nonce != expected_nonce + 1 {
                continue; // Skip if nonce is not next in sequence
            }
            
            selected.push(tx.clone());
            total_size += tx_size;
            total_gas += tx_gas;
            
            sender_nonces.insert(sender.clone(), tx.data.nonce);
            
            if selected.len() >= 1000 { // Max transactions per block
                break;
            }
        }
        
        selected
    }

    // For backward compatibility
    pub fn get_transactions_4_block(&self, max_transactions: usize) -> Vec<Transaction> {
        self.get_transactions_for_block(max_transactions * 500, 1000000)
    }

    // Remove expired transactions
    fn expire_transactions(&mut self) {
        let now = Instant::now();
        let mut expired_hashes = Vec::new();
        
        for (hash, pool_tx) in &self.transactions {
            if now.duration_since(pool_tx.added_at) > TX_EXPIRY_TIME {
                expired_hashes.push(hash.clone());
            }
        }
        
        for hash in expired_hashes {
            self.remove_transaction(&hash);
        }
    }
    
    // Remove transaction with lowest fee when pool is full
    fn remove_lowest_fee_transaction(&mut self) -> Result<(), &'static str> {
        let lowest = self.transactions.values()
            .min_by_key(|pt| pt.fee_per_byte)
            .ok_or("No transactions to remove")?;
        
        let hash = lowest.transaction.tx_hash.clone();
        self.remove_transaction(&hash);
        Ok(())
    }
    
    // Estimate transaction size (for fee calculations)
    fn estimate_transaction_size(&self, tx: &Transaction) -> u64 {
        (serde_json::to_vec(tx).unwrap_or_default().len() + 100) as u64
    }
    
    // Estimate gas cost for a transaction
    fn estimate_gas_cost(&self, tx: &Transaction) -> u64 {
        match tx.data.tx_type {
            TransactionType::Transfer => 21000,
            TransactionType::Data => {
                21000 + tx.data.data.as_ref().map_or(0, |d| d.len() as u64 * 68)
            }
        }
    }
    
    // Update nonces after a block is mined
    pub fn process_block(&mut self, transactions: &[Transaction]) {
        for tx in transactions {
            // Update nonce tracker
            self.nonce_tracker.validate_and_update(&tx.data.sender, tx.data.nonce);
            
            // Remove from pool
            self.remove_transaction(&tx.tx_hash);
        }
        
        // Remove now-invalid transactions (wrong nonces)
        self.cleanup_invalid_nonces();
    }
    
    // Remove transactions with invalid nonces
    fn cleanup_invalid_nonces(&mut self) {
        let mut to_remove = Vec::new();
        
        for (hash, pool_tx) in &self.transactions {
            let tx = &pool_tx.transaction;
            let expected_nonce = self.nonce_tracker.get_nonce(&tx.data.sender) + 1;
            
            if tx.data.nonce < expected_nonce {
                to_remove.push(hash.clone());
            }
        }
        
        for hash in to_remove {
            self.remove_transaction(&hash);
        }
    }
    
    // Get pending transactions by sender (nonce ordered)
    pub fn get_pending_by_sender(&self, sender: &PublicKey) -> Vec<Transaction> {
        if let Some(sender_txs) = self.by_sender.get(sender) {
            sender_txs.values()
                .filter_map(|hash| self.transactions.get(hash))
                .map(|pool_tx| pool_tx.transaction.clone())
                .collect()
        } else {
            Vec::new()
        }
    }
    
    // Get total memory usage of the pool
    pub fn total_memory_usage(&self) -> usize {
        self.total_size
    }
}

// Thread-safe wrapper for transaction pool
pub struct SharedTransactionPool {
    pool: Arc<Mutex<TransactionPool>>,
}

impl SharedTransactionPool {
    pub fn new() -> Self {
        SharedTransactionPool {
            pool: Arc::new(Mutex::new(TransactionPool::new())),
        }
    }

    pub fn add_transaction(&self, tx: Transaction) -> Result<(), &'static str> {
        let mut pool = self.pool.lock().unwrap();
        pool.add_transaction(tx)
    }

    pub fn remove_transaction(&self, tx_hash: &TxHash) -> Option<Transaction> {
        let mut pool = self.pool.lock().unwrap();
        pool.remove_transaction(tx_hash)
    }

    pub fn get_transaction(&self, tx_hash: &TxHash) -> Option<Transaction> {
        let pool = self.pool.lock().unwrap();
        pool.get_transaction(tx_hash).cloned()
    }

    pub fn get_all_transactions(&self) -> Vec<Transaction> {
        let pool = self.pool.lock().unwrap();
        pool.get_all_transactions().into_iter().cloned().collect()
    }

    pub fn get_transactions_4_block(&self, max_size: usize) -> Vec<Transaction> {
        let pool = self.pool.lock().unwrap();
        pool.get_transactions_4_block(max_size)
    }

    // New methods for the secure version
    pub fn get_transactions_for_block(&self, max_size: usize, max_gas: u64) -> Vec<Transaction> {
        let pool = self.pool.lock().unwrap();
        pool.get_transactions_for_block(max_size, max_gas)
    }
    
    pub fn process_block(&self, transactions: &[Transaction]) {
        let mut pool = self.pool.lock().unwrap();
        pool.process_block(transactions);
    }
    
    pub fn get_pending_by_sender(&self, sender: &PublicKey) -> Vec<Transaction> {
        let pool = self.pool.lock().unwrap();
        pool.get_pending_by_sender(sender)
    }
    
    pub fn total_memory_usage(&self) -> usize {
        let pool = self.pool.lock().unwrap();
        pool.total_memory_usage()
    }

    pub fn size(&self) -> usize {
        let pool = self.pool.lock().unwrap();
        pool.size()
    }

    pub fn clear(&self) {
        let mut pool = self.pool.lock().unwrap();
        pool.clear();
    }

    pub fn clone(&self) -> Self {
        SharedTransactionPool {
            pool: Arc::clone(&self.pool),
        }
    }
}