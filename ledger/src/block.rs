use std::fmt::{ self, Debug, Formatter };
use super::*;

pub struct Block {
    pub index: u32,
    pub timestamp: u128,
    pub hash: BHash,
    pub prev_hash: BHash,
    pub nonce: u64,
    pub payload: String,
}

impl Debug for Block {
    fn  fmt (&self, f: &mut Formatter) -> fmt::Result{
        write!(f,"Block[{}]: {} at: {} with: {}",
            &self.index,
            &hex::encode(&self.hash),
            &self.timestamp,
            &self.payload,
            )
    }
}

impl Block{
    pub fn new (index: u32, timestamp: u128, prev_hash: BHash, 
    nonce: u64, payload: String) -> Self{
        Block{
            index,
            timestamp,
            hash: vec![0;32],
            prev_hash,
            nonce,
            payload,
        }
    }

}


impl Hashable for Block{
    fn bytes (&self) -> Vec<u8> {
        let bytes = vec![];
        
        bytes
    }
}