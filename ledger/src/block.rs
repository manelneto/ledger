use std::fmt::{ self, Debug, Formatter };
use super::*;

#[derive(Clone)]
pub struct Block {
    pub index: u32,
    pub timestamp: u128,
    pub hash: Hash,
    pub prev_hash: Hash,
    pub nonce: u64,
    pub transactions: Vec<Transaction>,
}

impl Debug for Block {
    fn  fmt (&self, f: &mut Formatter) -> fmt::Result{
        write!(f,"Block[{}]: {} at: {} with: {}",
            &self.index,
            &hex::encode(&self.hash),
            &self.timestamp,
            &self.transactions.len(),
            )
    }
}

impl Block{
    pub fn new (index: u32, timestamp: u128, prev_hash: Hash, 
    transactions:Vec<Transaction>) -> Self{
        Block{
            index,
            timestamp,
            hash: vec![0;32],
            prev_hash,
            nonce: 0,
            transactions,
        }
    }

}

impl Hashable for Block{
    fn bytes (&self) -> Vec<u8> {
        let mut bytes = vec![];
        
        bytes.extend(&u32_to_bytes(&self.index));
        bytes.extend(&u128_to_bytes(&self.timestamp));
        bytes.extend(&self.prev_hash);
        bytes.extend(&u64_to_bytes(&self.nonce));
        bytes.extend(
            self.transactions.iter()
            .flat_map(|transaction| transaction.bytes())
            .collect::<Vec<u8>>()
        );

        bytes
    }
}