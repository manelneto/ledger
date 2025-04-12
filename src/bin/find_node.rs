use blockchain::kademlia::kademlia_proto::kademlia_client::KademliaClient;
use blockchain::kademlia::kademlia_proto::FindNodeRequest;
use blockchain::kademlia::node::Node;
use std::net::SocketAddr;
use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = KademliaClient::connect("http://127.0.0.1:50051").await?;

    let address: SocketAddr = "127.0.0.1:50052".parse()?;
    let sender = Node::new(address).to_send();

    let id = vec![99; 20];

    let request = Request::new(FindNodeRequest {
        sender: Some(sender),
        id,
    });

    let response = client.find_node(request).await?.into_inner();

    println!("FIND_NODE response: {:?}", &response);

    Ok(())
}
