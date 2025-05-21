use std::collections::HashMap;

use crate::transaction::{Transaction, TransactionType};
use serde::{Deserialize, Serialize};
use crate::auction_commands::AuctionCommand;

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



#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::transaction::{Transaction, TransactionData, TransactionType};
    use crate::auction_commands::AuctionCommand;
    use ed25519_dalek::Keypair;
    use rand::rngs::OsRng;

    // Helper to create a test keypair
    fn test_keypair() -> Keypair {
        Keypair::generate(&mut OsRng)
    }

    // Helper to create a data transaction with auction command
    fn create_auction_tx(
        sender: &Keypair,
        command: AuctionCommand,
        timestamp: u128,
        nonce: u64,
    ) -> Transaction {
        let data = command.to_data_string().unwrap();
        Transaction {
            data: TransactionData {
                tx_type: TransactionType::Data,
                sender: sender.public.to_bytes().to_vec(),
                data: Some(data),
                timestamp,
                nonce,
                fee: 0,
                signature: Vec::new(), // Not needed for these tests
            },
            signature: Vec::new(),
        }
    }

    #[test]
    fn test_create_auction() {
        let keypair = test_keypair();
        let transactions = vec![
            create_auction_tx(
                &keypair,
                AuctionCommand::CreateAuction {
                    id: "test1".to_string(),
                    title: "Test Auction".to_string(),
                    description: "Description".to_string(),
                },
                1000,
                1,
            ),
        ];

        let auctions = collect_auctions(&transactions);
        let auction = auctions.get("test1").unwrap();

        assert_eq!(auction.status, AuctionStatus::Pending);
        assert_eq!(auction.title, "Test Auction");
        assert_eq!(auction.owner, keypair.public.to_bytes().to_vec());
        assert_eq!(auction.start_time, None);
        assert_eq!(auction.end_time, None);
        assert_eq!(auction.highest_bid, None);
    }

    #[test]
    fn test_full_auction_lifecycle() {
        let owner = test_keypair();
        let bidder = test_keypair();
        
        let transactions = vec![
            // Create auction
            create_auction_tx(
                &owner,
                AuctionCommand::CreateAuction {
                    id: "lifecycle".to_string(),
                    title: "Lifecycle Test".to_string(),
                    description: "Test".to_string(),
                },
                1000,
                1,
            ),
            // Start auction
            create_auction_tx(
                &owner,
                AuctionCommand::StartAuction {
                    id: "lifecycle".to_string(),
                },
                2000,
                2,
            ),
            // Place bids
            create_auction_tx(
                &bidder,
                AuctionCommand::Bid {
                    id: "lifecycle".to_string(),
                    amount: 100,
                },
                2001,
                1,
            ),
            create_auction_tx(
                &bidder,
                AuctionCommand::Bid {
                    id: "lifecycle".to_string(),
                    amount: 150,
                },
                2002,
                2,
            ),
            // End auction
            create_auction_tx(
                &owner,
                AuctionCommand::EndAuction {
                    id: "lifecycle".to_string(),
                },
                3000,
                3,
            ),
            // Late bid (should be ignored)
            create_auction_tx(
                &bidder,
                AuctionCommand::Bid {
                    id: "lifecycle".to_string(),
                    amount: 200,
                },
                3001,
                3,
            ),
        ];

        let auctions = collect_auctions(&transactions);
        let auction = auctions.get("lifecycle").unwrap();

        assert_eq!(auction.status, AuctionStatus::Ended);
        assert_eq!(auction.start_time, Some(2000));
        assert_eq!(auction.end_time, Some(3000));
        assert_eq!(auction.highest_bid, Some((150, bidder.public.to_bytes().to_vec())));
    }

    #[test]
    fn test_invalid_bids() {
        let owner = test_keypair();
        let bidder = test_keypair();
        
        let transactions = vec![
            create_auction_tx(
                &owner,
                AuctionCommand::CreateAuction {
                    id: "invalid".to_string(),
                    title: "Invalid Bid Test".to_string(),
                    description: "Test".to_string(),
                },
                1000,
                1,
            ),
            // Owner trying to bid (should be ignored)
            create_auction_tx(
                &owner,
                AuctionCommand::Bid {
                    id: "invalid".to_string(),
                    amount: 100,
                },
                1001,
                2,
            ),
            // Bid before auction starts (should be ignored)
            create_auction_tx(
                &bidder,
                AuctionCommand::Bid {
                    id: "invalid".to_string(),
                    amount: 100,
                },
                1001,
                1,
            ),
            // Start auction
            create_auction_tx(
                &owner,
                AuctionCommand::StartAuction {
                    id: "invalid".to_string(),
                },
                2000,
                3,
            ),
            // Valid bid
            create_auction_tx(
                &bidder,
                AuctionCommand::Bid {
                    id: "invalid".to_string(),
                    amount: 100,
                },
                2001,
                1,
            ),
        ];

        let auctions = collect_auctions(&transactions);
        let auction = auctions.get("invalid").unwrap();

        // Only the valid bid should be recorded
        assert_eq!(auction.highest_bid, Some((100, bidder.public.to_bytes().to_vec())));
        assert_eq!(auction.status, AuctionStatus::Active);
    }
}