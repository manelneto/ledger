use crate::constants::{DIFFICULTY, ID_LENGTH, K, KEY_LENGTH};
use crate::kademlia::kademlia_proto::kademlia_server::Kademlia;
use crate::kademlia::kademlia_proto::{FindNodeRequest, FindNodeResponse, FindValueRequest, FindValueResponse, JoinRequest, JoinResponse, Node as ProtoNode, PingRequest, PingResponse, ShutdownRequest, ShutdownResponse, StoreRequest, StoreResponse};
use crate::kademlia::node::{BlockchainMessage, Node};
use std::sync::Arc;
use tokio::sync::Notify;
use tonic::{Request, Response, Status};

pub struct KademliaService {
    node: Node,
    shutdown: Arc<Notify>,
}

impl KademliaService {
    pub fn new(node: Node) -> Self {
        Self {
            node,
            shutdown: Arc::new(Notify::new()),
        }
    }

    pub fn new_with_shutdown(node: Node, shutdown: Arc<Notify>) -> Self {
        Self {
            node,
            shutdown,
        }
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

        if let Some(ref node) = sender {
            self.update_routing_table(node).await;
        }

        Ok(Response::new(PingResponse {
            alive: true,
        }))
    }

    async fn store(&self, request: Request<StoreRequest>) -> Result<Response<StoreResponse>, Status> {
        let StoreRequest { sender, key, value } = request.into_inner();

        if let Some(ref node) = sender {
            self.update_routing_table(node).await;
        }

        let key: [u8; KEY_LENGTH] = key.try_into().map_err(|_| {
            Status::invalid_argument("KEY length must be 160 bits (20 bytes)")
        })?;

        if let Some(response_data) = self.node.handle_blockchain_message(&value).await {
            let storage_lock = self.node.get_storage();
            let mut storage = storage_lock.write().map_err(|_| {
                Status::internal("failed to acquire lock on storage")
            })?;
            storage.insert(key, response_data);
        } else {
            let storage_lock = self.node.get_storage();
            let mut storage = storage_lock.write().map_err(|_| {
                Status::internal("failed to acquire lock on storage")
            })?;
            storage.insert(key, value);
        }

        Ok(Response::new(StoreResponse {
            success: true,
        }))
    }

    async fn find_node(&self, request: Request<FindNodeRequest>) -> Result<Response<FindNodeResponse>, Status> {
        let FindNodeRequest { sender, id } = request.into_inner();

        if let Some(ref node) = sender {
            self.update_routing_table(node).await;
        }

        let id: [u8; ID_LENGTH] = id.try_into().map_err(|_| {
            Status::invalid_argument("KEY length must be 160 bits (20 bytes)")
        })?;

        let routing_table_lock = self.node.get_routing_table();
        let routing_table = routing_table_lock.read().map_err(|_| {
            Status::internal("failed to acquire lock on routing table")
        })?;

        Ok(Response::new(FindNodeResponse {
            nodes: routing_table.find_closest_nodes(&id, K).into_iter().map(|n| n.to_send()).collect()
        }))
    }

    async fn find_value(&self, request: Request<FindValueRequest>) -> Result<Response<FindValueResponse>, Status> {
        let FindValueRequest { sender, key } = request.into_inner();

        if let Some(ref node) = sender {
            self.update_routing_table(node).await;
        }

        let key: [u8; KEY_LENGTH] = key.try_into().map_err(|_| {
            Status::invalid_argument("KEY length must be 160 bits (20 bytes)")
        })?;

        let storage_lock = self.node.get_storage();
        let storage = storage_lock.read().map_err(|_| {
            Status::internal("failed to acquire lock on storage")
        })?;

        if let Some(value) = storage.get(&key) {
            Ok(Response::new(FindValueResponse {
                value: Some(value.clone()),
                nodes: vec![],
            }))
        } else {
            let routing_table_lock = self.node.get_routing_table();
            let table = routing_table_lock.read().map_err(|_| {
                Status::internal("failed to acquire lock on routing table")
            })?;

            Ok(Response::new(FindValueResponse {
                value: None,
                nodes: table.find_closest_nodes(&key, K).into_iter().map(|n| n.to_send()).collect(),
            }))
        }
    }

    async fn join(&self, request: Request<JoinRequest>) -> Result<Response<JoinResponse>, Status> {
        let JoinRequest { sender, nonce, pow_hash } = request.into_inner();

        let sender_proto = sender.ok_or(Status::invalid_argument("no sender provided"))?;

        let sender = match Node::from_sender(&sender_proto) {
            Some(node) => node,
            None => return Err(Status::invalid_argument("invalid sender")),
        };

        if !self.node.verify_pow(sender.get_id(), &nonce, &pow_hash, DIFFICULTY) {
            return Err(Status::permission_denied("invalid Proof-of-Work"));
        }

        self.update_routing_table(&sender.to_send()).await;

        let closest_nodes = {
            let routing_table_lock = self.node.get_routing_table();
            let routing_table = routing_table_lock.write().map_err(|_| {
                Status::internal("failed to acquire lock on routing table")
            })?;
            let mut nodes = routing_table.find_closest_nodes(sender.get_id(), K)
                .into_iter()
                .map(|n| n.to_send())
                .collect::<Vec<_>>();

            if !nodes.iter().any(|n| n.id == self.node.get_id().to_vec()) {
                nodes.push(self.node.to_send());
            }

            nodes
        };

        tokio::spawn({
            let node = self.node.clone();
            let sender_node = sender.clone();
            async move {
                let blockchain = node.get_blockchain().read().unwrap().clone();
                let message = BlockchainMessage::ResponseFullBlockchain { blockchain };

                if let Ok(data) = serde_json::to_vec(&message) {
                    let blockchain_key = {
                        use sha2::{Digest, Sha256};
                        let mut hasher = Sha256::new();
                        hasher.update(b"initial_blockchain");
                        hasher.update(sender_node.get_id());
                        let hash = hasher.finalize();
                        hash[..KEY_LENGTH].try_into().unwrap_or([0; KEY_LENGTH])
                    };

                    let _ = node.store(blockchain_key, data).await;
                }
            }
        });

        Ok(Response::new(JoinResponse {
            accepted: true,
            closest_nodes,
        }))
    }

    async fn shutdown(&self, _request: Request<ShutdownRequest>) -> Result<Response<ShutdownResponse>, Status> {
        self.shutdown.notify_one();
        Ok(Response::new(ShutdownResponse {}))
    }
}
