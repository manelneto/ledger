pub mod kbucket;
pub mod node;
pub mod routing_table;
pub mod service;

pub mod kademlia_proto {
    tonic::include_proto!("kademlia");
}
