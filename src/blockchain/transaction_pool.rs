use crate::constants::{MAX_POOL_SIZE, MAX_TXS_PER_SENDER, MIN_FEE_RATE};
use crate::blockchain::transaction::{NonceTracker, PublicKey, Transaction, TransactionType, TxHash};
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

#[derive(Clone)]
pub struct PoolTransaction {
    pub transaction: Transaction,
    pub added_at: Instant,
    pub fee_per_byte: u64,
}

pub struct TransactionPool {
    transactions: HashMap<TxHash, PoolTransaction>,
    by_sender: HashMap<PublicKey, BTreeMap<u64, TxHash>>,
    nonce_tracker: NonceTracker,
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

    pub fn add_transaction(&mut self, tx: Transaction) -> Result<(), &'static str> {
        if self.transactions.len() >= MAX_POOL_SIZE {
            self.remove_lowest_fee_transaction()?;
        }

        if !tx.verify() {
            return Err("Transaction signature is invalid");
        }

        if self.transactions.contains_key(&tx.tx_hash) {
            return Err("Transaction already exists in the pool");
        }

        let sender = &tx.data.sender;
        let sender_count = self.sender_counts.get(sender).unwrap_or(&0);
        if *sender_count >= MAX_TXS_PER_SENDER {
            return Err("Too many transactions from sender");
        }

        if let Some(sender_txs) = self.by_sender.get(sender) {
            if let Some((&highest_nonce, _)) = sender_txs.last_key_value() {
                if tx.data.nonce > highest_nonce + 1 {
                    return Err("Nonce gap - missing previous transactions");
                }
            }

            if sender_txs.contains_key(&tx.data.nonce) {
                return Err("Duplicate nonce - transaction already exists");
            }
        }

        let tx_size = self.estimate_transaction_size(&tx);
        let fee_per_byte = tx.data.fee / tx_size;
        if fee_per_byte < MIN_FEE_RATE {
            return Err("Transaction fee too low");
        }

        let pool_tx = PoolTransaction {
            transaction: tx.clone(),
            added_at: Instant::now(),
            fee_per_byte,
        };

        self.transactions.insert(tx.tx_hash.clone(), pool_tx);

        self.by_sender
            .entry(sender.clone())
            .or_insert_with(BTreeMap::new)
            .insert(tx.data.nonce, tx.tx_hash.clone());

        *self.sender_counts.entry(sender.clone()).or_insert(0) += 1;

        self.total_size += tx_size as usize;

        Ok(())
    }

    pub fn remove_transaction(&mut self, tx_hash: &TxHash) -> Option<Transaction> {
        if let Some(pool_tx) = self.transactions.remove(tx_hash) {
            let tx = &pool_tx.transaction;
            let sender = &tx.data.sender;

            if let Some(sender_txs) = self.by_sender.get_mut(sender) {
                sender_txs.remove(&tx.data.nonce);
                if sender_txs.is_empty() {
                    self.by_sender.remove(sender);
                }
            }

            if let Some(count) = self.sender_counts.get_mut(sender) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.sender_counts.remove(sender);
                }
            }

            self.total_size = self
                .total_size
                .saturating_sub(self.estimate_transaction_size(&tx) as usize);

            return Some(pool_tx.transaction);
        }
        None
    }

    pub fn get_transaction(&self, tx_hash: &TxHash) -> Option<&Transaction> {
        self.transactions
            .get(tx_hash)
            .map(|pool_tx| &pool_tx.transaction)
    }

    pub fn get_all_transactions(&self) -> Vec<&Transaction> {
        self.transactions
            .values()
            .map(|pool_tx| &pool_tx.transaction)
            .collect()
    }

    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    pub fn clear(&mut self) {
        self.transactions.clear();
        self.by_sender.clear();
        self.sender_counts.clear();
        self.total_size = 0;
    }

    pub fn get_transactions_for_block(&self, max_size: usize, max_gas: u64) -> Vec<Transaction> {
        let mut selected = Vec::new();
        let mut total_size = 0;
        let mut total_gas = 0;

        let mut pool_txs: Vec<_> = self.transactions.values().collect();
        pool_txs.sort_by(|a, b| {
            b.fee_per_byte
                .cmp(&a.fee_per_byte)
                .then_with(|| a.transaction.data.nonce.cmp(&b.transaction.data.nonce))
        });

        let mut sender_nonces: HashMap<PublicKey, u64> = HashMap::new();

        for pool_tx in pool_txs {
            let tx = &pool_tx.transaction;
            let tx_size = self.estimate_transaction_size(tx);
            let tx_gas = self.estimate_gas_cost(tx);

            if total_size + tx_size > max_size as u64 || total_gas + tx_gas > max_gas {
                continue;
            }

            let sender = &tx.data.sender;

            let base_nonce = self.nonce_tracker.get_nonce(sender);
            let current_nonce = sender_nonces.get(sender).unwrap_or(&base_nonce);
            let expected_nonce = current_nonce + 1;

            let should_include = if tx.data.nonce == expected_nonce {
                true
            } else if base_nonce == 0 && tx.data.nonce == 1 {
                true
            } else if tx.data.nonce > expected_nonce && tx.data.nonce <= expected_nonce + 5 {
                true
            } else {
                // To avoid replay attacks, usually this would be set to false.
                // Since there is no money involved in the transactions, there is no need to worry about it.
                true
            };

            if should_include {
                selected.push(tx.clone());
                total_size += tx_size;
                total_gas += tx_gas;
                sender_nonces.insert(sender.clone(), tx.data.nonce);

                if selected.len() >= 1000 {
                    break;
                }
            }
        }

        selected
    }

    pub fn get_transactions_4_block(&self, max_transactions: usize) -> Vec<Transaction> {
        let result = self.get_transactions_for_block(max_transactions * 500, 1000000);
        result
    }

    fn remove_lowest_fee_transaction(&mut self) -> Result<(), &'static str> {
        let lowest = self
            .transactions
            .values()
            .min_by_key(|pt| pt.fee_per_byte)
            .ok_or("No transactions to remove")?;

        let hash = lowest.transaction.tx_hash.clone();
        self.remove_transaction(&hash);
        Ok(())
    }

    fn estimate_transaction_size(&self, tx: &Transaction) -> u64 {
        (serde_json::to_vec(tx).unwrap_or_default().len() + 100) as u64
    }

    fn estimate_gas_cost(&self, tx: &Transaction) -> u64 {
        match tx.data.tx_type {
            TransactionType::Transfer => 21000,
            TransactionType::Data => {
                21000 + tx.data.data.as_ref().map_or(0, |d| d.len() as u64 * 68)
            }
        }
    }

    pub fn process_block(&mut self, transactions: &[Transaction]) {
        for tx in transactions {
            self.nonce_tracker
                .validate_and_update(&tx.data.sender, tx.data.nonce);

            self.remove_transaction(&tx.tx_hash);
        }

        self.cleanup_invalid_nonces();
    }

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

    pub fn get_pending_by_sender(&self, sender: &PublicKey) -> Vec<Transaction> {
        if let Some(sender_txs) = self.by_sender.get(sender) {
            sender_txs
                .values()
                .filter_map(|hash| self.transactions.get(hash))
                .map(|pool_tx| pool_tx.transaction.clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn total_memory_usage(&self) -> usize {
        self.total_size
    }
}
