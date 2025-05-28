use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use blockchain::auction::auction::{collect_auctions, find_auction_transactions, Auction, AuctionStatus};
use blockchain::auction::auction_commands::{generate_auction_id, tx_bid, tx_create_auction, tx_end_auction, tx_start_auction};
use tonic::transport::Server;
use blockchain::kademlia::kademlia_proto::kademlia_server::KademliaServer;
use blockchain::kademlia::node::Node;
use blockchain::kademlia::service::KademliaService;
use ed25519_dalek::Keypair;
use tokio::io::{self as tokio_io, AsyncBufReadExt};
use tokio::sync::Notify;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        println!("Usage: cargo run --bin main <SELF PORT> <BOOTSTRAP PORT> <POW DIFFICULTY>");
        return Ok(());
    }

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let port: u16 = args[1].parse()?;
    let bootstrap_port: u16 = args[2].parse()?;
    let difficulty: usize = args[3].parse()?;

    let address = SocketAddr::new(ip, port);
    let bootstrap_address = SocketAddr::new(ip, bootstrap_port);

    let node = Node::new(address);
    let shutdown = Arc::new(Notify::new());
    let shutdown_trigger = shutdown.clone();
    let service = KademliaService::new_with_shutdown(node.clone(), shutdown);

    let keypair = node.clone().get_keypair()?;
    let nonce = Arc::new(std::sync::Mutex::new(0u64));

    let server = Server::builder()
        .add_service(KademliaServer::new(service))
        .serve_with_shutdown(address, async move {
            shutdown_trigger.notified().await;
        });

    tokio::select! {
        result = server => result?,
        result = menu(node.clone(), ip, address, bootstrap_address,difficulty, keypair, nonce) => result?,
    }

    println!("Shutting down...");
    Ok(())
}

async fn menu(
    node: Node,
    ip: IpAddr,
    address: SocketAddr,
    bootstrap_address: SocketAddr,
    difficulty: usize,
    keypair: Keypair,
    nonce: Arc<std::sync::Mutex<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let bootstrap_node = Node::new(bootstrap_address);
    node.join(bootstrap_node.clone(), difficulty).await?;

    let stdin = tokio_io::BufReader::new(tokio_io::stdin());
    let mut lines = stdin.lines();

    loop {
        println!("\n=== MENU {} ===", address);
        println!("0. EXIT");
        println!("1. PING");
        println!("2. STORE");
        println!("3. FIND NODE");
        println!("4. FIND VALUE");
        println!("5. WHO AM I?");
        println!("6. CREATE AUCTION");
        println!("7. LIST AUCTIONS");
        println!("8. LIST MY AUCTIONS");
        println!("9. Mine Block");
        println!("10. Show BLOCKCHAIN INFO");
        println!("99. DEBUG TEST");
        print!("\nOption: ");
        io::stdout().flush().unwrap();

        let input = match lines.next_line().await? {
            Some(line) => line.trim().to_string(),
            None => continue,
        };

        match input.as_str() {
            "0" => return Ok(()),
            "1" => handle_ping(&node, ip).await?,
            "2" => handle_store(&node).await?,
            "3" => handle_find_node(&node, ip).await?,
            "4" => handle_find_value(&node, ip).await?,
            "5" => handle_whoami(&node, &keypair),
            "6" => handle_create_auction(&node, &keypair, nonce.clone()).await?,
            "7" => handle_list_auctions(&node, &keypair, nonce.clone()).await?,
            "8" => handle_my_auctions(&node, &keypair, nonce.clone()).await?,
            "9" => handle_mine_block(&node).await?,
            "10" => handle_blockchain_info(&node),
            "99" => handle_debug_test(&node, nonce.clone()).await?,
            _ => println!("Invalid option."),
        }
    }
}

