use crate::kademlia::constants::K;
use crate::kademlia::node::Node;
use std::collections::VecDeque;
use std::fmt;

pub struct KBucket {
    nodes: VecDeque<Node>,
}

impl fmt::Display for KBucket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "KBucket:")?;
        for node in &self.nodes {
            writeln!(f, "    - {}", node)?;
        }
        Ok(())
    }
}

impl KBucket {
    pub fn new() -> Self {
        Self {
            nodes: VecDeque::new(),
        }
    }

    pub fn contains(&self, node: &Node) -> bool {
        self.nodes.iter().any(|n| n.get_id() == node.get_id())
    }

    pub fn get_nodes(&self) -> impl Iterator<Item = Node> + '_ {
        self.nodes.iter().cloned()
    }

    pub fn get_lru(&self) -> Option<&Node> {
        self.nodes.front()
    }

    pub fn is_full(&self) -> bool {
        self.nodes.len() >= K
    }

    pub fn replace_lru(&mut self, node: Node) {
        self.nodes.pop_front();
        self.nodes.push_back(node);
    }

    pub fn remove(&mut self, node: &Node) {
        if let Some(pos) = self.nodes.iter().position(|n| n.get_id() == node.get_id()) {
            self.nodes.remove(pos);
        }
    }

    pub fn update(&mut self, node: Node) -> bool {
        if let Some(pos) = self.nodes.iter().position(|n| n.get_id() == node.get_id()) {
            self.nodes.remove(pos);
            self.nodes.push_back(node);
            true
        } else if self.nodes.len() < K {
            self.nodes.push_back(node);
            true
        } else {
            false
        }
    }
}
