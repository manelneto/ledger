use std::fmt::{ self, Debug, Formatter };
use std::vec;
use crate::ledger::lib::{u128_to_bytes, u32_to_bytes, u64_to_bytes, BHash};
use crate::ledger::transaction::Transaction;
use crate::ledger::merkle_tree::MerkleTree;
use super::*;

#[derive(Clone)]
pub struct Block {
    pub index: u32,
    pub timestamp: u128,
    pub hash: BHash,
    pub prev_hash: BHash,
    pub nonce: u64,
    pub merkle_root: BHash,
    pub transactions: Vec<Transaction>,
    pub tx_count: u32,
}

impl Debug for Block {
    fn  fmt (&self, f: &mut Formatter) -> fmt::Result{
        write!(f,"Block[{}]: {} at: {} with: {}",
               &self.index,
               &hex::encode(&self.hash),
               &self.timestamp,
               &self.tx_count,
               &hex::encode(&self.merkle_root),
        )
    }
}

impl Block{
    pub fn new (index: u32, timestamp: u128, prev_hash: BHash,
                nonce: u64, transactions: Vec<Transaction>) -> Self{
        
        let merkle_tree = MerkleTree::new(&transactions);
        let merkle_root = merkle_tree.get_root_hash().unwrap_or_else(|| vec![0;32]);
        let tx_count = transactions.len() as u32;
        Block{
            index,
            timestamp,
            hash: vec![0;32],
            prev_hash,
            nonce,
            merkle_root,
            transactions,
            tx_count,
        }
    }

    pub fn genesis() -> Self {
        Block { 
            index: 0, 
            timestamp: crate::ledger::lib::now(), 
            hash: vec![0; 32], 
            prev_hash: vec![0; 32], 
            nonce: 0, 
            merkle_root: vec![0; 32], 
            transactions: Vec::new(), 
            tx_count: 0,
        }
    }

    pub fn get_transaction(&self, tx_hash: &[u8]) -> Option<&Transaction> {
        self.transactions.iter().find(|tx| tx.tx_hash == tx_hash)
    }

    pub fn verify_transaction_inclusion(&self, tx_hash: &[u8], proof: &crate::ledger::merkle_tree::MerkleProof) -> bool {
        MerkleTree::verify_proof(&self.merkle_root, tx_hash, proof)
    }

    pub fn generate_inclusion_proof(&self, tx_hash: &[u8]) -> Option<crate::ledger::merkle_tree::MerkleProof> {
        let merkle_tree = MerkleTree::new(&self.transactions);
        merkle_tree.generate_proof(tx_hash)
    }

    pub fn get_header(&self) -> BlockHeader {
        BlockHeader {
            index: self.index,
            timestamp: self.timestamp,
            hash: self.hash.clone(),
            prev_hash: self.prev_hash.clone(),
            nonce: self.nonce,
            merkle_root: self.merkle_root.clone(),
            tx_count: self.tx_count,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlockHeader {
    pub index: u32,
    pub timestamp: u128,
    pub hash: BHash,
    pub prev_hash: BHash,
    pub nonce: u64,
    pub merkle_root: BHash,
    pub tx_count: u32,
}

impl Hashable for Block {
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];

        bytes.extend(&u32_to_bytes(&self.index));
        bytes.extend(&u128_to_bytes(&self.timestamp));
        bytes.extend(&self.prev_hash);
        bytes.extend(&u64_to_bytes(&self.nonce));
        
        // Include Merkle root in hash calculation
        bytes.extend(&self.merkle_root);
        
        // Optional: include transaction count
        bytes.extend(&u32_to_bytes(&self.tx_count));

        bytes
    }
}

impl Hashable for BlockHeader {
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];

        bytes.extend(&u32_to_bytes(&self.index));
        bytes.extend(&u128_to_bytes(&self.timestamp));
        bytes.extend(&self.prev_hash);
        bytes.extend(&u64_to_bytes(&self.nonce));
        bytes.extend(&self.merkle_root);
        bytes.extend(&u32_to_bytes(&self.tx_count));

        bytes
    }
}
