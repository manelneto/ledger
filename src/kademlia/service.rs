use crate::kademlia::kademlia_proto::kademlia_server::Kademlia;
use crate::kademlia::kademlia_proto::{PingRequest, PingResponse, StoreRequest, StoreResponse, FindNodeRequest, FindNodeResponse, FindValueRequest, FindValueResponse};
use crate::kademlia::node::Node;
use tonic::{Request, Response, Status};

pub struct KademliaService {
    node: Node,
}

impl KademliaService {
    pub fn new(node: Node) -> Self {
        Self { node }
    }
}

#[tonic::async_trait]
impl Kademlia for KademliaService {
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        let sender = request.into_inner().sender;

        println!("PING from: {:?}", sender);

        Ok(Response::new(PingResponse {
            alive: true,
        }))
    }

    async fn store(&self, request: Request<StoreRequest>) -> Result<Response<StoreResponse>, Status> {
        let StoreRequest { sender, key, value } = request.into_inner();

        println!("STORE from: {:?}", sender);

        let key: [u8; 20] = match key.try_into() {
            Ok(k) => k,
            Err(_) => {
                return Err(Status::invalid_argument("STORE: KEY length must be 160 bits (20 bytes)"));
            }
        };

        let storage_arc = self.node.get_storage();
        let mut storage = storage_arc.write().unwrap();
        storage.insert(key, value);

        for (k, v) in storage.iter() {
            println!("Key: {:02x?}; Value: {:?}", k, String::from_utf8_lossy(v));
        }

        Ok(Response::new(StoreResponse {
            success: true,
        }))
    }

    async fn find_node(&self, request: Request<FindNodeRequest>) -> Result<Response<FindNodeResponse>, Status> {
        let FindNodeRequest { sender, id } = request.into_inner();

        println!("FIND_NODE from: {:?}", sender);

        let id: [u8; 20] = match id.try_into() {
            Ok(i) => i,
            Err(_) => {
                return Err(Status::invalid_argument("FIND_NODE: ID length must be 160 bits (20 bytes)"));
            }
        };

        Ok(Response::new(FindNodeResponse {
            nodes: vec![],
        }))
    }

    async fn find_value(&self, request: Request<FindValueRequest>) -> Result<Response<FindValueResponse>, Status> {
        let FindValueRequest { sender, key } = request.into_inner();

        println!("FIND_VALUE from: {:?}", sender);

        let key: [u8; 20] = match key.try_into() {
            Ok(k) => k,
            Err(_) => {
                return Err(Status::invalid_argument("FIND_VALUE: KEY length must be 160 bits (20 bytes)"));
            }
        };

        let storage_arc = self.node.get_storage();
        let storage = storage_arc.read().unwrap();

        if let Some(value) = storage.get(&key) {
            println!("Key: {:02x?}; Value: {:?}", key, value);

            Ok(Response::new(FindValueResponse {
                value: Some(value.clone()),
                nodes: vec![],
            }))
        } else {
            println!("Key: {:02x?}; Value: NOT FOUND", key);

            Ok(Response::new(FindValueResponse {
                value: None,
                nodes: vec![],
            }))
        }
    }
}
