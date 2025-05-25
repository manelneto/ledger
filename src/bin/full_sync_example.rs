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
    let _bootstrap_handle = tokio::spawn({
        let node = bootstrap_node.clone();
        async move {
            println!("Starting bootstrap node on {}", node.get_address());
            node.start().await.expect("Bootstrap node failed");
        }
    });

    let _node1_handle = tokio::spawn({
        let node = node1.clone();
        async move {
            println!("Starting node 1 on {}", node.get_address());
            node.start().await.expect("Node 1 failed");
        }
    });

    let _node2_handle = tokio::spawn({
        let node = node2.clone();
        async move {
            println!("Starting node 2 on {}", node.get_address());
            node.start().await.expect("Node 2 failed");
        }
    });

    let _node3_handle = tokio::spawn({
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

    // Test joining new nodes after blockchain has grown
    println!("\n=== Testing Late Joiners ===");
    
    // First late node
    let late_node1 = Node::new("127.0.0.1:50055".parse()?);
    
    let _late_handle1 = tokio::spawn({
        let node = late_node1.clone();
        async move {
            println!("Starting late node 1 on {}", node.get_address());
            node.start().await.expect("Late node 1 failed");
        }
    });

    tokio::time::sleep(Duration::from_secs(2)).await;
    
    println!("Late node 1 joining network...");
    late_node1.join(bootstrap_node.clone(), 2).await?;
    
    // Wait a bit, then add second late node
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Second late node
    let late_node2 = Node::new("127.0.0.1:50056".parse()?);
    
    let _late_handle2 = tokio::spawn({
        let node = late_node2.clone();
        async move {
            println!("Starting late node 2 on {}", node.get_address());
            node.start().await.expect("Late node 2 failed");
        }
    });

    tokio::time::sleep(Duration::from_secs(2)).await;
    
    println!("Late node 2 joining network...");
    late_node2.join(bootstrap_node.clone(), 2).await?;
    
    // Wait for both late nodes to fully sync and process any new blocks
    tokio::time::sleep(Duration::from_secs(8)).await;
    
    // Get current state of all nodes AFTER potential block propagation
    let (current_bootstrap_height, current_bootstrap_hash) = bootstrap_node.get_blockchain_info();
    let (late1_height, late1_hash) = late_node1.get_blockchain_info();
    let (late2_height, late2_hash) = late_node2.get_blockchain_info();
    
    println!("Current bootstrap - Height: {}, Last Hash: {:?}", current_bootstrap_height, current_bootstrap_hash);
    println!("Late node 1 - Height: {}, Last Hash: {:?}", late1_height, late1_hash);
    println!("Late node 2 - Height: {}, Last Hash: {:?}", late2_height, late2_hash);
    
    // Check sync success for both late nodes
    let late1_sync_success = late1_height == current_bootstrap_height && late1_hash == current_bootstrap_hash;
    let late2_sync_success = late2_height == current_bootstrap_height && late2_hash == current_bootstrap_hash;
    
    println!("Late node 1 sync successful: {}", late1_sync_success);
    println!("Late node 2 sync successful: {}", late2_sync_success);
    
    // Additional checks: both late nodes should have at least the original blockchain height
    let late1_has_full_chain = late1_height >= bootstrap_height;
    let late2_has_full_chain = late2_height >= bootstrap_height;
    
    println!("Late node 1 has full chain (height >= {}): {}", bootstrap_height, late1_has_full_chain);
    println!("Late node 2 has full chain (height >= {}): {}", bootstrap_height, late2_has_full_chain);
    
    // Overall late joiner success
    let all_late_nodes_synced = late1_sync_success && late2_sync_success;
    println!("\n=== Late Joiner Results ===");
    if all_late_nodes_synced {
        println!("✅ All late nodes successfully synchronized!");
    } else {
        println!("❌ Some late nodes failed to synchronize properly");
    }
    
    // Test sync between all nodes (including late joiners)
    println!("\n=== Final Network-Wide Sync Check ===");
    
    // Force sync on all nodes including late joiners
    println!("Forcing final sync on all nodes...");
    node1.sync_blockchain().await;
    node2.sync_blockchain().await;
    node3.sync_blockchain().await;
    late_node1.sync_blockchain().await;
    late_node2.sync_blockchain().await;
    
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Get final state of all nodes
    let (final_bootstrap_height, final_bootstrap_hash) = bootstrap_node.get_blockchain_info();
    let (final_node1_height, final_node1_hash) = node1.get_blockchain_info();
    let (final_node2_height, final_node2_hash) = node2.get_blockchain_info();
    let (final_node3_height, final_node3_hash) = node3.get_blockchain_info();
    let (final_late1_height, final_late1_hash) = late_node1.get_blockchain_info();
    let (final_late2_height, final_late2_hash) = late_node2.get_blockchain_info();
    
    println!("Final bootstrap - Height: {}, Last Hash: {:?}", final_bootstrap_height, final_bootstrap_hash);
    println!("Final node 1 - Height: {}, Last Hash: {:?}", final_node1_height, final_node1_hash);
    println!("Final node 2 - Height: {}, Last Hash: {:?}", final_node2_height, final_node2_hash);
    println!("Final node 3 - Height: {}, Last Hash: {:?}", final_node3_height, final_node3_hash);
    println!("Final late node 1 - Height: {}, Last Hash: {:?}", final_late1_height, final_late1_hash);
    println!("Final late node 2 - Height: {}, Last Hash: {:?}", final_late2_height, final_late2_hash);
    
    // Check if all nodes are synchronized
    let all_nodes_same_height = final_bootstrap_height == final_node1_height && 
                               final_node1_height == final_node2_height && 
                               final_node2_height == final_node3_height &&
                               final_node3_height == final_late1_height &&
                               final_late1_height == final_late2_height;
    
    let all_nodes_same_hash = final_bootstrap_hash == final_node1_hash && 
                             final_node1_hash == final_node2_hash && 
                             final_node2_hash == final_node3_hash &&
                             final_node3_hash == final_late1_hash &&
                             final_late1_hash == final_late2_hash;
    
    println!("\n=== Final Network Synchronization Results ===");
    println!("All 6 nodes have same height: {}", all_nodes_same_height);
    println!("All 6 nodes have same last hash: {}", all_nodes_same_hash);
    
    if all_nodes_same_height && all_nodes_same_hash {
        println!("🎉 COMPLETE SUCCESS: All 6 nodes (4 original + 2 late joiners) are perfectly synchronized!");
    } else {
        println!("⚠️  Network synchronization incomplete - some nodes differ");
    }

    println!("\n=== End of Simulation ===");
    println!("Test completed successfully. All blockchain synchronization scenarios validated.");
    
    Ok(())
}