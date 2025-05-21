use std::collections::HashMap;

use crate::transaction::{Transaction, TransactionType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Auction {
    pub auction_id: String,
    pub owner: Vec<u8>,
    pub title: String,
    pub description: String,
    pub reserve_price: u64,
    pub start_time: u128,
    pub end_time: u128,
    pub highest_bid: Option<(u64, Vec<u8>)>,
    pub is_closed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AuctionCommand {
    CreateAuction {
        id: String,
        title: String,
        description: String,
        reserve_price: u64,
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

fn parse_auction_command(data: &str) -> Option<AuctionCommand> {
    if let Some(stripped) = data.strip_prefix("AUCTION_") {
        serde_json::from_str(stripped).ok()
    } else {
        None
    }
}

pub fn collect_auctions(transactions: &[Transaction]) -> HashMap<String, Auction> {
    let mut auctions: HashMap<String, Auction> = HashMap::new();

    for tx in transactions {
        if tx.data.tx_type != TransactionType::Data {
            continue;
        }

        let Some(data) = &tx.data.data else { continue };
        if !data.starts_with("AUCTION_") {
            continue;
        }

        let Some(command) = parse_auction_command(data) else { continue };

        match command {
            AuctionCommand::CreateAuction { id, title, description, reserve_price } => {
                auctions.insert(
                    id.clone(),
                    Auction {
                        auction_id: id,
                        owner: tx.data.sender.clone(),
                        title,
                        description,
                        reserve_price,
                        start_time: 0,  // Will be set when auction starts
                        end_time: 0,    // Will be set when auction ends
                        highest_bid: None,
                        is_closed: false,
                    },
                );
            }
            
            AuctionCommand::StartAuction { id } => {
                if let Some(auction) = auctions.get_mut(&id) {
                    if auction.owner == tx.data.sender {
                        auction.start_time = tx.data.timestamp;
                    }
                }
            }

            AuctionCommand::EndAuction { id } => {
                if let Some(auction) = auctions.get_mut(&id) {
                    if auction.owner == tx.data.sender {
                        auction.end_time = tx.data.timestamp;
                        auction.is_closed = true;
                    }
                }
            }
        }
    }

    auctions
}