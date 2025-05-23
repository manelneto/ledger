use blockchain::ledger::blockchain::Blockchain;
use blockchain::ledger::transaction::Transaction;
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;
use std::time::Duration;
use tokio::time::sleep;
use serde_json;

// Helper function to print transaction information
fn print_tx(tx: &Transaction, label: &str) {
    let tx_hash = hex::encode(&tx.tx_hash[0..8]);
    let sender = hex::encode(&tx.data.sender[0..8]);
    
    println!("{} Transaction:", label);
    println!("  Hash: {}", tx_hash);
    println!("  Sender: {}", sender);
    println!("  Fee: {}", tx.data.fee);
    if let Some(ref data) = tx.data.data {
        println!("  Data: {}", data);
    }
    
    // Debug size estimation
    let tx_size = serde_json::to_vec(tx).unwrap_or_default().len() as u64 + 100;
    println!("  Estimated size: {} bytes", tx_size);
    println!("  Fee per byte: {}", tx.data.fee / tx_size);
    println!("  Recommended fee (MIN_FEE_RATE=0): any fee >= 0");
    println!();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Auction Transaction Test ===\n");
    
    // 1. Create a blockchain and transaction pool
    let mut blockchain = Blockchain::new();
    println!("Created new blockchain with genesis block");
    
    // 2. Create accounts for testing
    let mut csprng = OsRng;
    let alice_keypair = Keypair::generate(&mut csprng); // Auctioneer
    let bob_keypair = Keypair::generate(&mut csprng);   // Bidder 1
    let charlie_keypair = Keypair::generate(&mut csprng); // Bidder 2
    
    println!("Generated test keypairs:");
    println!("Alice (Auctioneer): {}", hex::encode(&alice_keypair.public.to_bytes()[0..8]));
    println!("Bob (Bidder 1): {}", hex::encode(&bob_keypair.public.to_bytes()[0..8]));
    println!("Charlie (Bidder 2): {}", hex::encode(&charlie_keypair.public.to_bytes()[0..8]));
    
    // 3. Add funds to accounts for testing
    println!("\n=== Adding Funds to Test Accounts ===");
    
    // In a real blockchain, you would add funds through mining or transfers from existing accounts
    // This is a simplified version for testing that directly modifies balances
    
    // Add funds to Alice
    blockchain.balances.insert(
        alice_keypair.public.to_bytes().to_vec(), 
        1_000_000
    );
    println!("Added 1,000,000 tokens to Alice's account");
    
    // Add funds to Bob
    blockchain.balances.insert(
        bob_keypair.public.to_bytes().to_vec(), 
        500_000
    );
    println!("Added 500,000 tokens to Bob's account");
    
    // Add funds to Charlie
    blockchain.balances.insert(
        charlie_keypair.public.to_bytes().to_vec(), 
        500_000
    );
    println!("Added 500,000 tokens to Charlie's account");
    
    // Verify balances
    println!("Alice's balance: {}", blockchain.get_balance(&alice_keypair.public.to_bytes().to_vec()));
    println!("Bob's balance: {}", blockchain.get_balance(&bob_keypair.public.to_bytes().to_vec()));
    println!("Charlie's balance: {}", blockchain.get_balance(&charlie_keypair.public.to_bytes().to_vec()));
    
    // 4. Mine an empty block to establish the chain
    println!("\nMining empty block...");
    sleep(Duration::from_secs(2)).await; // To meet minimum block time requirement
    match blockchain.mine_empty_block() {
        Ok(block) => println!("Mined block: {} with hash: {}", block.index, hex::encode(&block.hash[0..8])),
        Err(e) => {
            println!("Failed to mine empty block: {}", e);
            return Ok(());
        }
    }
    
    // 5. Create auction transactions
    println!("\n=== Creating Auction Transactions ===");

    // Create auction
    let create_auction_tx = {
        // Get the auction command data
        let auction_cmd = blockchain::auction::auction_commands::AuctionCommand::CreateAuction {
            id: "auction1".to_string(),
            title: "Rare Book Collection".to_string(),
            description: "First edition of classic novels".to_string(),
        };
        let data = format!("AUCTION_{}", serde_json::to_string(&auction_cmd)?);
        
        // Create transaction with minimum fee
        blockchain::ledger::transaction::Transaction::create_data_tx(
            &alice_keypair,
            data,
            1, // nonce
            1000, // fee - should be accepted with MIN_FEE_RATE=0
        )?
    };
    print_tx(&create_auction_tx, "Create Auction");
    
    // Start auction
    let start_auction_tx = {
        let auction_cmd = blockchain::auction::auction_commands::AuctionCommand::StartAuction { 
            id: "auction1".to_string() 
        };
        let data = format!("AUCTION_{}", serde_json::to_string(&auction_cmd)?);
        
        blockchain::ledger::transaction::Transaction::create_data_tx(
            &alice_keypair,
            data,
            2, // nonce
            1000, // fee
        )?
    };
    print_tx(&start_auction_tx, "Start Auction");
    
    // Bob places bid
    let bob_bid_tx = {
        let auction_cmd = blockchain::auction::auction_commands::AuctionCommand::Bid { 
            id: "auction1".to_string(),
            amount: 1000,
        };
        let data = format!("AUCTION_{}", serde_json::to_string(&auction_cmd)?);
        
        blockchain::ledger::transaction::Transaction::create_data_tx(
            &bob_keypair,
            data,
            1, // nonce
            1000, // fee
        )?
    };
    print_tx(&bob_bid_tx, "Bob Bid");
    
    // Charlie places higher bid
    let charlie_bid_tx = {
        let auction_cmd = blockchain::auction::auction_commands::AuctionCommand::Bid { 
            id: "auction1".to_string(),
            amount: 1500,
        };
        let data = format!("AUCTION_{}", serde_json::to_string(&auction_cmd)?);
        
        blockchain::ledger::transaction::Transaction::create_data_tx(
            &charlie_keypair,
            data,
            1, // nonce
            1000, // fee
        )?
    };
    print_tx(&charlie_bid_tx, "Charlie Bid");
    
    // End auction
    let end_auction_tx = {
        let auction_cmd = blockchain::auction::auction_commands::AuctionCommand::EndAuction { 
            id: "auction1".to_string() 
        };
        let data = format!("AUCTION_{}", serde_json::to_string(&auction_cmd)?);
        
        blockchain::ledger::transaction::Transaction::create_data_tx(
            &alice_keypair,
            data,
            3, // nonce
            1000, // fee
        )?
    };
    print_tx(&end_auction_tx, "End Auction");
    
    // 5. Add transactions to the blockchain's transaction pool
    println!("\n=== Adding Transactions to Pool ===");
    
    // Adding the transactions
    let txs = vec![
        create_auction_tx.clone(),
        start_auction_tx.clone(),
        bob_bid_tx.clone(),
        charlie_bid_tx.clone(),
        end_auction_tx.clone()
    ];
    
    for tx in &txs {
        match blockchain.add_transaction(tx.clone()) {
            Ok(_) => println!("Successfully added tx {} to pool", hex::encode(&tx.tx_hash[0..8])),
            Err(e) => println!("Failed to add tx to pool: {}", e),
        }
    }
    
    // Verify transactions in pool
    let pool_size = blockchain.uncofirmed_transactions.size();
    println!("\nTransaction pool size: {}", pool_size);
    
    // 6. Mine a block with the auction transactions
    println!("\n=== Mining Block with Auction Transactions ===");
    sleep(Duration::from_secs(2)).await; // To meet minimum block time requirement
    
    match blockchain.mine_block(10) { // Mine block with up to 10 transactions
        Ok(block) => {
            println!("Successfully mined block {} with hash {}", 
                     block.index, hex::encode(&block.hash[0..8]));
            println!("Block contains {} transactions", block.transactions.len());
            
            // Verify transactions in block
            println!("\n=== Verifying Transactions in Block ===");
            for (i, tx) in block.transactions.iter().enumerate() {
                let tx_hash = hex::encode(&tx.tx_hash[0..8]);
                println!("Transaction {}: {}", i+1, tx_hash);
                
                if let Some(ref data) = tx.data.data {
                    if data.starts_with("AUCTION_") {
                        println!("  - Auction Command: {}", 
                                 if data.len() > 50 { &data[0..50] } else { data });
                    }
                }
            }
            
            // Verify all transactions were included
            let all_included = txs.iter().all(|tx| 
                block.transactions.iter().any(|btx| btx.tx_hash == tx.tx_hash)
            );
            
            println!("\nAll auction transactions included in block: {}", all_included);
            
            // Check transaction pool is now empty
            let pool_size_after = blockchain.uncofirmed_transactions.size();
            println!("Transaction pool size after mining: {}", pool_size_after);
        },
        Err(e) => println!("Failed to mine block: {}", e),
    }
    
    // 7. Verify blockchain state after mining
    println!("\n=== Final Blockchain State ===");
    println!("Chain length: {} blocks", blockchain.blocks.len());
    if let Some(last_block) = blockchain.get_last_block() {
        println!("Latest block hash: {}", hex::encode(&last_block.hash[0..8]));
        println!("Latest block contains {} transactions", last_block.transactions.len());
    }
    
    println!("\n=== Auction Transaction Test Completed ===");
    
    Ok(())
}