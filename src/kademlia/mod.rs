pub mod node;
pub mod service;

pub mod kademlia_proto {
    tonic::include_proto!("kademlia");
}
