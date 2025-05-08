// src/bin/pow_join.rs
use blockchain::kademlia::node::Node;  // Import the Node struct
use std::time::Duration;  // Import Duration
use tokio::sync::oneshot;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize nodes
    let bootstrap_node = Node::new("127.0.0.1:50051".parse()?);
    let node1 = Node::new("127.0.0.1:50052".parse()?);
    let node2 = Node::new("127.0.0.1:50053".parse()?);

    // 2. Start all nodes in separate tasks
    let (shutdown_send, shutdown_recv) = oneshot::channel();
    
    let bootstrap_handle = tokio::spawn({
        let node = bootstrap_node.clone();
        async move {
            println!("🛡️ Bootstrap node starting on {}", node.get_address());
            node.start().await.expect("Bootstrap node crashed");
        }
    });

    let node1_handle = tokio::spawn({
        let node = node1.clone();
        async move {
            println!("🆕 Node 1 starting on {}", node.get_address());
            node.start().await.expect("Node 1 crashed");
        }
    });

    let node2_handle = tokio::spawn({
        let node = node2.clone();
        async move {
            println!("🆕 Node 2 starting on {}", node.get_address());
            node.start().await.expect("Node 2 crashed");
        }
    });

    // 3. Wait briefly for servers to start
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 4. Run join procedures
    println!("🚀 Node 1 joining network...");
    node1.join_with_pow(bootstrap_node.clone(), 2).await?;

    println!("🚀 Node 2 joining network...");
    node2.join_with_pow(bootstrap_node.clone(), 2).await?;

    // 5. Verify network state
    println!("🔍 Network State:");
    print_routing_table("Bootstrap", &bootstrap_node).await;
    print_routing_table("Node 1", &node1).await;
    print_routing_table("Node 2", &node2).await;

    // 6. Keep nodes running until Ctrl-C
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\n🛑 Received shutdown signal");
            let _ = shutdown_send.send(());
        },
        _ = bootstrap_handle => {},
        _ = node1_handle => {},
        _ = node2_handle => {},
    }

    Ok(())
}

async fn print_routing_table(name: &str, node: &Node) {
    let table = node.get_routing_table();
    let read_table = table.read().unwrap();
    let nodes = read_table.find_closest_nodes(node.get_id(), 10);
    println!("📊 {} knows {} nodes: {:?}",
        name,
        nodes.len(),
        nodes.iter().map(|n| n.get_address().port()).collect::<Vec<_>>()
    );
}