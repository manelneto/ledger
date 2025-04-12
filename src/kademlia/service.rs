use crate::kademlia::kademlia_proto::kademlia_server::Kademlia;
use crate::kademlia::kademlia_proto::{PingRequest, PingResponse, StoreRequest, StoreResponse};
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

        println!("Ping from: {:?}", sender);

        Ok(Response::new(PingResponse { alive: true }))
    }

    async fn store(&self, request: Request<StoreRequest>) -> Result<Response<StoreResponse>, Status> {
        let StoreRequest { key, value, sender } = request.into_inner();

        println!("Store from: {:?}", sender);

        let key: [u8; 20] = match key.try_into() {
            Ok(k) => k,
            Err(_) => {
                return Err(Status::invalid_argument("STORE: key length must be 20 bytes"));
            }
        };

        let storage_arc = self.node.get_storage();
        let mut storage = storage_arc.write().unwrap();
        storage.insert(key, value);

        for (k, v) in storage.iter() {
            println!("Key: {:02x?}; Value: {:?}", k, String::from_utf8_lossy(v));
        }

        Ok(Response::new(StoreResponse { success: true }))
    }
}
