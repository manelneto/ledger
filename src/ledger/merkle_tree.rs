use super::*;
use crate::ledger::transaction::Transaction;
use sha2::{Digest, Sha256};

pub type MerkleHash = Vec<u8>;

#[derive(Clone, Debug)]
pub struct MerkleNode {
    pub left: Option<Box<MerkleNode>>,
    pub right: Option<Box<MerkleNode>>,
    pub hash: MerkleHash,
}

#[derive(Clone, Debug)]
pub struct MerkleTree {
    pub root: Option<Box<MerkleNode>>,
    pub leaves: Vec<MerkleHash>,
}

#[derive(Clone, Debug)]
pub struct MerkleProof {
    pub proof: Vec<(MerkleHash, bool)>,
}

impl MerkleTree {
    pub fn new(transactions: &[Transaction]) -> Self {
        if transactions.is_empty() {
            return MerkleTree {
                root: None,
                leaves: Vec::new(),
            };
        }

        let mut leaves: Vec<MerkleHash> = transactions
            .iter()
            .map(|tx| tx.tx_hash.clone())
            .collect();

        if leaves.len() % 2 == 1 {
            leaves.push(leaves.last().unwrap().clone());
        }

        let leave_nodes: Vec<Box<MerkleNode>> = leaves
            .iter()
            .map(|hash| Box::new(MerkleNode {
                left: None,
                right: None,
                hash: hash.clone(),
            }))
            .collect();

        let root = Self::build_tree(leave_nodes);

        MerkleTree {
            root: Some(root),
            leaves: leaves[0..transactions.len()].to_vec(),
        }
    }

    fn build_tree(mut nodes: Vec<Box<MerkleNode>>) -> Box<MerkleNode> {
        if nodes.len() == 1 {
            return nodes.pop().unwrap();
        }

        let mut parent_nodes = Vec::new();

        for i in (0..nodes.len()).step_by(2) {
            let left = nodes[i].clone();
            let right = if i + 1 < nodes.len() {
                nodes[i + 1].clone()
            } else {
                nodes[i].clone()
            };

            let parent_hash = Self::hash_pair(&left.hash, &right.hash);

            parent_nodes.push(Box::new(MerkleNode {
                left: Some(left),
                right: Some(right),
                hash: parent_hash,
            }));
        }

        Self::build_tree(parent_nodes)
    }

    fn hash_pair(left: &[u8], right: &[u8]) -> MerkleHash {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().to_vec()
    }

    pub fn get_root_hash(&self) -> Option<MerkleHash> {
        self.root.as_ref().map(|root| root.hash.clone())
    }

    pub fn generate_proof(&self, tx_hash: &[u8]) -> Option<MerkleProof> {
        let leaf_index = self.leaves.iter().position(|leaf| leaf == tx_hash)?;

        let mut proof = Vec::new();
        let current_index = leaf_index;
        let current_level_size = self.leaves.len();

        self.traverse_for_proof(
            self.root.as_ref()?,
            &mut proof,
            current_index,
            current_level_size,
        );

        Some(MerkleProof { proof })
    }

    fn traverse_for_proof(
        &self,
        node: &MerkleNode,
        proof: &mut Vec<(MerkleHash, bool)>,
        index: usize,
        level_size: usize,
    ) {
        if node.left.is_none() && node.right.is_none() {
            return;
        }

        let is_right = index % 2 == 1;

        if let (Some(left), Some(right)) = (&node.left, &node.right) {
            if is_right {
                proof.push((left.hash.clone(), false));
                if let Some(right_node) = &node.right {
                    self.traverse_for_proof(
                        right_node,
                        proof,
                        index / 2,
                        (level_size + 1) / 2,
                    );
                }
            } else {
                proof.push((right.hash.clone(), true));
                if let Some(left_node) = &node.left {
                    self.traverse_for_proof(
                        left_node,
                        proof,
                        index / 2,
                        (level_size + 1) / 2,
                    );
                }
            }
        }
    }

    pub fn verify_proof(
        root_hash: &[u8],
        tx_hash: &[u8],
        proof: &MerkleProof,
    ) -> bool {
        let mut current_hash = tx_hash.to_vec();

        for (sibling_hash, is_right) in &proof.proof {
            current_hash = if *is_right {
                Self::hash_pair(&current_hash, sibling_hash)
            } else {
                Self::hash_pair(sibling_hash, &current_hash)
            };
        }

        current_hash == root_hash
    }

    pub fn from_hashes(hashes: Vec<MerkleHash>) -> Self {
        if hashes.is_empty() {
            return MerkleTree {
                root: None,
                leaves: Vec::new(),
            };
        }

        let mut leaves = hashes.clone();

        if leaves.len() % 2 == 1 {
            leaves.push(leaves.last().unwrap().clone());
        }

        let leaf_nodes: Vec<Box<MerkleNode>> = leaves
            .iter()
            .map(|hash| Box::new(MerkleNode {
                left: None,
                right: None,
                hash: hash.clone(),
            }))
            .collect();

        let root = Self::build_tree(leaf_nodes);

        MerkleTree {
            root: Some(root),
            leaves: hashes,
        }
    }

    pub fn get_leaves(&self) -> &[MerkleHash] {
        &self.leaves
    }

    pub fn print_tree(&self) {
        if let Some(root) = &self.root {
            self.print_node(root, 0);
        } else {
            println!("Empty tree");
        }
    }

    fn print_node(&self, node: &MerkleNode, depth: usize) {
        let indent = "  ".repeat(depth);
        println!("{}Hash: {}", indent, hex::encode(&node.hash));

        if let Some(left) = &node.left {
            println!("{}Left:", indent);
            self.print_node(left, depth + 1);
        }

        if let Some(right) = &node.right {
            println!("{}Right:", indent);
            self.print_node(right, depth + 1);
        }
    }
}

impl Hashable for MerkleTree {
    fn bytes(&self) -> Vec<u8> {
        self.get_root_hash().unwrap_or_default()
    }
}
