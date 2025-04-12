use blockchain::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use blockchain::kademlia::kademlia_proto::{Node, FindValueRequest};

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
