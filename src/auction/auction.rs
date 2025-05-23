use std::collections::HashMap;

use crate::ledger::{blockchain::Blockchain, transaction::{Transaction, TransactionType}};
use serde::{Deserialize, Serialize};
use crate::auction::auction_commands::AuctionCommand;

#[derive(Debug, Clone, PartialEq)]
pub enum AuctionStatus {
    Pending,  // Created but not started
    Active,   // Started and accepting bids
    Ended,    // Manually closed or expired
}


#[derive(Debug, Clone)]
pub struct Auction {
    pub auction_id: String,
    pub status: AuctionStatus,
    pub owner: Vec<u8>,
    pub title: String,
    pub description: String,
    pub created_time: u128,
    pub start_time: Option<u128>,
    pub end_time: Option<u128>,
    pub highest_bid: Option<(u64, Vec<u8>)>,
}

fn find_auction_transactions(blockchain: &Blockchain) -> Vec<&Transaction> {
    let mut auction_txs = Vec::new();
    
    for block in &blockchain.blocks {
        for tx in &block.transactions {
            if tx.data.tx_type != TransactionType::Data { continue; }

            if let Some(data) = &tx.data.data {
                if data.starts_with("AUCTION_") {
                    auction_txs.push(tx);
                }
            }
        }
    }

    auction_txs
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
            AuctionCommand::CreateAuction { id, title, description } => {
                auctions.insert(
                    id.clone(),
                    Auction {
                        auction_id: id,
                        status: AuctionStatus::Pending,
                        owner: tx.data.sender.clone(),
                        title,
                        description,
                        created_time: tx.data.timestamp,
                        start_time: None,  
                        end_time: None,    
                        highest_bid: None,
                    },
                );
            }
            
            AuctionCommand::StartAuction { id } => {
                if let Some(auction) = auctions.get_mut(&id) {
                    if auction.owner == tx.data.sender {
                        auction.start_time = Some(tx.data.timestamp);
                        auction.status= AuctionStatus::Active;
                    }
                }
            }

            AuctionCommand::EndAuction { id } => {
                if let Some(auction) = auctions.get_mut(&id) {
                    if auction.owner == tx.data.sender {
                        auction.end_time = Some(tx.data.timestamp);                        
                        auction.status=AuctionStatus::Ended;
                    }
                }
            }

            AuctionCommand::Bid { id, amount } => {
                    let Some(auction) = auctions.get_mut(&id) else { continue };


                if auction.status != AuctionStatus::Active {
                    continue;
                }
            
                if tx.data.sender == auction.owner {
                    continue;
                }
            
                let is_active_period = match (auction.start_time, auction.end_time) {
                    (Some(start), None) => tx.data.timestamp >= start,
                    (Some(start), Some(end)) => tx.data.timestamp >= start && tx.data.timestamp <= end,
                    _ => false, // Should theoretically never happen since status is Active
                };
            
                if !is_active_period {
                    continue;
                }

                match auction.highest_bid {
                    Some((current_highest, _)) if amount > current_highest => {
                        auction.highest_bid = Some((amount, tx.data.sender.clone()));
                    }
                    None => {
                        auction.highest_bid = Some((amount, tx.data.sender.clone()));
                    }
                    _ => {}
                }
            }
        }
    }

    auctions
}


