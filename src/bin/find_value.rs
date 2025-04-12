use blockchain::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use blockchain::kademlia::kademlia_proto::{Node as ProtoNode, FindValueRequest};
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

    let request = Request::new(FindValueRequest {
        sender: Some(sender),
        key,
    });

    let response = client.find_value(request).await?.into_inner();

    match response.value {
        Some(value) => {
            println!("FIND_VALUE response: {:?}", &value);
        }
        None => {
            println!("FIND_VALUE response: NOT FOUND {:?}", response.nodes);
        }
    }

    Ok(())
}
