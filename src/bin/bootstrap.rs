use blockchain::kademlia::kademlia_proto::kademlia_server::KademliaServer;
use blockchain::kademlia::node::Node;
use blockchain::kademlia::service::KademliaService;
use tonic::transport::Server;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap_address: SocketAddr = "127.0.0.1:50051".parse()?;
    let bootstrap_node = Node::new(bootstrap_address);

    let kademlia_service = KademliaService::new(bootstrap_node.clone());

    tokio::spawn(async move {
        println!("BOOTSTRAP NODE at: {}", bootstrap_address);
        Server::builder()
            .add_service(KademliaServer::new(kademlia_service))
            .serve(bootstrap_address)
            .await
            .unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let client_address: SocketAddr = "127.0.0.1:50052".parse()?;
    let client_node = Node::new(client_address);

    println!("CLIENT NODE at: {}", client_address);

    client_node.bootstrap(bootstrap_node).await?;

    let routing_table_lock = client_node.get_routing_table();
    let routing_table = routing_table_lock.read().unwrap();
    let nodes = routing_table.find_closest_nodes(client_node.get_id(), 10);
    for node in nodes {
        println!("ID: {:02x?}, IP: {}", node.get_id(), node.get_address());
    }

    Ok(())
}