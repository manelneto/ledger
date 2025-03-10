// Module: blockchain
use super::*;
use std::{collections::HashSet, vec};
//use std::collections::HashMap;

const DIFFICULTY_PREFIX: &str = "00000";
pub struct Blockchain {
    pub blocks: Vec<Block>,
    //pub uncofirmed_transactions: Vec<Transaction>,
    pub difficulty: usize,
    unspent_outputs:HashSet<Hash>,
    
}

impl Blockchain {
    pub fn new() -> Self {
        let mut chain = Blockchain {
            blocks: Vec::new(),
            //uncofirmed_transactions: Vec::new(),
            difficulty: DIFFICULTY_PREFIX.len(),
            unspent_outputs: HashSet::new(),
        };

        chain.create_genesis_block();
        chain
    }

    //TODO
    fn create_genesis_block(&mut self){
        let genesis_block = Block::new(0, now(), vec![0;32], vec![
            Transaction {
                inputs: vec![ ],
                outputs: vec![ ],
            },
        ]);

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

        if let Some((coinbase,transactions)) = 
        block.transactions.split_first(){
            if !coinbase.is_coinbase(){
                return  Err("InvalidCoinbaseTransaction");
            }
            let mut block_spent = HashSet::new();
            let mut block_created = HashSet::new();
            let mut total_fee = 0;

            for transaction in transactions{
                let input_hashes = transaction.input_hashses();

                if !(&input_hashes - &self.unspent_outputs).is_empty() ||
                    !(&input_hashes & &block_spent).is_empty()
                {
                    return Err ("Invalid Input");
                }

                let input_value = transaction.input_value();
                let output_value = transaction.output_value();

                if output_value > input_value {
                    return Err ("Transaction has insufficient input value");
                }

                let fee = input_value - output_value;
                total_fee += fee;

                block_spent.extend(input_hashes);
                block_created.extend(transaction.output_hashses());

            }


            //TODO: Add the ammount for mining too
            if coinbase.output_value() < total_fee {
                return Err ("InvalidCoinbaseTransaction");
            } else {
                block_created.extend(coinbase.output_hashses());
            }

            self.unspent_outputs.retain(|output| !block_spent.contains(output));
            self.unspent_outputs.extend(block_created);

        }

        self.blocks.push(block);
        Ok(())
    }

    //Proof of Work: Mining
    pub fn mine_block(&mut self) -> Result<Block, &'static str> {
        let last_block = match self.blocks.last(){
            Some(block) => block,
            None => return Err("Blockchain has no blocks"),
        };

        let mut new_block = Block::new(
            last_block.index + 1,
            now(),
            last_block.hash.clone(),
            vec![
            Transaction {
                inputs: vec![ ],
                outputs: vec![ ],
            },
        ]
        );

        self.proof_of_work(&mut new_block)?;

        self.add_block(new_block.clone())?;

        Ok(new_block)
    }

    fn proof_of_work(&self, block: &mut Block) -> Result<(), &'static str> {
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

    pub fn is_chain_valid(&self) -> bool {
        for i in 1..self.blocks.len() {
            let current_block = &self.blocks[i];
            let previous_block = &self.blocks[i - 1];

            if current_block.hash != current_block.hash(){
                println!("Current block hash is invalid");
                return false;
            }

            if current_block.prev_hash != previous_block.hash {
                println!("Current block previous hash is invalid");
                return false;
            }

            if !self.is_block_hash_valid(&current_block.hash){
                println!("Current block hash is invalid");
                return false;
            }
        }
        true
    }

    pub fn get_last_block(&self) -> Option<&Block> {
        self.blocks.last()
    }

}