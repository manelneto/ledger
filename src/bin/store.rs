use blockchain::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use blockchain::kademlia::kademlia_proto::{Node, StoreRequest};

use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = KademliaClient::connect("http://127.0.0.1:50051").await?;

    let sender = Node {
        id: vec![0; 20],
        ip: "127.0.0.1".to_string(),
        port: 12345,
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
