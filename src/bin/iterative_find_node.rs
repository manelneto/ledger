use blockchain::kademlia::node::Node;
use blockchain::kademlia::kademlia_proto::kademlia_server::KademliaServer;
use blockchain::kademlia::service::KademliaService;
use std::net::SocketAddr;
use tonic::transport::Server;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id_a = [0x00; 20];
    let id_b = [0x01; 20];
    let id_c = [0x02; 20];
    let target = [0x01; 20];

    let addr_a: SocketAddr = "127.0.0.1:50051".parse()?;
    let addr_b: SocketAddr = "127.0.0.1:50052".parse()?;
    let addr_c: SocketAddr = "127.0.0.1:50053".parse()?;

    let node_a = Node::new_with_id(addr_a, id_a);
    let node_b = Node::new_with_id(addr_b, id_b);
    let node_c = Node::new_with_id(addr_c, id_c);

    tokio::spawn(async move {
        println!("Node A (bootstrap) at {}", addr_a);
        Server::builder()
            .add_service(KademliaServer::new(KademliaService::new(node_a)))
            .serve(addr_a)
            .await
            .unwrap();
    });

    sleep(Duration::from_secs(1)).await;

    println!("Node B bootstrapping...");
    node_b.bootstrap(Node::new_with_id(addr_a, id_a)).await?;

    println!("Node C bootstrapping...");
    node_c.bootstrap(Node::new_with_id(addr_a, id_a)).await?;

    println!("Node C running iterative_find_node...");
    let results = node_c.iterative_find_node(target).await;

    println!("Results:");
    for node in results {
        println!("- ID: {:02x?}, Addr: {}", node.get_id(), node.get_address());
    }

    Ok(())
}
