use crate::auction::auction::collect_auctions;
use crate::auction::auction::AuctionStatus;
use crate::auction::auction_commands::AuctionCommand;
use crate::ledger::transaction::{Transaction, TransactionData, TransactionType};
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;

// Helper functions
fn test_keypair() -> Keypair {
    Keypair::generate(&mut OsRng)
}

use sha2::{Sha256, Digest};

fn create_auction_tx(
    sender: &Keypair,
    command: AuctionCommand,
    timestamp: u128,
    nonce: u64,
) -> Transaction {
    let data = command.to_data_string().unwrap();
    let tx_data = TransactionData {
        tx_type: TransactionType::Data,
        sender: sender.public.to_bytes().to_vec(),
        data: Some(data),
        timestamp,
        receiver: None, 
        amount: Some(0),     
        nonce,
        fee: 0,
        valid_until: Some(timestamp + 86400),
    };
    
    // Calculate transaction hash
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_string(&tx_data).unwrap());
    let tx_hash = hasher.finalize().to_vec();
    
    Transaction {
        tx_hash,  // Add the calculated hash
        data: tx_data,
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