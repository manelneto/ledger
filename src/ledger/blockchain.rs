// src/ledger/blockchain.rs
use super::*;
use std::vec;
use std::collections::{HashMap, HashSet};
use crate::ledger::block::Block;
use crate::ledger::lib::{now, BHash};
use crate::ledger::transaction::{Transaction, PublicKey, TransactionType};
use std::time::{Duration, Instant};
use crate::ledger::merkle_tree::{MerkleTree, MerkleProof};
use serde::{Serialize, Deserialize};

const DIFFICULTY_PREFIX: &str = "0000";
const MAX_BLOCK_TIME: u128 = 600_000; // 10 minutes
const MIN_BLOCK_TIME: u128 = 1_000; // 1 second
const MAX_MINING_TIME: Duration = Duration::from_secs(300);
const MAX_FORK_DEPTH: usize = 6;

#[derive(Serialize, Deserialize)]
pub struct Blockchain {
    pub blocks: Vec<Block>,
    pub difficulty: usize,
    pub forks: HashMap<BHash, Vec<Block>>,
    pub balances: HashMap<Vec<u8>, u64>,
}

impl Blockchain {
    pub fn new() -> Self {
        let mut chain = Blockchain {
            blocks: Vec::new(),
            difficulty: DIFFICULTY_PREFIX.len(),
            forks: HashMap::new(),
            balances: HashMap::new(),
        };
        chain.create_genesis_block();
        chain
    }

    fn create_genesis_block(&mut self) {
        let genesis_block = Block::genesis();
        let hash = genesis_block.hash();
        let mut genesis = genesis_block;
        genesis.hash = hash;
        self.blocks.push(genesis);
    }

    /// Criar um novo bloco candidato
    pub fn create_block(&self, transactions: Vec<Transaction>) -> Result<Block, &'static str> {
        let last_block = self.get_last_block()
            .ok_or("No blocks in chain")?;
        
        let new_block = Block::new(
            last_block.index + 1,
            now(),
            last_block.hash.clone(),
            0, // nonce inicial
            transactions,
        );
        
