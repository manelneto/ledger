use ledgerlib::*;


fn main() {
    println!("Blockchain Test - With Transactions and Fork Handling");
    println!("====================================================\n");

    // Create a new blockchain
    let mut blockchain = Blockchain::new();

    println!("Created blockchain");
    println!("Genesis block: {:?}\n", blockchain.get_last_block().unwrap());

    // Generate key pairs for testing
    println!("Generating key pairs for testing...");
    let alice_keypair = Transaction::generate_keypair();
    let alice_pubkey = Transaction::get_public_key(&alice_keypair);
    
    let bob_keypair = Transaction::generate_keypair();
    let bob_pubkey = Transaction::get_public_key(&bob_keypair);
    
    println!("Alice's public key: {}", hex::encode(&alice_pubkey));
    println!("Bob's public key: {}", hex::encode(&bob_pubkey));
    
    // Create and add transactions to the pool
    println!("\nCreating and adding transactions to the pool...");
    
    // Alice creates a data transaction
    let data_tx = Transaction::create_data_tx(
        &alice_keypair,
        "Alice's auction item: Vintage Watch, starting bid: 100".to_string(),
        1, // nonce
        10, // fee
    );
    println!("Created data transaction: {:?}", data_tx);
    
    match blockchain.add_transaction(data_tx.clone()) {
        Ok(_) => println!("Added data transaction to pool"),
        Err(e) => println!("Error adding data transaction: {}", e),
    }
    
    // Bob creates a transfer transaction to Alice
    let transfer_tx = Transaction::create_transfer(
        &bob_keypair,
        alice_pubkey.clone(),
        200, // amount
        1,   // nonce
        15,  // fee
    );
    println!("Created transfer transaction: {:?}", transfer_tx);
    
    match blockchain.add_transaction(transfer_tx.clone()) {
        Ok(_) => println!("Added transfer transaction to pool"),
        Err(e) => println!("Error adding transfer transaction: {}", e),
    }
    
    // Mine a block with transactions
    println!("\nMining block with transactions...");
    match blockchain.mine_block(5) { // Mine with max 5 transactions
        Ok(block) => println!("Block mined: {:?}", block),
        Err(e) => println!("Error mining block: {}", e),
    }
    
    // Validate the chain
    println!("\nValidating blockchain...");
    if blockchain.is_chain_valid(None) {
        println!("Blockchain is valid!");
    } else {
        println!("Blockchain is invalid!");
    }
    
    // Mine another block
    println!("\nMining another block...");
    match blockchain.mine_empty_block() {
        Ok(block) => println!("Empty block mined: {:?}", block),
        Err(e) => println!("Error mining block: {}", e),
    }
    
    // Print the entire blockchain
    println!("\nBlockchain:");
    for (i, block) in blockchain.blocks.iter().enumerate() {
        println!("Block {}: {:?}", i, block);
    }
    
    // TEST FORK HANDLING
    println!("\n--------------- FORK TEST ---------------");
    
    // Create a fork
    println!("\nSimulating a fork from block 1...");
    match blockchain.simulate_fork("Fork block data".to_string()) {
        Ok(block) => println!("Fork block created: {:?}", block),
        Err(e) => println!("Error creating fork: {}", e),
    }
    
    // Check if the fork was created
    println!("\nCurrent forks: {}", blockchain.forks.len());
    
    // Add more transactions
    println!("\nAdding more transactions to pool...");
    
    // Alice creates another data transaction
    let data_tx2 = Transaction::create_data_tx(
        &alice_keypair,
        "Bidding update: Current highest bid is 250".to_string(),
        2, // nonce
        12, // fee
    );
    
    match blockchain.add_transaction(data_tx2) {
        Ok(_) => println!("Added second data transaction to pool"),
        Err(e) => println!("Error adding data transaction: {}", e),
    }
    
    // Create a second block in the fork to make it longer
    println!("\nAdding a second block to the fork to make it longer...");
    if let Some(fork) = blockchain.forks.values().next() {
        if let Some(last_fork_block) = fork.last() {
            let mut longer_fork_block = Block::new(
                last_fork_block.index + 1,
                now(),
                last_fork_block.hash.clone(),
                0,
                "Second block in fork".to_string(),
            );
            
            // Mine the block
            match blockchain.proof_of_work(&mut longer_fork_block) {
                Ok(_) => {
                    println!("Second fork block mined: {:?}", longer_fork_block);
                    
                    // Add to blockchain to handle the fork
                    match blockchain.receive_block(longer_fork_block) {
                        Ok(_) => println!("Second fork block added"),
                        Err(e) => println!("Error adding second fork block: {}", e),
                    }
                },
                Err(e) => println!("Error mining second fork block: {}", e),
            }
        }
    }
    
    // Check if the longer fork was adopted as the main chain
    println!("\nAfter fork resolution, blockchain:");
    for (i, block) in blockchain.blocks.iter().enumerate() {
        println!("Block {}: {:?}", i, block);
    }
    
    println!("\nRemaining forks: {}", blockchain.forks.len());
    
    // Validate the new main chain
    println!("\nValidating new main chain...");
    if blockchain.is_chain_valid(None) {
        println!("Blockchain is valid!");
    } else {
        println!("Blockchain is invalid!");
    }
    
    // Transaction pool status
    println!("\nTransaction pool status:");
    println!("Pending transactions: {}", blockchain.uncofirmed_transactions.size());
    
    // Mine the remaining transactions
    println!("\nMining remaining transactions...");
    match blockchain.mine_block(10) {
        Ok(block) => println!("Final block mined: {:?}", block),
        Err(e) => println!("Error mining final block: {}", e),
    }
    
    println!("\nTransactions left in pool: {}", blockchain.uncofirmed_transactions.size());
}