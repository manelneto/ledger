use tonic::{Request, Response, Status};

use crate::kademlia::kademlia_proto::kademlia_server::Kademlia;
use crate::kademlia::kademlia_proto::{PingRequest, PingResponse};

pub struct KademliaService;

#[tonic::async_trait]
impl Kademlia for KademliaService {
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        let sender = request.into_inner().sender;

        println!("Ping from: {:?}", sender);

        Ok(Response::new(PingResponse { alive: true }))
    }
}
