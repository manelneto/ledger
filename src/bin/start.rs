use blockchain::kademlia::kademlia_proto::kademlia_server::KademliaServer;
use blockchain::kademlia::node::Node;
use blockchain::kademlia::service::KademliaService;
use tonic::transport::Server;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let default_port = 50051;
    let port: u16 = args.get(1)
        .map(|s| s.parse().expect("Invalid port"))
        .unwrap_or(default_port);

    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);

    let node = Node::new(address);

    println!("[BOOTSTRAP] Listening on {}", address);
    println!("[BOOTSTRAP] ID: {:02x?}", node.get_id());

    let service = KademliaService::new(node);
    Server::builder()
        .add_service(KademliaServer::new(service))
        .serve(address)
        .await?;

    Ok(())
}
