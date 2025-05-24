// Example usage file: src/bin/full_sync_example.rs
use blockchain::kademlia::node::Node;
use std::time::Duration;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Full Blockchain Sync Example ===");

    // Create bootstrap node
    let bootstrap_node = Node::new("127.0.0.1:50051".parse()?);
    
    // Create other nodes
    let node1 = Node::new("127.0.0.1:50052".parse()?);
    let node2 = Node::new("127.0.0.1:50053".parse()?);
    let node3 = Node::new("127.0.0.1:50054".parse()?);

    // Start all nodes
    let bootstrap_handle = tokio::spawn({
        let node = bootstrap_node.clone();
        async move {
            println!("Starting bootstrap node on {}", node.get_address());
            node.start().await.expect("Bootstrap node failed");
        }
    });

    let node1_handle = tokio::spawn({
        let node = node1.clone();
        async move {
            println!("Starting node 1 on {}", node.get_address());
            node.start().await.expect("Node 1 failed");
        }
    });

    let node2_handle = tokio::spawn({
        let node = node2.clone();
        async move {
            println!("Starting node 2 on {}", node.get_address());
            node.start().await.expect("Node 2 failed");
        }
    });

    let node3_handle = tokio::spawn({
        let node = node3.clone();
        async move {
            println!("Starting node 3 on {}", node.get_address());
            node.start().await.expect("Node 3 failed");
        }
    });

    // Wait for nodes to start
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Join nodes to network (they will automatically sync blockchain)
    println!("\n=== Joining Nodes to Network ===");
    
    println!("Node 1 joining...");
    node1.join(bootstrap_node.clone(), 2).await?;
    
    println!("Node 2 joining...");
    node2.join(bootstrap_node.clone(), 2).await?;
    
    println!("Node 3 joining...");
    node3.join(bootstrap_node.clone(), 2).await?;

    // Start mining on bootstrap node
    println!("\n=== Starting Mining ===");
    bootstrap_node.start_mining().await;
    
    // Wait a bit for some blocks to be mined
    println!("Waiting for blocks to be mined...");
    for i in 1..=6 {
        tokio::time::sleep(Duration::from_secs(15)).await;
        let (height, _) = bootstrap_node.get_blockchain_info();
        println!("Mining progress: {} blocks after {} seconds", height, i * 15);
    }

    // Create and submit some transactions
    println!("\n=== Creating Transactions ===");
    
    // Create a transfer transaction
    let transfer_tx = node1.create_transaction(
        Some(node2.get_public_key().to_vec()),
        blockchain::ledger::transaction::TransactionType::Transfer,
        Some(1000),
        None
    ).await?;
    
    node1.submit_transaction(transfer_tx).await?;
    println!("Submitted transfer transaction");

    // Create an auction transaction
    let auction_data = serde_json::to_string(&blockchain::auction::auction_commands::AuctionCommand::CreateAuction {
        id: "auction1".to_string(),
        title: "Test Auction".to_string(),
        description: "A test auction item".to_string(),
    })?;
    
    let auction_tx = node2.create_transaction(
        None,
        blockchain::ledger::transaction::TransactionType::Data,
        None,
        Some(format!("AUCTION_{}", auction_data))
    ).await?;
    
    node2.submit_transaction(auction_tx).await?;
    println!("Submitted auction transaction");

    // Wait for transactions to be mined and blocks to be synced
    println!("Waiting for transactions to be mined and blocks to sync...");
    tokio::time::sleep(Duration::from_secs(45)).await;
    
    // Force sync on all nodes
    println!("Forcing blockchain sync on all nodes...");
    node1.sync_blockchain().await;
    node2.sync_blockchain().await;
    node3.sync_blockchain().await;
    
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Check blockchain status on all nodes
    println!("\n=== Blockchain Status ===");
    
    let (bootstrap_height, bootstrap_hash) = bootstrap_node.get_blockchain_info();
    println!("Bootstrap node - Height: {}, Last Hash: {:?}", bootstrap_height, bootstrap_hash);
    
    let (node1_height, node1_hash) = node1.get_blockchain_info();
    println!("Node 1 - Height: {}, Last Hash: {:?}", node1_height, node1_hash);
    
    let (node2_height, node2_hash) = node2.get_blockchain_info();
    println!("Node 2 - Height: {}, Last Hash: {:?}", node2_height, node2_hash);
    
    let (node3_height, node3_hash) = node3.get_blockchain_info();
    println!("Node 3 - Height: {}, Last Hash: {:?}", node3_height, node3_hash);

    // Verify all nodes have the same blockchain
    let all_same_height = bootstrap_height == node1_height && 
                         node1_height == node2_height && 
                         node2_height == node3_height;
    
    let all_same_hash = bootstrap_hash == node1_hash && 
                       node1_hash == node2_hash && 
                       node2_hash == node3_hash;

    println!("\n=== Synchronization Results ===");
    println!("All nodes have same height: {}", all_same_height);
    println!("All nodes have same last hash: {}", all_same_hash);
    
    if all_same_height && all_same_hash {
        println!("✅ Full blockchain synchronization successful!");
    } else {
        println!("❌ Blockchain synchronization failed!");
    }

    // Test joining a new node after blockchain has grown
    println!("\n=== Testing Late Joiner ===");
    let late_node = Node::new("127.0.0.1:50055".parse()?);
    
    let late_handle = tokio::spawn({
        let node = late_node.clone();
        async move {
            println!("Starting late node on {}", node.get_address());
            node.start().await.expect("Late node failed");
        }
    });

    tokio::time::sleep(Duration::from_secs(2)).await;
    
    println!("Late node joining network...");
    late_node.join(bootstrap_node.clone(), 2).await?;
    
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    let (late_height, late_hash) = late_node.get_blockchain_info();
    println!("Late node - Height: {}, Last Hash: {:?}", late_height, late_hash);
    
    let late_sync_success = late_height == bootstrap_height && late_hash == bootstrap_hash;
    println!("Late node sync successful: {}", late_sync_success);

    // Keep running until Ctrl+C
    println!("\n=== System Running ===");
    println!("Press Ctrl+C to stop the system");
    
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nReceived shutdown signal");
        },
        _ = bootstrap_handle => {},
        _ = node1_handle => {},
        _ = node2_handle => {},
        _ = node3_handle => {},
        _ = late_handle => {},
    }

    println!("System shutdown complete");
    Ok(())
}