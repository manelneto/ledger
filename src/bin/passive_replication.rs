#[tokio::main]
async fn main() {
    use blockchain::kademlia::node::Node;
    use blockchain::kademlia::service::KademliaService;
    use blockchain::kademlia::kademlia_proto::kademlia_server::KademliaServer;
    use blockchain::kademlia::constants::{KEY_LENGTH};
    use std::net::SocketAddr;
    use tonic::transport::Server;
    use tokio::time::{sleep, Duration};

    let addr_a: SocketAddr = "127.0.0.1:50051".parse().unwrap();
    let addr_b: SocketAddr = "127.0.0.1:50052".parse().unwrap();
    let addr_c: SocketAddr = "127.0.0.1:50053".parse().unwrap();

    let id_a = [0x00; 20];
    let id_b = [0x01; 20];
    let id_c = [0x02; 20];

    let node_a = Node::new_with_id(addr_a, id_a);
    let node_b = Node::new_with_id(addr_b, id_b);
    let node_c = Node::new_with_id(addr_c, id_c);

    tokio::spawn(Server::builder()
        .add_service(KademliaServer::new(KademliaService::new(node_a.clone())))
        .serve(addr_a));
    tokio::spawn(Server::builder()
        .add_service(KademliaServer::new(KademliaService::new(node_b.clone())))
        .serve(addr_b));
    tokio::spawn(Server::builder()
        .add_service(KademliaServer::new(KademliaService::new(node_c.clone())))
        .serve(addr_c));

    sleep(Duration::from_millis(500)).await;

    println!("Node B bootstrapping...");
    node_b.bootstrap(node_a.clone()).await.unwrap();

    println!("Node B storing...");
    let key = [0xAA; KEY_LENGTH];
    let value = b"Passive replication test".to_vec();
    node_b.store(key, value.clone()).await.unwrap();

    println!("Node C bootstrapping...");
    node_c.bootstrap(node_a.clone()).await.unwrap();

    println!("Node C retrieving...");
    let result = node_c.iterative_find_value(key).await;

    assert_eq!(result, Some(value.clone()));
    println!("Value successfully retrieved by C.");

    // Confirma replicação passiva em C
    let storage_lock = node_c.get_storage();
    let storage = storage_lock.read().unwrap();
    let replicated = storage.get(&key).cloned();
    assert_eq!(replicated, Some(value.clone()));
}
