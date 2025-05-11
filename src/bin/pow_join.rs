
use blockchain::kademlia::node::Node;  
use std::time::Duration;  
use tokio::sync::oneshot;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let bootstrap_node = Node::new("127.0.0.1:50051".parse()?);
    let node1 = Node::new("127.0.0.1:50052".parse()?);
    let node2 = Node::new("127.0.0.1:50053".parse()?);

    let (shutdown_send, shutdown_recv) = oneshot::channel();
    
    let bootstrap_handle = tokio::spawn({
        let node = bootstrap_node.clone();
        async move {
            println!(" Bootstrap node starting on {}", node.get_address());
            node.start().await.expect("Bootstrap node crashed");
        }
    });

    let node1_handle = tokio::spawn({
        let node = node1.clone();
        async move {
            println!("Node 1 starting on {}", node.get_address());
            node.start().await.expect("Node 1 crashed");
        }
    });

    let node2_handle = tokio::spawn({
        let node = node2.clone();
        async move {
            println!("Node 2 starting on {}", node.get_address());
            node.start().await.expect("Node 2 crashed");
        }
    });

    tokio::time::sleep(Duration::from_secs(1)).await;

    println!("Node 1 joining network...");
    node1.join_with_pow(bootstrap_node.clone(), 2).await?;

    println!("Node 2 joining network...");
    node2.join_with_pow(bootstrap_node.clone(), 2).await?;

    println!(" Network State:");
    print_routing_table("Bootstrap", &bootstrap_node).await;
    print_routing_table("Node 1", &node1).await;
    print_routing_table("Node 2", &node2).await;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\n Received shutdown signal");
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
    println!(" {} knows {} nodes: {:?}",
        name,
        nodes.len(),
        nodes.iter().map(|n| n.get_address().port()).collect::<Vec<_>>()
    );
}