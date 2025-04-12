use crate::ledger::block::Block;
use crate::ledger::lib::*;
use crate::ledger::Hashable;
use std::collections::HashMap;

const DIFFICULTY_PREFIX: &str = "00000";
pub struct Blockchain {
    pub blocks: Vec<Block>,
    //pub uncofirmed_transactions: Vec<Transaction>,
    pub difficulty: usize,
    pub forks: HashMap<BHash, Vec<Block>>,
}

impl Blockchain {
    pub fn new() -> Self {
        let mut chain = Blockchain {
            blocks: Vec::new(),
            //uncofirmed_transactions: Vec::new(),
            difficulty: DIFFICULTY_PREFIX.len(),
            forks: HashMap::new(),
        };

        chain.create_genesis_block();
        chain
    }

    fn create_genesis_block(&mut self){
        let genesis_block = Block::new(0, now(), vec![0;32], 0, "Genesis block".to_owned());

        // Calculte the hash of the genesis block
        let hash = genesis_block.hash();
        let mut genesis = genesis_block;
        genesis.hash = hash;

        self.blocks.push(genesis);
    }

    pub fn add_block(&mut self, block:Block) -> Result<(), &'static str>{
        let last_block = match self.blocks.last() {
            Some(block) => block,
            None => return Err("Blockchain has no blocks"),
        };

        if block.prev_hash != last_block.hash {
            return Err("Block has invalid previous hash");
        }

        if block.index != last_block.index + 1 {
            return Err("Block has invalid index");
        }

        if !self.is_block_hash_valid(&block.hash()) {
            return Err("Block hash doesn't meet difficulty requirements");
        }

        self.blocks.push(block);
        Ok(())
    }

    //Proof of Work: Mining
    pub fn mine_block(&mut self, payload: String) -> Result<Block, &'static str> {
        let last_block = match self.blocks.last(){
            Some(block) => block,
            None => return Err("Blockchain has no blocks"),
        };

        let mut new_block = Block::new(
            last_block.index + 1,
            now(),
            last_block.hash.clone(),
            0,
            payload,
        );

        self.proof_of_work(&mut new_block)?;

        self.add_block(new_block.clone())?;

        Ok(new_block)
    }

    pub fn proof_of_work(&self, block: &mut Block) -> Result<(), &'static str> {
        println!("Mining block {:?}", &block);
        
        let target = "0".repeat(self.difficulty);

        loop{
            block.hash = block.hash();
            let hash_str = hex::encode(&block.hash);

            if hash_str.starts_with(&target){
                println!("Found valid hash: {}", hash_str);
                return Ok(());
            }

            block.nonce += 1;

            if block.nonce == u64::MAX {
                return Err("No valid nonce found");
            }
        }
    }

    fn is_block_hash_valid(&self, hash: &[u8]) -> bool {
        let hash_string = hex::encode(hash);
        let target = "0".repeat(self.difficulty);
        hash_string.starts_with(&target)
    }

    pub fn is_chain_valid(&self, chain: Option<&Vec<Block>>) -> bool {
        // If no chain is provided, validate the main chain
        let chain_to_validate = chain.unwrap_or(&self.blocks);

        for i in 1..chain_to_validate.len() {
            let current_block = &chain_to_validate[i];
            let prev_block = &chain_to_validate[i - 1];

            if current_block.hash != current_block.hash(){
                println!("Current block hash is invalid");
                return false;
            }

            if current_block.prev_hash != prev_block.hash {
                println!("Current block previous hash is invalid");
                return false;
            }

            if !self.is_block_hash_valid(&current_block.hash()) {
                println!("Current block hash doesn't meet difficulty requirements");
                return false;
            }
        }
        true
    }

    pub fn get_last_block(&self) -> Option<&Block> {
        self.blocks.last()
    }

    pub fn receive_block(&mut self, block:Block) -> Result<(), &'static str> {
        if !self.is_block_hash_valid(&block.hash()) {
            return Err("Block hash doesn't meet difficulty requirements");
        }

        if let Some(last_block) = self.blocks.last() {
            if block.prev_hash == last_block.hash && block.index == last_block.index + 1 {
                self.blocks.push(block);
                println!("Block added to main chain");
                return Ok(());
            }
        }

        for (i, existing_block) in self.blocks.iter().enumerate(){
            if block.prev_hash == existing_block.hash{
                // Create a new fork from this point
                let mut fork_chain  = self.blocks[0..=i].to_vec();
                fork_chain.push(block.clone());

                //Store the Fork
                self.forks.insert(existing_block.hash.clone(), fork_chain);
                println!("Fork created from block index {}", i);

                // Check if this fork is now the biggest chain
                self.resolve_forks();

                return Ok(());
            } 
        }

        let mut fork_update = false;

        for (_ , fork_chain) in &mut self.forks{
            if let Some(last_fork_block) = fork_chain.last(){
                if block.prev_hash == last_fork_block.hash && block.index == last_fork_block.index + 1{
                    fork_chain.push(block.clone());
                    fork_update = true;
                    println!("Block added to existing fork");
                    break;
                }
            }
        }

        if fork_update {
            //check updated forks
            self.resolve_forks();
            return Ok(());
        }
        Err("Block doesn't fit in any chain")
    }

    pub fn resolve_forks(&mut self){
        let main_chain_length = self.blocks.len();

        let mut forks_to_remove= Vec::new();

        for (fork_key, fork_chain) in &self.forks  {
            // if the fork is valid and longer then the main chain.
            if self.is_chain_valid(Some(fork_chain)) && fork_chain.len() > main_chain_length{
                println!("Found longer valid fork (length:{})", fork_chain.len());

                // Use the fork as main chain
                self.blocks = fork_chain.clone();

                // Remove all the forks 
                for key in self.forks.keys(){
                    forks_to_remove.push(key.clone());
                }

                break; 
            }
            // If fork is invalid, remove him to.
            if !self.is_chain_valid(Some(fork_chain)){
                forks_to_remove.push(fork_key.clone());
            }
        }

        for key in forks_to_remove{
            self.forks.remove(&key);
        }
    }

    // Simulate fork creation for test proporses - TO REMOVE AFTER FLIGHT 😉//
    pub fn simulate_fork(&mut self, payload: String) -> Result<Block, &'static str> {
        if self.blocks.len() < 2 {
            return Err("Need at least 2 blocks to create a fork");
        }

        let fork_base_index = self.blocks.len() - 2;
        let fork_base = &self.blocks[fork_base_index];

        let mut fork_block = Block::new(
            fork_base.index + 1,
            now(),
            fork_base.hash.clone(),
            0,
            payload,
        );

        self.proof_of_work(&mut fork_block)?;

        match self.receive_block(fork_block.clone()){
            Ok(_) => Ok(fork_block),
            Err(e) => Err(e),
        }
    }
}
