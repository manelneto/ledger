use blockchain::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use blockchain::kademlia::kademlia_proto::{Node as ProtoNode, StoreRequest};
use blockchain::kademlia::node::Node as LocalNode;
use std::net::SocketAddr;
use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = KademliaClient::connect("http://127.0.0.1:50051").await?;

    let address: SocketAddr = "127.0.0.1:50052".parse()?;
    let node = LocalNode::new(address);

    let sender = ProtoNode {
        id: node.get_id().to_vec(),
        ip: node.get_address().ip().to_string(),
        port: node.get_address().port() as u32,
    };

    let key = vec![42; 20];
    let value = b"Hello, Kademlia!".to_vec();

    let request = Request::new(StoreRequest {
        sender: Some(sender),
        key,
        value,
    });

    let response = client.store(request).await?.into_inner();

    println!("STORE response: {:?}", response);

    Ok(())
}
