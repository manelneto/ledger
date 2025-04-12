mod kademlia;

use kademlia::kademlia_proto::kademlia_server::KademliaServer;
use kademlia::node::Node;
use kademlia::service::KademliaService;
use std::net::SocketAddr;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = "127.0.0.1:50051".parse()?;
    let node = Node::new(address);

    println!("Listening on {}", address);
    println!("ID: {:02x?}", node.get_id());

    let service = KademliaService::new(node);
    Server::builder()
        .add_service(KademliaServer::new(service))
        .serve(address)
        .await?;

    Ok(())
}
