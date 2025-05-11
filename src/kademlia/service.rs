use crate::kademlia::constants::{ID_LENGTH, K, KEY_LENGTH};
use crate::kademlia::kademlia_proto::kademlia_server::Kademlia;
use crate::kademlia::kademlia_proto::{Node as ProtoNode, PingRequest, PingResponse, StoreRequest, StoreResponse, FindNodeRequest, FindNodeResponse, FindValueRequest, FindValueResponse, JoinRequest, JoinResponse};
use crate::kademlia::node::Node;
use tonic::{Request, Response, Status};

static DIFFICULTY_POW: usize = 2;

pub struct KademliaService {
    node: Node,
}

impl KademliaService {
    pub fn new(node: Node) -> Self {
        Self { node }
    }

    async fn update_routing_table(&self, sender: &ProtoNode) {
        if let Some(sender) = Node::from_sender(sender) {
            let routing_table_lock = self.node.get_routing_table();
            let lru = {
                let mut table = match routing_table_lock.write() {
                    Ok(lock) => lock,
                    Err(_) => return,
                };
                table.update(sender.clone())
            };

            if let Some(lru_node) = lru {
                if let Ok(false) = self.node.ping(&lru_node).await {
                    if let Ok(mut table) = routing_table_lock.write() {
                        table.replace_node(lru_node, sender);
                    }
                }
            }
        }
    }
}

#[tonic::async_trait]
impl Kademlia for KademliaService {
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        let sender = request.into_inner().sender;

        println!("PING from: {:?}", sender);

        if let Some(ref node) = sender {
            self.update_routing_table(node).await;
        }

        Ok(Response::new(PingResponse {
            alive: true,
        }))
    }

    async fn store(&self, request: Request<StoreRequest>) -> Result<Response<StoreResponse>, Status> {
        let StoreRequest { sender, key, value } = request.into_inner();

        println!("STORE from: {:?}", sender);

        if let Some(ref node) = sender {
            self.update_routing_table(node).await;
        }

        let key: [u8; KEY_LENGTH] = key.try_into().map_err(|_| {
            Status::invalid_argument("STORE: KEY length must be 160 bits (20 bytes)")
        })?;

        let storage_lock = self.node.get_storage();
        let mut storage = storage_lock.write().map_err(|_| {
           Status::internal("STORE: failed to acquire lock on storage")
        })?;
        storage.insert(key, value);

        Ok(Response::new(StoreResponse {
            success: true,
        }))
    }

    async fn find_node(&self, request: Request<FindNodeRequest>) -> Result<Response<FindNodeResponse>, Status> {
        let FindNodeRequest { sender, id } = request.into_inner();

        println!("FIND_NODE from: {:?}", sender);

        if let Some(ref node) = sender {
            self.update_routing_table(node).await;
        }

        let id: [u8; ID_LENGTH] = id.try_into().map_err(|_| {
            Status::invalid_argument("FIND_NODE: KEY length must be 160 bits (20 bytes)")
        })?;

        let routing_table_lock = self.node.get_routing_table();
        let routing_table = routing_table_lock.read().map_err(|_| {
            Status::internal("FIND_NODE: failed to acquire lock on routing table")
        })?;

        Ok(Response::new(FindNodeResponse {
            nodes: routing_table.find_closest_nodes(&id, K).into_iter().map(|n| n.to_send()).collect()
        }))
    }

    async fn find_value(&self, request: Request<FindValueRequest>) -> Result<Response<FindValueResponse>, Status> {
        let FindValueRequest { sender, key } = request.into_inner();

        println!("FIND_VALUE from: {:?}", sender);

        if let Some(ref node) = sender {
            self.update_routing_table(node).await;
        }

        let key: [u8; KEY_LENGTH] = key.try_into().map_err(|_| {
            Status::invalid_argument("FIND_VALUE: KEY length must be 160 bits (20 bytes)")
        })?;

        let storage_lock = self.node.get_storage();
        let storage = storage_lock.read().map_err(|_| {
            Status::internal("FIND_VALUE: failed to acquire lock on storage")
        })?;

        if let Some(value) = storage.get(&key) {
            println!("Key: {:02x?}; Value: {:?}", key, value);

            Ok(Response::new(FindValueResponse {
                value: Some(value.clone()),
                nodes: vec![],
            }))
        } else {
            println!("Key: {:02x?}; Value: NOT FOUND", key);

            let routing_table_lock = self.node.get_routing_table();
            let table = routing_table_lock.read().map_err(|_| {
                Status::internal("FIND_VALUE: failed to acquire lock on routing table")
            })?;

            Ok(Response::new(FindValueResponse {
                value: None,
                nodes: table.find_closest_nodes(&key, K).into_iter().map(|n| n.to_send()).collect(),
            }))
        }
    }

    async fn join(&self, request: Request<JoinRequest>) -> Result<Response<JoinResponse>, Status> {
        let JoinRequest { sender, nonce, pow_hash } = request.into_inner();
        
        let sender_node = match Node::from_sender(&sender.ok_or(Status::invalid_argument("No sender provided"))?) {
            Some(node) => node,
            None => return Err(Status::invalid_argument("Invalid sender node")),
        };

        if !self.node.verify_pow(sender_node.get_id(), &nonce, &pow_hash, DIFFICULTY_POW) { 
            return Err(Status::permission_denied("Invalid PoW proof"));
        }

        self.update_routing_table(&sender_node.to_send()).await;

        let routing_table_lock = self.node.get_routing_table();
        let routing_table = routing_table_lock.read().map_err(|_| {
            Status::internal("Failed to acquire lock on routing table")
        })?;

        let closest_nodes = {
            let table = self.node.get_routing_table();
            let unwrap_table = table.read().unwrap();
            let mut nodes = unwrap_table.find_closest_nodes(sender_node.get_id(), K)
                .into_iter()
                .map(|n| n.to_send())
                .collect::<Vec<_>>();
            
            if !nodes.iter().any(|n| n.id == self.node.get_id().to_vec()) {
                nodes.push(self.node.to_send());
            }

            nodes
        };

        Ok(Response::new(JoinResponse {
            accepted: true,
            closest_nodes,
        }))
    }
}

