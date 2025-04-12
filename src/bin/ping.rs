use blockchain::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use blockchain::kademlia::kademlia_proto::{PingRequest, Node};

use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = KademliaClient::connect("http://127.0.0.1:50051").await?;

    let sender = Node {
        id: vec![0; 20],
        ip: "127.0.0.1".to_string(),
        port: 12345,
    };

    let request = Request::new(PingRequest {
        sender: Some(sender),
    });

    let response = client.ping(request).await?;

    println!("Pong from: {:?}", response.into_inner());

    Ok(())
}
