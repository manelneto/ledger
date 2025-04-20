use crate::kademlia::constants::{ID_LENGTH, N_BUCKETS};
use crate::kademlia::kbucket::KBucket;
use crate::kademlia::node::Node;
use std::array;

pub struct RoutingTable {
    id: [u8; ID_LENGTH],
    buckets: Vec<KBucket>,
}

impl RoutingTable {
    pub fn new(id: [u8; ID_LENGTH]) -> Self {
        Self {
            id,
            buckets: (0..N_BUCKETS).map(|_| KBucket::new()).collect(),
        }
    }

    pub fn find_closest_nodes(&self, id: &[u8; ID_LENGTH], k: usize) -> Vec<Node> {
        let mut nodes: Vec<Node> = self.buckets.iter().flat_map(|bucket| bucket.get_all_nodes()).collect();
        nodes.sort_by_key(|node| Self::xor_distance(id, node.get_id()));
        nodes.into_iter().take(k).collect()
    }

    fn index(&self, id: &[u8; ID_LENGTH]) -> Option<usize> {
        let xor: [u8; ID_LENGTH] = Self::xor_distance(&self.id, id);

        for (i, byte) in xor.iter().enumerate() {
            if *byte != 0 {
                return Some(i * 8 + byte.leading_zeros() as usize);
            }
        }

        None
    }

    pub fn replace_node(&mut self, lru: Node, node: Node) {
        if let Some(index) = self.index(lru.get_id()) {
            if let Some(bucket) = self.buckets.get_mut(index) {
                bucket.replace_lru(node);
            }
        }
    }

    pub fn update(&mut self, node: Node) -> Option<Node> {
        if let Some(index) = self.index(node.get_id()) {
            if let Some(bucket) = self.buckets.get_mut(index) {
                if bucket.update(node.clone()) {
                    return None;
                }
                return bucket.get_lru().cloned();
            }
        }
        None
    }

    fn xor_distance(a: &[u8; ID_LENGTH], b: &[u8; ID_LENGTH]) -> [u8; ID_LENGTH] {
        array::from_fn(|i| a[i] ^ b[i])
    }
}
