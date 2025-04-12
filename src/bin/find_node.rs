use blockchain::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use blockchain::kademlia::kademlia_proto::{Node, FindNodeRequest};

use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = KademliaClient::connect("http://127.0.0.1:50051").await?;

    let sender = Node {
        id: vec![0; 20],
        ip: "127.0.0.1".to_string(),
        port: 12345,
    };

    let id = vec![99; 20];

    let request = Request::new(FindNodeRequest {
        sender: Some(sender),
        id,
    });

    let response = client.find_node(request).await?.into_inner();

    println!("FIND_NODE response: {:?}", &response);

    Ok(())
}
