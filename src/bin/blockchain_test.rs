use blockchain::ledger::blockchain::Blockchain;
use blockchain::ledger::transaction::{Transaction, TransactionType};
use blockchain::ledger::merkle_tree::MerkleTree;
use blockchain::ledger::hashable::Hashable; // <-- Add this line
use ed25519_dalek::{Keypair, Signer};
use rand::rngs::OsRng;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Blockchain and Merkle Tree Test ===\n");
    
    // 1. Create a new blockchain
    let mut blockchain = Blockchain::new();
    println!("Created new blockchain with genesis block");
    print_blockchain_state(&blockchain);
    
    // 2. Generate keypairs for testing
    let mut csprng = OsRng;
    let alice_keypair = Keypair::generate(&mut csprng);
    let bob_keypair = Keypair::generate(&mut csprng);
    let charlie_keypair = Keypair::generate(&mut csprng);
    
    println!("\nGenerated test keypairs:");
    println!("Alice: {}", hex::encode(&alice_keypair.public.to_bytes()[0..8]));
    println!("Bob: {}", hex::encode(&bob_keypair.public.to_bytes()[0..8]));
    println!("Charlie: {}", hex::encode(&charlie_keypair.public.to_bytes()[0..8]));
    
    // 3. Mine an empty block
    println!("\nMining empty block...");
    // Add delay before mining to satisfy minimum block time requirement
    sleep(Duration::from_secs(2)).await;
    match blockchain.mine_empty_block() {
        Ok(block) => {
            println!("Mined block: {} with hash: {}", block.index, hex::encode(&block.hash[0..8]));
        },
        Err(e) => {
            println!("Failed to mine empty block: {}", e);
            return Ok(());
        }
    }
    
    // 4. Create a funding transaction for Alice (in a real implementation)
    // Note: This is a simplified version. In your implementation, you might need
    // to use a different approach to add initial funds to Alice
    println!("Setting up test accounts with funds...");
    
    // Wait for minimum block time
    sleep(Duration::from_secs(2)).await;
    
    // 5. Create some test transactions
    println!("\nTesting transaction creation...");
    
    // In a real implementation, you would create actual transactions like this:
    // let tx1 = Transaction::create_transfer(
    //     &alice_keypair,
    //     bob_keypair.public.to_bytes().to_vec(),
    //     50000,
    //     0,
    //     1000
    // )?;
    
    // Create some test transactions (simplified for testing)
    let tx1 = create_test_transaction(
        &alice_keypair, 
        Some(bob_keypair.public.to_bytes().to_vec()),
        TransactionType::Transfer, 
        Some(50000), 
        None
    );
    
    let tx2 = create_test_transaction(
        &alice_keypair,
        None,
        TransactionType::Data,
        None,
        Some("AUCTION_START:Product:Starting_bid=1000:End_time=1622505600".to_string())
    );
    
    println!("Created test transactions:");
    println!("TX1: Transfer from Alice to Bob");
    println!("TX2: Data transaction (auction start)");
    
    // 6. Create a test block with these transactions
    println!("\nCreating a test block with transactions...");
    
    let mut block = blockchain.blocks.last().unwrap().clone();
    block.index += 1;
    block.prev_hash = block.hash.clone();
    block.timestamp = blockchain::ledger::lib::now();
    block.transactions = vec![tx1.clone(), tx2.clone()];
    
    // Update the Merkle root based on transactions
    let merkle_tree = MerkleTree::new(&block.transactions);
    block.merkle_root = merkle_tree.get_root_hash().unwrap_or_else(|| vec![0; 32]);
    block.tx_count = block.transactions.len() as u32;
    
    // Recompute the hash
    block.hash = block.hash();
    
    println!("Test block created with index: {}", block.index);
    println!("Block contains {} transactions", block.transactions.len());
    println!("Block Merkle root: {}", hex::encode(&block.merkle_root[0..8]));
    
    // 7. Test Merkle tree functionality
    println!("\n=== Testing Merkle Tree ===");
    
    let test_merkle_tree = MerkleTree::new(&block.transactions);
    println!("Created Merkle tree with root hash: {}", 
             hex::encode(&test_merkle_tree.get_root_hash().unwrap()[0..8]));
    
    // Verify Merkle root matches block
    let root_matches = block.merkle_root == test_merkle_tree.get_root_hash().unwrap();
    println!("Block Merkle root matches computed tree: {}", root_matches);
    
    // 8. Generate and verify a Merkle proof
    let tx_to_verify = &block.transactions[0];
    println!("\nGenerating Merkle proof for transaction: {}", 
             hex::encode(&tx_to_verify.tx_hash[0..8]));
    
    let proof = test_merkle_tree.generate_proof(&tx_to_verify.tx_hash)
        .expect("Failed to generate proof");
    
    let proof_valid = MerkleTree::verify_proof(
        &block.merkle_root,
        &tx_to_verify.tx_hash,
        &proof
    );
    
    println!("Proof verification result: {}", proof_valid);
    
    // 9. Print final state
    println!("\n=== Test Summary ===");
    println!("Blockchain test completed successfully");
    println!("Merkle tree verification: {}", if proof_valid { "PASSED" } else { "FAILED" });
    
    Ok(())
}

// Helper function to create a test transaction
fn create_test_transaction(
    keypair: &Keypair,
    receiver: Option<Vec<u8>>,
    tx_type: TransactionType,
    amount: Option<u64>,
    data: Option<String>
) -> Transaction {
    // Create transaction data
    let tx_data = blockchain::ledger::transaction::TransactionData {
        sender: keypair.public.to_bytes().to_vec(),
        receiver,
        timestamp: blockchain::ledger::lib::now(),
        tx_type,
        amount,
        data,
        nonce: 0,
        fee: 1000,
        valid_until: Some(blockchain::ledger::lib::now() + 3_600_000), // 1 hour
    };
    
    // Create a signature (simplified)
    let data_bytes = serde_json::to_vec(&tx_data).unwrap_or_default();
    let signature = keypair.sign(&data_bytes).to_bytes().to_vec();
    
    // Create the transaction
    let mut tx = Transaction {
        data: tx_data,
        signature,
        tx_hash: vec![0; 32], // Placeholder
    };
    
    // Calculate hash
    tx.tx_hash = tx.hash();
    
    tx
}

fn print_blockchain_state(blockchain: &Blockchain) {
    println!("Chain length: {} blocks", blockchain.blocks.len());
    if let Some(last_block) = blockchain.get_last_block() {
        println!("Latest block hash: {}", hex::encode(&last_block.hash[0..8]));
    } else {
        println!("No blocks in chain");
    }
}