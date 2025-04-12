use crate::kademlia::constants::K;
use crate::kademlia::node::Node;
use std::collections::VecDeque;

pub struct KBucket {
    nodes: VecDeque<Node>,
}

impl KBucket {
    pub fn new() -> Self {
        Self {
            nodes: VecDeque::new(),
        }
    }

    pub fn get_all_nodes(&self) -> impl Iterator<Item = Node> + '_ {
        self.nodes.iter().cloned()
    }

    pub fn update(&mut self, node: Node) {
        if let Some(pos) = self.nodes.iter().position(|n| n.get_id() == node.get_id()) {
            self.nodes.remove(pos);
            self.nodes.push_back(node);
        } else if self.nodes.len() < K {
            self.nodes.push_back(node);
        } else {
            // TODO: LRU
        }
    }
}