async fn handle_debug_test(
    node: &Node,
    nonce: Arc<std::sync::Mutex<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 === DEBUG TEST START ===");
    
    // Test 1: Check transaction pool before anything
    println!("\n🔍 TEST 1: Initial pool state");
    let pool = node.get_transaction_pool();
    let pool_guard = pool.lock().unwrap();
    println!("📊 Pool size: {}", pool_guard.size());
    drop(pool_guard);
    
    // Test 2: Create a simple transaction
    println!("\n🔍 TEST 2: Creating test transaction");
    let mut nonce_lock = nonce.lock().unwrap();
    let current_nonce = *nonce_lock;
    
    let test_data = format!("DEBUG_TEST_{}", current_nonce);
    println!("📝 Creating transaction with data: {}", test_data);
    
    match node.create_transaction(
        None,
        blockchain::ledger::transaction::TransactionType::Data,
        None,
        Some(test_data.clone()),
    ).await {
        Ok(tx) => {
            println!("✅ Transaction created:");
            println!("   Hash: {}", hex::encode(&tx.tx_hash[..8]));
            println!("   Valid: {}", tx.verify());
            println!("   Sender: {:02x?}", &tx.data.sender[..4]);
            println!("   Data: {:?}", tx.data.data);
            println!("   Fee: {}", tx.data.fee);
            println!("   Nonce: {}", tx.data.nonce);
            
            // Test 3: Submit transaction
            println!("\n🔍 TEST 3: Submitting transaction");
            match node.submit_transaction(tx).await {
                Ok(_) => {
                    println!("✅ Transaction submitted successfully");
                    *nonce_lock += 1;
                }
                Err(e) => {
                    println!("❌ Failed to submit transaction: {}", e);
                    return Ok(());
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to create transaction: {}", e);
            return Ok(());
        }
    }
    drop(nonce_lock);
    
    // Test 4: Check pool after submission
    println!("\n🔍 TEST 4: Pool state after submission");
    let pool = node.get_transaction_pool();
    let pool_guard = pool.lock().unwrap();
    let pool_size = pool_guard.size();
    println!("📊 Pool size: {}", pool_size);
    
    if pool_size > 0 {
        println!("📦 Transactions in pool:");
        let all_txs = pool_guard.get_all_transactions();
        for (i, tx) in all_txs.iter().enumerate() {
            println!("   {}. Hash: {}, Valid: {}", 
                     i+1, 
                     hex::encode(&tx.tx_hash[..8]), 
                     tx.verify());
            if let Some(data) = &tx.data.data {
                println!("      Data: {}", data);
            }
            println!("      Fee: {}, Nonce: {}", tx.data.fee, tx.data.nonce);
        }
    } else {
        println!("❌ No transactions in pool!");
    }
    drop(pool_guard);
    
    // Test 5: Try to mine
    println!("\n🔍 TEST 5: Attempting to mine block");
    match node.mine_block().await {
        Ok(block) => {
            println!("✅ Block mined successfully!");
            println!("   Index: {}", block.index);
            println!("   Hash: {}", hex::encode(&block.hash[..8]));
            println!("   Transactions: {}", block.transactions.len());
            
            if block.transactions.len() > 0 {
                println!("   🎉 SUCCESS: Transaction was mined into block!");
                for (i, tx) in block.transactions.iter().enumerate() {
                    println!("      {}. TX: {}", i+1, hex::encode(&tx.tx_hash[..8]));
                    if let Some(data) = &tx.data.data {
                        println!("         Data: {}", data);
                    }
                }
            } else {
                println!("   ❌ PROBLEM: Block is empty even though we had transactions!");
            }
        }
        Err(e) => {
            println!("❌ Mining failed: {}", e);
        }
    }
    
    // Test 6: Final pool check
    println!("\n🔍 TEST 6: Final pool state");
    let pool = node.get_transaction_pool();
    let pool_guard = pool.lock().unwrap();
    println!("📊 Final pool size: {}", pool_guard.size());
    
    println!("\n🔍 === DEBUG TEST COMPLETE ===");
    Ok(())
}


async fn handle_mine_block(node: &Node) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n [NODE {}] STARTING MANUAL MINING...", node.get_address().port());
    
    // Check transaction pool first
    let pool_size = {
        let pool = node.get_transaction_pool();
        let pool_guard = pool.lock().unwrap();
        pool_guard.size()
    };
    
    if pool_size == 0 {
        println!("[NODE {}] No transactions in pool - mining empty block", node.get_address().port());
    } else {
        println!("[NODE {}] Mining block with {} pending transactions", node.get_address().port(), pool_size);
        
        // Show what transactions we're about to mine
        let pool = node.get_transaction_pool();
        let pool_guard = pool.lock().unwrap();
        let transactions = pool_guard.get_all_transactions();
        
        for (i, tx) in transactions.iter().enumerate() {
            if let Some(data) = &tx.data.data {
                if data.starts_with("AUCTION_") {
                    let cmd_part = &data[8..std::cmp::min(data.len(), 50)];
                    println!("   {}. Auction: {}", i+1, cmd_part);
                }
            } else if tx.data.amount.is_some() {
                println!("   {}. Transfer: {} tokens", i+1, tx.data.amount.unwrap());
            }
        }
    }
    
    let start_time = std::time::Instant::now();
    
    match node.mine_block().await {
        Ok(block) => {
            let mining_time = start_time.elapsed();
            
            println!("[NODE {}] BLOCK MINED SUCCESSFULLY!", node.get_address().port());
            println!("Block Index: {}", block.index);
            println!("Block Hash: {}", hex::encode(&block.hash[..8]));
            println!("Nonce: {}", block.nonce);
            println!("Mining Time: {:.2}s", mining_time.as_secs_f64());
            println!("Transactions Mined: {}", block.transactions.len());
            
            // Show which auction operations were mined
            if block.transactions.len() > 0 {
                println!("Auction operations in this block:");
                for (i, tx) in block.transactions.iter().enumerate() {
                    if let Some(data) = &tx.data.data {
                        if data.starts_with("AUCTION_") {
                            if data.contains("CreateAuction") {
                                println!("      {}. Auction Created", i+1);
                            } else if data.contains("StartAuction") {
                                println!("      {}. Auction Started", i+1);
                            } else if data.contains("Bid") {
                                println!("      {}. Bid Placed", i+1);
                            } else if data.contains("EndAuction") {
                                println!("      {}. Auction Ended", i+1);
                            }
                        }
                    }
                }
            }
            
            println!("[NODE {}] Block successfully added to blockchain!", node.get_address().port());
        }
        Err(e) => {
            println!("[NODE {}] Mining failed: {}", node.get_address().port(), e);
        }
    }
    Ok(())
}

fn handle_blockchain_info(node: &Node) {
    println!("\n [NODE {}] BLOCKCHAIN STATUS", node.get_address().port());
    
    let (height, last_hash) = node.get_blockchain_info();
    println!("Chain Height: {} blocks", height);
    
    if let Some(hash) = last_hash {
        println!("Last Block Hash: {}", &hash[..16]);
    }
    
    // Show transaction pool status
    let pool = node.get_transaction_pool();
    let pool_guard = pool.lock().unwrap();
    let pool_size = pool_guard.size();
    println!("Transaction Pool: {} pending transactions", pool_size);
    
    if pool_size > 0 {
        println!("Tip: Use option 9 to mine these transactions into a block!");
    }
    
    // Show recent blocks
    let blockchain = node.get_blockchain();
    let blockchain_guard = blockchain.read().unwrap();
    let recent_blocks = if blockchain_guard.blocks.len() >= 3 {
        &blockchain_guard.blocks[blockchain_guard.blocks.len()-3..]
    } else {
        &blockchain_guard.blocks[..]
    };
    
    println!("Recent Blocks:");
    for block in recent_blocks {
        println!("Block {}: {} transactions, hash: {}", 
                block.index, 
                block.transactions.len(), 
                hex::encode(&block.hash[..8]));
    }
}

async fn handle_ping(node: &Node, ip: IpAddr) -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = prompt_parse("Target Port: ").await;
    let target = Node::new(SocketAddr::new(ip, port));
    let ok = node.ping(&target).await?;
    println!("Alive: {}", ok);
    Ok(())
}

