use ledgerlib::*;

fn main() {
    println!("Blockchain Test - Without Transactions");
    println!("=====================================\n");

    // Create a new blockchain
    let mut blockchain = Blockchain::new();

    println!("Created blockchain");
    println!("Genesis block: {:?}\n", blockchain.get_last_block().unwrap());

    // Mine a few blocks with simple string payloads
    println!("Mining block 1...");
    match blockchain.mine_block("Test data for block 1".to_string()) {
        Ok(block) => println!("Block mined: {:?}", block),
        Err(e) => println!("Error: {}", e),
    }

    println!("\nMining block 2...");
    match blockchain.mine_block("Test data for block 2".to_string()) {
        Ok(block) => println!("Block mined: {:?}", block),
        Err(e) => println!("Error: {}", e),
    }

    // Validate the chain
    println!("\nValidating blockchain...");
    if blockchain.is_chain_valid() {
        println!("Blockchain is valid!");
    } else {
        println!("Blockchain is invalid!");
    }

    // Print the entire blockchain
    println!("\nBlockchain:");
    for (i, block) in blockchain.blocks.iter().enumerate() {
        println!("Block {}: {:?}", i, block);
    }
}