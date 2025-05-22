use crate::auction::auction::collect_auctions;
use crate::auction::auction::AuctionStatus;
use crate::auction::auction_commands::tx_bid;
use crate::auction::auction_commands::tx_create_auction;
use crate::auction::auction_commands::tx_end_auction;
use crate::auction::auction_commands::tx_start_auction;
use crate::auction::auction_commands::AuctionCommand;
use crate::ledger::blockchain::Blockchain;
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

    #[test]
fn test_auction_lifecycle() {
    // Initialize blockchain and keypairs
    let mut blockchain = Blockchain::new();
    let mut csprng = OsRng;
    let owner_keypair = Keypair::generate(&mut csprng);
    let bidder1_keypair = Keypair::generate(&mut csprng);
    let bidder2_keypair = Keypair::generate(&mut csprng);

    // Give some initial balances
    blockchain.balances.insert(owner_keypair.public.to_bytes().to_vec(), 1000);
    blockchain.balances.insert(bidder1_keypair.public.to_bytes().to_vec(), 500);
    blockchain.balances.insert(bidder2_keypair.public.to_bytes().to_vec(), 500);

    let auction_id = "auction-1".to_string();

    // 1. Create auction
    let create_tx = tx_create_auction(
        &owner_keypair,
        auction_id.clone(),
        "Rare Painting".to_string(),
        "A beautiful renaissance painting".to_string(),
        blockchain.get_next_nonce(&owner_keypair.public.to_bytes().to_vec()),
    ).unwrap();
    blockchain.add_transaction(create_tx).unwrap();
    blockchain.mine_block(10).unwrap();

    // 2. Start auction
    let start_tx = tx_start_auction(
        &owner_keypair,
        auction_id.clone(),
        blockchain.get_next_nonce(&owner_keypair.public.to_bytes().to_vec()),
    ).unwrap();
    blockchain.add_transaction(start_tx).unwrap();
    blockchain.mine_block(10).unwrap();

    // 3. Place bids
    let bid1_tx = tx_bid(
        &bidder1_keypair,
        auction_id.clone(),
        100,
        blockchain.get_next_nonce(&bidder1_keypair.public.to_bytes().to_vec()),
    ).unwrap();
    blockchain.add_transaction(bid1_tx).unwrap();

    let bid2_tx = tx_bid(
        &bidder2_keypair,
        auction_id.clone(),
        150,
        blockchain.get_next_nonce(&bidder2_keypair.public.to_bytes().to_vec()),
    ).unwrap();
    blockchain.add_transaction(bid2_tx).unwrap();
    blockchain.mine_block(10).unwrap();

    // 4. End auction
    let end_tx = tx_end_auction(
        &owner_keypair,
        auction_id.clone(),
        blockchain.get_next_nonce(&owner_keypair.public.to_bytes().to_vec()),
    ).unwrap();
    blockchain.add_transaction(end_tx).unwrap();
    blockchain.mine_block(10).unwrap();

    // Collect all transactions
    let all_txs: Vec<Transaction> = blockchain.blocks
        .iter()
        .flat_map(|block| block.transactions.clone())
        .collect();

    let auctions = collect_auctions(&all_txs);

    // ✅ Assert auction exists
    let auction = auctions.get(&auction_id)
        .expect("Auction should be found in collected auctions");

    // ✅ Assert final state
    assert_eq!(auction.status, AuctionStatus::Ended, "Auction should have ended");
    assert_eq!(
        auction.highest_bid,
        Some((150, bidder2_keypair.public.to_bytes().to_vec())),
        "Highest bid should be 150 from bidder2"
    );

    // ✅ Optional: Print auction state (useful for debugging)
    println!("Auction State:");
    println!("ID: {}", auction.auction_id);
    println!("Title: {}", auction.title);
    println!("Status: {:?}", auction.status);
    println!("Owner: {}", hex::encode(&auction.owner));
    match &auction.highest_bid {
        Some((amount, bidder)) => {
            println!("Highest bid: {} from {}", amount, hex::encode(bidder));
        }
        None => println!("No bids placed"),
    }
}