async fn handle_store(node: &Node) -> Result<(), Box<dyn std::error::Error>> {
    let key = prompt_hex("Key (40 hex chars): ").await;
    let value = prompt("Value: ").await.into_bytes();
    match key.try_into() {
        Ok(key_array) => {
            node.store(key_array, value).await?;
            println!("Stored");
        }
        Err(_) => println!("Key must be exactly 40 hex characters (20 bytes)."),
    }
    Ok(())
}

async fn handle_find_node(node: &Node, ip: IpAddr) -> Result<(), Box<dyn std::error::Error>> {
    let id = prompt_hex("Target ID (40 hex chars): ").await;
    let port: u16 = prompt_parse("Target Port: ").await;
    let target = Node::new(SocketAddr::new(ip, port));
    match id.try_into() {
        Ok(id_array) => {
            let nodes = node.find_node(target, id_array).await?;
            for n in nodes {
                println!("Node ID: {:02x?} @ {}", n.get_id(), n.get_address());
            }
        }
        Err(_) => println!("ID must be exactly 40 hex characters (20 bytes)."),
    }
    Ok(())
}

async fn handle_find_value(node: &Node, ip: IpAddr) -> Result<(), Box<dyn std::error::Error>> {
    let key = prompt_hex("Key (40 hex chars): ").await;
    let port: u16 = prompt_parse("Target Port: ").await;
    let target = Node::new(SocketAddr::new(ip, port));
    match key.try_into() {
        Ok(key_array) => {
            let (value, nodes) = node.find_value(target, key_array).await?;
            match value {
                Some(v) => println!("Value: {:?}", String::from_utf8_lossy(&v)),
                None => {
                    println!("Value not found. Closest nodes:");
                    for n in nodes {
                        println!("Node ID: {:02x?} @ {}", n.get_id(), n.get_address());
                    }
                }
            }
        }
        Err(_) => println!("Key must be exactly 40 hex characters (20 bytes)."),
    }
    Ok(())
}

