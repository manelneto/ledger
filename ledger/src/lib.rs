use std::time::{SystemTime, UNIX_EPOCH};

pub type BHash = Vec<u8>;
//pub type Address = String; Não sei o que é ainda

// Get the current time in seconds since the UNIX epoch
pub fn now() -> u128 {
    let epoch_duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    epoch_duration.as_secs() as u128 * 1000 + epoch_duration.subsec_millis() as u128
}

pub fn u32_to_bytes(n: &u32) -> [u8; 4] {
    n.to_be_bytes()
}

pub fn u64_to_bytes(n: &u64) -> [u8; 8] {
    n.to_be_bytes()
}

pub fn u128_to_bytes(n: &u128) -> [u8; 16] {
    n.to_be_bytes()
}

pub fn bytes_to_u32(v: &[u8]) -> u128 {
    let len = v.len();
    if len < 16 {
        panic!("Input vector must have at least 16 bytes");
    }

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&v[len - 16..]);
    u128::from_be_bytes(bytes) 
}

mod block;
pub use block::Block;
mod hashable;
pub use hashable::Hashable;
mod blockchain;
pub use blockchain::Blockchain;
mod transaction;
pub use transaction::{Transaction, TransactionData, TransactionType};
mod transaction_pool;
pub use transaction_pool::{TransactionPool, SharedTransactionPool};