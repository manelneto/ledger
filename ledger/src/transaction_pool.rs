use super::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use super::transaction::TxHash;

pub struct TransactionPool{
    transactions: HashMap<TxHash, Transaction>,
}

impl TransactionPool{
    pub fn new() -> Self{
        TransactionPool{
            transactions: HashMap::new(),
        }
    }

    pub fn add_transaction(&mut self, tx: Transaction) -> Result<(), &'static str>{
        if !tx.verify(){
            return Err("Transaction signature is invalid");
        }

        if self.transactions.contains_key(&tx.tx_hash){
            return Err("Transaction already exists in the pool");
        }

        self.transactions.insert(tx.tx_hash.clone(), tx);
        Ok(())
    }

    pub fn get_transaction(&self, tx_hash: &TxHash) -> Option<&Transaction>{
        self.transactions.get(tx_hash)
    }

    pub fn remove_transaction(&mut self, tx_hash: &TxHash) -> Option<Transaction>{
        self.transactions.remove(tx_hash)
    }

    pub fn get_all_transactions(&self) -> Vec<&Transaction>{
        self.transactions.values().collect()
    }

    pub fn size(&self) -> usize{
        self.transactions.len()
    }

    pub fn clear(&mut self){
        self.transactions.clear();
    }

    pub fn get_transactions_4_block(&self, max_size: usize) -> Vec<Transaction>{
        let mut txs: Vec<&Transaction> = self.transactions.values().collect();
        txs.sort_by(|a, b| b.data.fee.cmp(&a.data.fee));

        txs.into_iter().take(max_size).cloned().collect()
    }
}

pub struct SharedTransactionPool{
    pool: Arc<Mutex<TransactionPool>>,
}

impl SharedTransactionPool{
    pub fn new() -> Self{
        SharedTransactionPool{
            pool: Arc::new(Mutex::new(TransactionPool::new())),
        }
    }

    pub fn add_transaction(&self, tx: Transaction) -> Result<(), &'static str>{
        let mut pool = self.pool.lock().unwrap();
        pool.add_transaction(tx)
    }

    pub fn remove_transaction(&self, tx_hash: &TxHash) -> Option<Transaction> {
        let mut pool = self.pool.lock().unwrap();
        pool.remove_transaction(tx_hash)
    }

    pub fn get_transaction(&self, tx_hash: &TxHash) -> Option<Transaction>{
        let pool = self.pool.lock().unwrap();
        pool.get_transaction(tx_hash).cloned()
    }

    pub fn get_all_transactions(&self) -> Vec<Transaction>{
        let pool = self.pool.lock().unwrap();
        pool.get_all_transactions().into_iter().cloned().collect()
    }

    pub fn get_transactions_4_block(&self, max_size: usize) -> Vec<Transaction>{
        let pool = self.pool.lock().unwrap();
        pool.get_transactions_4_block(max_size)
    }

    pub fn size(&self) -> usize{
        let pool = self.pool.lock().unwrap();
        pool.size()
    }

    pub fn clear(&self){
        let mut pool = self.pool.lock().unwrap();
        pool.clear();
    }

    pub fn clone(&self) -> Self{
        SharedTransactionPool{
            pool: Arc::clone(&self.pool),
        }
    }
}