fn handle_whoami(node: &Node, keypair: &Keypair) {
    println!("ID: {:02x?}", node.get_id());
    println!("IP: {}", node.get_address());
    println!("Public Key: {:02x?}", keypair.public.to_bytes());
}

async fn handle_create_auction(
    node: &Node,
    keypair: &Keypair,
    nonce: Arc<std::sync::Mutex<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let title = prompt("Auction Title: ").await;
    let description = prompt("Auction Description: ").await;

    // Calculate the correct nonce by considering both blockchain state AND pending transactions
    let correct_nonce = {
        let blockchain = node.get_blockchain();
        let blockchain_guard = blockchain.read().unwrap();
        let blockchain_nonce = blockchain_guard.get_next_nonce(&keypair.public.to_bytes().to_vec());
        drop(blockchain_guard);
        
        // Check how many pending transactions we have from this sender
        let pool = node.get_transaction_pool();
        let pool_guard = pool.lock().unwrap();
        let sender_key = keypair.public.to_bytes().to_vec();
        let pending_txs = pool_guard.get_pending_by_sender(&sender_key);
        let pending_count = pending_txs.len() as u64;
        drop(pool_guard);
        
        // The correct nonce is blockchain nonce + number of pending transactions
        let calculated_nonce = blockchain_nonce + pending_count;
        
        println!("DEBUG: Blockchain nonce: {}, Pending txs: {}, Using nonce: {}", 
                 blockchain_nonce, pending_count, calculated_nonce);
        
        calculated_nonce
    };
    
    match tx_create_auction(keypair, title.clone(), description.clone(), correct_nonce) {
        Ok(transaction) => {
            println!("DEBUG: Transaction created successfully");
            println!("  Hash: {}", hex::encode(&transaction.tx_hash[..8]));
            println!("  Valid: {}", transaction.verify());
            println!("  Nonce: {}", transaction.data.nonce);
            println!("  Fee: {}", transaction.data.fee);
            
            let auction_id = generate_auction_id(&keypair.public.to_bytes(), &title, &description, correct_nonce);
            
            // Use the node's submit_transaction method
            match node.submit_transaction(transaction).await {
                Ok(_) => {
                    println!("Auction created successfully!");
                    println!("Auction ID: {}", auction_id);
                    println!("Title: {}", title);
                    println!("Description: {}", description);
                    
                    // Update our local nonce counter to the next expected value
                    let mut nonce_lock = nonce.lock().unwrap();
                    *nonce_lock = correct_nonce + 1;
                    
                    // Verify it's in the pool
                    let pool = node.get_transaction_pool();
                    let pool_guard = pool.lock().unwrap();
                    println!("DEBUG: Pool now has {} transactions", pool_guard.size());
                }
                Err(e) => println!("Failed to submit auction transaction: {}", e),
            }
        }
        Err(e) => println!("Failed to create auction transaction: {}", e),
    }
    Ok(())
}