        Ok(new_block)
    }

    /// Fazer proof of work em um bloco (apenas PoW)
    pub fn mine_block(&self, block: &mut Block) -> Result<(), &'static str> {
        let start_time = Instant::now();
        let target = "0".repeat(self.difficulty);

        loop {
            if start_time.elapsed() > MAX_MINING_TIME {
                return Err("Mining timed out - adjust difficulty");
            }

            block.hash = block.hash();
            let hash_str = hex::encode(&block.hash);

            if hash_str.starts_with(&target) {
                return Ok(());
            }

            block.nonce += 1;

            if block.nonce == u64::MAX {
                return Err("Nonce overflow - difficulty too high");
            }
        }
    }

    /// Validar e adicionar bloco à cadeia
    pub fn add_block(&mut self, block: Block) -> Result<(), &'static str> {
        // Validação completa
        self.validate_block(&block)?;
        
        // Processar transações
        self.process_block_transactions(&block)?;
        
        // Adicionar à cadeia
        self.blocks.push(block);
        Ok(())
    }

    /// Validar bloco completamente
    fn validate_block(&self, block: &Block) -> Result<(), &'static str> {
        let last_block = self.get_last_block()
            .ok_or("No blocks in chain")?;

        // Validar encadeamento
        if block.prev_hash != last_block.hash {
            return Err("Block has invalid previous hash");
        }

        if block.index != last_block.index + 1 {
            return Err("Block has invalid index");
        }

        // Validar timestamp
        let time_diff = block.timestamp.saturating_sub(last_block.timestamp);
        if time_diff < MIN_BLOCK_TIME {
            return Err("Block time is too short");
        }
        if time_diff > MAX_BLOCK_TIME {
            return Err("Block time is too long");
        }

        // Validar proof of work
        if !self.is_block_hash_valid(&block.hash) {
            return Err("Block hash doesn't meet difficulty requirements");
        }

        // Validar transações
        self.validate_transactions(block)?;
        
        Ok(())
    }

    fn validate_transactions(&self, block: &Block) -> Result<(), &'static str> {
        if block.index == 0 || block.transactions.is_empty() {
            return Ok(());
        }

        let mut tx_hashes = HashSet::new();
        for tx in &block.transactions {
            if !tx_hashes.insert(tx.tx_hash.clone()) {
                return Err("Duplicate transaction in block");
            }

            if !tx.verify() {
                return Err("Block contains invalid transaction signature");
            }

            if let Some(amount) = tx.data.amount {
                let sender_balance = self.balances.get(&tx.data.sender).unwrap_or(&0);
                let total_cost = amount + tx.data.fee;
                
                if *sender_balance < total_cost {
                    return Err("Insufficient balance for transaction");
                }
            }
        }

        Ok(())
    }

    fn process_block_transactions(&mut self, block: &Block) -> Result<(), &'static str> {
        if block.index == 0 || block.transactions.is_empty() {
            return Ok(());
        }

        for tx in &block.transactions {
            if !tx.verify() {
                return Err("Block contains invalid transaction");
            }

            match tx.data.tx_type {
                TransactionType::Transfer => {
                    if let Some(amount) = tx.data.amount {
                        let sender_balance = self.balances.entry(tx.data.sender.clone())
                            .or_insert(0);
                        *sender_balance = sender_balance.saturating_sub(amount + tx.data.fee);
                        
                        if let Some(receiver) = &tx.data.receiver {
                            let receiver_balance = self.balances.entry(receiver.clone())
                                .or_insert(0);
                            *receiver_balance += amount;
                        }
                    }
                },
                TransactionType::Data => {
                    let sender_balance = self.balances.entry(tx.data.sender.clone())
                        .or_insert(0);
                    *sender_balance = sender_balance.saturating_sub(tx.data.fee);
                }
            }
        }

        Ok(())
    }

    pub fn get_last_block(&self) -> Option<&Block> {
        self.blocks.last()
    }

    pub fn get_balance(&self, address: &PublicKey) -> u64 {
        *self.balances.get(address).unwrap_or(&0)
    }

    pub fn get_block_height(&self) -> usize {
        self.blocks.len()
    }

    pub fn get_blocks_from(&self, start_index: usize) -> Vec<Block> {
        if start_index >= self.blocks.len() {
            return Vec::new();
        }
        self.blocks[start_index..].to_vec()
    }

    pub fn get_next_nonce(&self, sender: &PublicKey) -> u64 {
        let mut confirmed_nonce = 0;
        for block in &self.blocks {
            for tx in &block.transactions {
                if tx.data.sender == *sender && tx.data.nonce > confirmed_nonce {
                    confirmed_nonce = tx.data.nonce;
                }
            }
        }
        confirmed_nonce + 1
    }

    fn is_block_hash_valid(&self, hash: &[u8]) -> bool {
        let hash_string = hex::encode(hash);
        let target = "0".repeat(self.difficulty);
        hash_string.starts_with(&target)
    }

    pub fn is_chain_valid(&self, chain: Option<&Vec<Block>>) -> bool {
        let chain_to_validate = chain.unwrap_or(&self.blocks);

        for i in 0..chain_to_validate.len() {
            let current_block = &chain_to_validate[i];

            if current_block.hash != current_block.hash() {
                return false;
            }

            if !self.verify_block_merkle_root(current_block) {
                return false;
            }

            if i > 0 {
                let prev_block = &chain_to_validate[i - 1];
                
                if current_block.prev_hash != prev_block.hash {
                    return false;
                }

                if !self.is_block_hash_valid(&current_block.hash()) {
                    return false;
                }
            }
        }
        true
    }

    pub fn verify_block_merkle_root(&self, block: &Block) -> bool {
        let merkle_tree = MerkleTree::new(&block.transactions);
        if let Some(calculated_root) = merkle_tree.get_root_hash() {
            calculated_root == block.merkle_root
        } else {
            block.merkle_root == vec![0; 32] && block.transactions.is_empty()
        }
    }

    pub fn receive_block(&mut self, block: Block) -> Result<(), &'static str> {
        if !self.is_block_hash_valid(&block.hash) {
            return Err("Block hash doesn't meet difficulty requirements");
        }

        let current_time = now();
        if block.timestamp > current_time + 7_200_000 {
            return Err("Block timestamp is too far in the future");
        }

        if let Some(last_block) = self.blocks.last() {
            if block.prev_hash == last_block.hash && block.index == last_block.index + 1 {
                return self.add_block(block);
            }
        }

        // Fork handling
        for (i, existing_block) in self.blocks.iter().enumerate() {
            if block.prev_hash == existing_block.hash {
                if (self.blocks.len() - i) > MAX_FORK_DEPTH {
                    return Err("Fork depth exceeded");
                }

                let mut fork_chain = self.blocks[0..=i].to_vec();
                if !self.validate_fork_chain(&fork_chain, &block) {
                    return Err("Invalid fork chain");
                }

                fork_chain.push(block.clone());
                self.forks.insert(existing_block.hash.clone(), fork_chain);
                self.resolve_forks();
                return Ok(());
            }
        }

        Err("Block doesn't fit in any chain")
    }

    fn validate_fork_chain(&self, fork_chain: &Vec<Block>, new_block: &Block) -> bool {
        for i in 1..fork_chain.len() {
            let current = &fork_chain[i];
            let prev = &fork_chain[i-1];
            
            if current.prev_hash != prev.hash {
                return false;
            }
            
            if !self.is_block_hash_valid(&current.hash()) {
                return false;
            }
        }
        
        let last_fork = fork_chain.last().unwrap();
        new_block.prev_hash == last_fork.hash && 
        new_block.index == last_fork.index + 1 &&
        self.is_block_hash_valid(&new_block.hash())
    }

    pub fn resolve_forks(&mut self) {
        let main_chain_length = self.blocks.len();
        let mut forks_to_remove = Vec::new();
        let fork_keys: Vec<_> = self.forks.keys().cloned().collect();

        for fork_key in fork_keys {
            let fork_chain_cloned = self.forks.get(&fork_key).cloned();

            if let Some(fork_chain) = fork_chain_cloned {
                if fork_chain.len() > main_chain_length + MAX_FORK_DEPTH {
                    forks_to_remove.push(fork_key.clone());
                    continue;
                }

                if self.is_chain_valid(Some(&fork_chain)) && fork_chain.len() > main_chain_length {
                    if self.get_cumulative_work(&fork_chain) > self.get_cumulative_work(&self.blocks) {
                        self.revert_to_fork_state(&fork_chain);
                        self.blocks = fork_chain.clone();
                        forks_to_remove.extend(self.forks.keys().cloned());
                        break;
                    }
                }

                if !self.is_chain_valid(Some(&fork_chain)) {
                    forks_to_remove.push(fork_key.clone());
                }
            }
        }

        for key in forks_to_remove {
            self.forks.remove(&key);
        }
    }

    fn get_cumulative_work(&self, chain: &Vec<Block>) -> u128 {
        chain.iter().map(|block| {
            let leading_zeros = block.hash.iter()
                .take_while(|&&byte| byte == 0)
                .count() * 8 + 
                block.hash.iter()
                .find(|&&byte| byte != 0)
                .map_or(0, |&byte| byte.leading_zeros() as usize);
            2u128.pow(leading_zeros as u32)
        }).sum()
    }

    fn revert_to_fork_state(&mut self, fork_chain: &Vec<Block>) {
        self.balances.clear();
        for block in fork_chain {
            let _ = self.process_block_transactions(block);
        }
    }

    pub fn get_block_headers(&self) -> Vec<crate::ledger::block::BlockHeader> {
        self.blocks.iter().map(|block| block.get_header()).collect()
    }

    pub fn get_block_header(&self, index: usize) -> Option<crate::ledger::block::BlockHeader> {
        self.blocks.get(index).map(|block| block.get_header())
    }

    pub fn get_transaction_proof(&self, block_index: usize, tx_hash: &[u8]) -> Option<MerkleProof> {
        if let Some(block) = self.blocks.get(block_index) {
            block.generate_inclusion_proof(tx_hash)
        } else {
            None
        }
    }

    pub fn verify_transaction_in_chain(&self, tx_hash: &[u8], proof: &MerkleProof, block_index: usize) -> bool {
        if let Some(block) = self.blocks.get(block_index) {
            block.verify_transaction_inclusion(tx_hash, proof)
        } else {
            false
        }
    }
}

// Light client implementation
pub struct LightClient {
    headers: Vec<crate::ledger::block::BlockHeader>,
}

impl LightClient {
    pub fn new() -> Self {
        LightClient {
            headers: Vec::new(),
        }
    }

    pub fn add_header(&mut self, header: crate::ledger::block::BlockHeader) -> Result<(), &'static str> {
        if let Some(last_header) = self.headers.last() {
            if header.prev_hash != last_header.hash {
                return Err("Invalid header chain");
            }
        }
        
        self.headers.push(header);
        Ok(())
    }

    pub fn verify_transaction(&self, tx_hash: &[u8], proof: &MerkleProof, block_index: usize) -> bool {
        if let Some(header) = self.headers.get(block_index) {
            MerkleTree::verify_proof(&header.merkle_root, tx_hash, proof)
        } else {
            false
        }
    }

    pub fn get_height(&self) -> usize {
        self.headers.len()
    }
}