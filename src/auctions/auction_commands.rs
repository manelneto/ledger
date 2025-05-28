use crate::blockchain::transaction::Transaction;
use ed25519_dalek::Keypair;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize, Debug)]
pub enum AuctionCommand {
    CreateAuction {
        id: String,
        title: String,
        description: String,
    },
    StartAuction {
        id: String,
    },
    EndAuction {
        id: String,
    },
    Bid {
        id: String,
        amount: u64,
    },
}

impl AuctionCommand {
    pub fn to_data_string(&self) -> Result<String, serde_json::Error> {
        let serialized = serde_json::to_string(self)?;
        Ok(format!("AUCTION_{}", serialized))
    }
}

pub fn create_auction_tx(
    key_pair: &Keypair,
    command: AuctionCommand,
    nonce: u64,
) -> Result<Transaction, &'static str> {
    let data = command
        .to_data_string()
        .map_err(|_| "Failed to serialize auction command")?;

    Transaction::create_data_tx(key_pair, data, nonce, 0)
}

pub fn generate_auction_id(public_key: &[u8], title: &str, description: &str, nonce: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(public_key);
    hasher.update(title.as_bytes());
    hasher.update(description.as_bytes());
    hasher.update(&nonce.to_le_bytes());

    format!("{:x}", hasher.finalize())[..16].to_string()
}

pub fn tx_create_auction(
    key_pair: &Keypair,
    title: String,
    description: String,
    nonce: u64,
) -> Result<Transaction, &'static str> {
    let id = generate_auction_id(&key_pair.public.to_bytes(), &title, &description, nonce);
    let command = AuctionCommand::CreateAuction {
        id,
        title,
        description,
    };
    create_auction_tx(key_pair, command, nonce)
}

pub fn tx_start_auction(
    key_pair: &Keypair,
    id: String,
    nonce: u64,
) -> Result<Transaction, &'static str> {
    create_auction_tx(key_pair, AuctionCommand::StartAuction { id }, nonce)
}

pub fn tx_end_auction(
    key_pair: &Keypair,
    id: String,
    nonce: u64,
) -> Result<Transaction, &'static str> {
    create_auction_tx(key_pair, AuctionCommand::EndAuction { id }, nonce)
}

pub fn tx_bid(
    key_pair: &Keypair,
    id: String,
    amount: u64,
    nonce: u64,
) -> Result<Transaction, &'static str> {
    create_auction_tx(key_pair, AuctionCommand::Bid { id, amount }, nonce)
}