async fn handle_list_auctions(
    node: &Node,
    keypair: &Keypair,
    nonce: Arc<std::sync::Mutex<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let blockchain = node.get_blockchain();
    let blockchain_data = {
        let guard = blockchain.read().unwrap();
        (*guard).clone()
    };

    let auction_txs = find_auction_transactions(&blockchain_data);
    let auctions = collect_auctions(&auction_txs.into_iter().cloned().collect::<Vec<_>>());
    
    if auctions.is_empty() {
        println!("No auctions found.");
        println!("Create some auctions first using option 6.");
        return Ok(());
    }

    println!("Found {} auction(s):\n", auctions.len());
    
    for (id, auction) in &auctions {
        let status_emoji = match auction.status {
            AuctionStatus::Pending => "⏳",
            AuctionStatus::Active => "🟢",
            AuctionStatus::Ended => "🔴",
        };
        
        println!("{} Auction ID: {}", status_emoji, id);
        println!("   Title: {}", auction.title);
        println!("   Status: {:?}", auction.status);
        println!("   Owner: {:02x?}", &auction.owner[..8]);
        
        if let Some((amount, bidder)) = &auction.highest_bid {
            println!("   Highest Bid: {} by {:02x?}", amount, &bidder[..8]);
        } else {
            println!("   Highest Bid: None");
        }
        println!();
    }

    auction_submenu(&node, &auctions, keypair, nonce).await?;
    Ok(())
}

async fn auction_submenu(
    node: &Node,
    auctions: &HashMap<String, Auction>,
    keypair: &Keypair,
    nonce: Arc<std::sync::Mutex<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = tokio_io::BufReader::new(tokio_io::stdin());
    let mut lines = stdin.lines();

    loop {
        println!("=== AUCTION ACTIONS ===");
        println!("0. Back to main menu");
        println!("B. Place a bid");
        print!("\nOption: ");
        io::stdout().flush().unwrap();

        let input = match lines.next_line().await? {
            Some(line) => line.trim().to_uppercase(),
            None => continue,
        };

        match input.as_str() {
            "0" => break,
            "B" => handle_bid(&node, auctions, keypair, nonce.clone()).await?,
            _ => println!("Invalid option."),
        }
    }
    Ok(())
}

async fn handle_bid(
    node: &Node,
    auctions: &HashMap<String, Auction>,
    keypair: &Keypair,
    nonce: Arc<std::sync::Mutex<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if auctions.is_empty() {
        println!("No auctions available for bidding.");
        return Ok(());
    }

    let auction_id = prompt("Enter auction ID to bid on: ").await;
    
    match auctions.get(&auction_id) {
        Some(auction) => {
            match auction.status {
                AuctionStatus::Ended => {
                    println!("This auction has ended. Cannot place bid.");
                    return Ok(());
                }
                AuctionStatus::Pending => {
                    println!("This auction is still pending. Cannot place bid yet.");
                    return Ok(());
                }
                AuctionStatus::Active => {}
            }
        
            let my_public_key = keypair.public.to_bytes();
            if auction.owner == my_public_key {
                println!("You cannot bid on your own auction.");
                return Ok(());
            }
        
            println!("\n📋 Bidding on: {}", auction.title);
            println!("🆔 Auction ID: {}", auction_id);
            if let Some((current_bid, _)) = &auction.highest_bid {
                println!("💰 Current highest bid: {}", current_bid);
                println!("💡 Your bid must be higher than {}", current_bid);
            } else {
                println!("💰 No bids yet - you can place the first bid!");
            }
        
            let bid_amount: u64 = prompt_parse("Enter your bid amount: ").await;
        
            if let Some((current_highest, _)) = &auction.highest_bid {
                if bid_amount <= *current_highest {
                    println!("Bid must be higher than current highest bid of {}", current_highest);
                    return Ok(());
                }
            }

            let correct_nonce = calculate_next_nonce(node, keypair);
            
            match tx_bid(keypair, auction_id.clone(), bid_amount, correct_nonce) {
                Ok(transaction) => {
                    match node.submit_transaction(transaction).await {
                        Ok(_) => {
                            println!("Bid placed successfully!");
                            println!("Auction ID: {}", auction_id);
                            println!("Amount: {}", bid_amount);
                            
                            let mut nonce_lock = nonce.lock().unwrap();
                            *nonce_lock = correct_nonce + 1;
                        }
                        Err(e) => println!("Failed to submit bid transaction: {}", e),
                    }
                }
                Err(e) => println!("Failed to create bid transaction: {}", e),
            }            
        }
        None => println!("Invalid auction ID"),
    }
    Ok(())
}


