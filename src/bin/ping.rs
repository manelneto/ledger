use blockchain::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use blockchain::kademlia::kademlia_proto::PingRequest;
use blockchain::kademlia::node::Node;
use std::net::SocketAddr;
use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = KademliaClient::connect("http://127.0.0.1:50051").await?;

    let address: SocketAddr = "127.0.0.1:50052".parse()?;
    let sender = Node::new(address).to_send();

    let request = Request::new(PingRequest {
        sender: Some(sender),
    });

    let response = client.ping(request).await?.into_inner();

    println!("PING response: {:?}", response);

    Ok(())
}