async fn handle_my_auctions(
    node: &Node,
    keypair: &Keypair,
    nonce: Arc<std::sync::Mutex<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(" Finding your auctions...");
    
    let blockchain = node.get_blockchain();
    let blockchain_data = {
        let guard = blockchain.read().unwrap();
        (*guard).clone()
    };

    let auction_txs = find_auction_transactions(&blockchain_data);
    let auctions = collect_auctions(&auction_txs.into_iter().cloned().collect::<Vec<_>>());
    let my_public_key = keypair.public.to_bytes();

    let my_auctions: HashMap<String, Auction> = auctions
        .into_iter()
        .filter(|(_, auction)| auction.owner == my_public_key)
        .collect();
    
    if my_auctions.is_empty() {
        println!("You haven't created any auctions yet.");
        println!("Create an auction using option 6.");
        return Ok(());
    }

    println!("You have {} auction(s):\n", my_auctions.len());
    
    for (id, auction) in &my_auctions {
        let status_emoji = match auction.status {
            AuctionStatus::Pending => "⏳",
            AuctionStatus::Active => "🟢",
            AuctionStatus::Ended => "🔴",
        };
        
        println!("{} Your Auction ID: {}", status_emoji, id);
        println!("   Title: {}", auction.title);
        println!("   Status: {:?}", auction.status);
        
        if let Some((amount, bidder)) = &auction.highest_bid {
            println!("   Highest Bid: {} by {:02x?}", amount, &bidder[..8]);
        } else {
            println!("   Highest Bid: None");
        }
        println!();
    }

    my_auctions_submenu(&node,&my_auctions, keypair, nonce).await?;
    Ok(())
}

async fn my_auctions_submenu(
    node: &Node,
    my_auctions: &HashMap<String, Auction>,
    keypair: &Keypair,
    nonce: Arc<std::sync::Mutex<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = tokio_io::BufReader::new(tokio_io::stdin());
    let mut lines = stdin.lines();

    loop {
        println!("=== AUCTION MANAGEMENT ===");
        println!("0. Back to main menu");
        println!("S. Start an auction");
        println!("E. End an auction");
        print!("\nOption: ");
        io::stdout().flush().unwrap();

        let input = match lines.next_line().await? {
            Some(line) => line.trim().to_uppercase(),
            None => continue,
        };

        match input.as_str() {
            "0" => break,
            "S" => handle_start_auction(&node, my_auctions, keypair, nonce.clone()).await?,
            "E" => handle_end_auction(&node,my_auctions, keypair, nonce.clone()).await?,
            _ => println!("Invalid option."),
        }
    }
    Ok(())
}

async fn handle_start_auction(
    node: &Node,
    my_auctions: &HashMap<String, Auction>,
    keypair: &Keypair,
    nonce: Arc<std::sync::Mutex<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let startable_auctions: HashMap<String, Auction> = my_auctions
        .iter()
        .filter(|(_, auction)| matches!(auction.status, AuctionStatus::Pending))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    if startable_auctions.is_empty() {
        println!("No auctions available to start.");
        println!("Only pending auctions can be started.");
        return Ok(());
    }

    println!("\n📋 Auctions you can start:");
    for (id, auction) in &startable_auctions {
        println!("⏳ ID: {} - Title: {}", id, auction.title);
    }

    let auction_id = prompt("Enter auction ID to start: ").await;

    match startable_auctions.get(&auction_id) {
        Some(auction) => {
            println!("\nStarting auction: {}", auction.title);
            println!("Auction ID: {}", auction_id);

            let correct_nonce = calculate_next_nonce(node, keypair);
            
            match tx_start_auction(keypair, auction_id.clone(), correct_nonce) {
                Ok(transaction) => {
                    match node.submit_transaction(transaction).await {
                        Ok(_) => {
                            println!("Auction started successfully!");
                            println!("Auction ID: {}", auction_id);
                            
                            let mut nonce_lock = nonce.lock().unwrap();
                            *nonce_lock = correct_nonce + 1;
                        }
                        Err(e) => println!("Failed to submit start auction transaction: {}", e),
                    }
                }
                Err(e) => println!("Failed to create start auction transaction: {}", e),
            }
        }
        None => {
            println!("Auction ID '{}' not found or cannot be started.", auction_id);
        }
    }
    Ok(())
}

async fn handle_end_auction(
    node: &Node,
    my_auctions: &HashMap<String, Auction>,
    keypair: &Keypair,
    nonce: Arc<std::sync::Mutex<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let endable_auctions: HashMap<String, Auction> = my_auctions
        .iter()
        .filter(|(_, auction)| matches!(auction.status, AuctionStatus::Active))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    if endable_auctions.is_empty() {
        println!("No auctions available to end.");
        println!("Only active auctions can be ended.");
        return Ok(());
    }

    println!("\nAuctions you can end:");
    for (id, auction) in &endable_auctions {
        let bid_info = if let Some((amount, _)) = &auction.highest_bid {
            format!(" - Highest Bid: {}", amount)
        } else {
            " - No bids".to_string()
        };
        println!("ID: {} - Title: {}{}", id, auction.title, bid_info);
    }

    let auction_id = prompt("Enter auction ID to end: ").await;

    match endable_auctions.get(&auction_id) {
        Some(auction) => {
            println!("\nEnding auction: {}", auction.title);
            println!("Auction ID: {}", auction_id);

            if let Some((amount, bidder)) = &auction.highest_bid {
                println!("Winner: {:02x?}", &bidder[..8]);
                println!("Winning bid: {}", amount);
            } else {
                println!("No bids were placed on this auction.");
            }

            let confirm = prompt("Are you sure you want to end this auction? (y/N): ").await;
            if confirm.to_lowercase() == "y" || confirm.to_lowercase() == "yes" {
                let correct_nonce = calculate_next_nonce(node, keypair);
                
                match tx_end_auction(keypair, auction_id.clone(), correct_nonce) {
                    Ok(transaction) => {
                        match node.submit_transaction(transaction).await {
                            Ok(_) => {
                                println!("Auction ended successfully!");
                                println!("Auction ID: {}", auction_id);
                                
                                let mut nonce_lock = nonce.lock().unwrap();
                                *nonce_lock = correct_nonce + 1;
                            }
                            Err(e) => println!("Failed to submit end auction transaction: {}", e),
                        }
                    }
                    Err(e) => println!("Failed to create end auction transaction: {}", e),
                }
            }
        }
        None => {
            println!("Auction ID '{}' not found or cannot be ended.", auction_id);
        }
    }
    Ok(())
}

async fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut stdin = tokio_io::BufReader::new(tokio_io::stdin());
    let mut input = String::new();
    stdin.read_line(&mut input).await.unwrap();
    input.trim().to_string()
}

async fn prompt_hex(msg: &str) -> Vec<u8> {
    loop {
        let input = prompt(msg).await;
        match hex::decode(&input) {
            Ok(bytes) => return bytes,
            Err(_) => println!("Invalid hex input. Please try again."),
        }
    }
}

async fn prompt_parse<T: FromStr>(msg: &str) -> T {
    loop {
        let input = prompt(msg).await;
        match input.parse::<T>() {
            Ok(value) => return value,
            Err(_) => println!("Invalid input. Please try again."),
        }
    }
}

fn calculate_next_nonce(node: &Node, keypair: &Keypair) -> u64 {
    let blockchain = node.get_blockchain();
    let blockchain_guard = blockchain.read().unwrap();
    let blockchain_nonce = blockchain_guard.get_next_nonce(&keypair.public.to_bytes().to_vec());
    drop(blockchain_guard);
    
    // Check how many pending transactions we have from this sender
    let pool = node.get_transaction_pool();
    let pool_guard = pool.lock().unwrap();
    let sender_key = keypair.public.to_bytes().to_vec();
    let pending_txs = pool_guard.get_pending_by_sender(&sender_key);
    let pending_count = pending_txs.len() as u64;
    drop(pool_guard);
    
    // The correct nonce is blockchain nonce + number of pending transactions
    let calculated_nonce = blockchain_nonce + pending_count;
    
    println!("DEBUG: Blockchain nonce: {}, Pending txs: {}, Using nonce: {}", 
             blockchain_nonce, pending_count, calculated_nonce);
    
    calculated_nonce
}