mod kademlia;

use kademlia::node::Node;
use kademlia::service::KademliaService;
use kademlia::kademlia_proto::kademlia_server::KademliaServer;
use tonic::transport::Server;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id = [0u8; 20];
    let address: SocketAddr = "127.0.0.1:50051".parse()?;

    let node = Node::new(id, address);
    let service = KademliaService::new(node);

    println!("Listening on {}", address);

    Server::builder()
        .add_service(KademliaServer::new(service))
        .serve(address)
        .await?;

    Ok(())
}

/*mod kademlia;

use ledgerlib::*;

fn main() {
    println!("Blockchain Test - With Fork Handling");
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

    println!("\nMining block 3...");
    match blockchain.mine_block("Test data for block 3".to_string()) {
        Ok(block) => println!("Block mined: {:?}", block),
        Err(e) => println!("Error: {}", e),
    }

    // Validate the chain
    println!("\nValidating blockchain...");
    if blockchain.is_chain_valid(None) {
        println!("Blockchain is valid!");
    } else {
        println!("Blockchain is invalid!");
    }

    // Print the entire blockchain
    println!("\nBlockchain:");
    for (i, block) in blockchain.blocks.iter().enumerate() {
        println!("Block {}: {:?}", i, block);
    }
    
    // TEST DE FORK
    println!("\n--------------- FORK TEST ---------------");
    
    // Simular um fork
    println!("\nSimulating a fork from block 1...");
    match blockchain.simulate_fork("Fork block data".to_string()) {
        Ok(block) => println!("Fork block created: {:?}", block),
        Err(e) => println!("Error creating fork: {}", e),
    }
    
    // Verificar se o fork foi criado
    println!("\nCurrent forks: {}", blockchain.forks.len());
    
    // Criar um segundo bloco no fork para torná-lo mais longo que a cadeia principal
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
            
            // Minerar o bloco
            match blockchain.proof_of_work(&mut longer_fork_block) {
                Ok(_) => {
                    println!("Second fork block mined: {:?}", longer_fork_block);
                    
                    // Adicionar à blockchain para lidar com o fork
                    match blockchain.receive_block(longer_fork_block) {
                        Ok(_) => println!("Second fork block added"),
                        Err(e) => println!("Error adding second fork block: {}", e),
                    }
                },
                Err(e) => println!("Error mining second fork block: {}", e),
            }
        }
    }
    
    // Verificar se o fork mais longo foi adotado como cadeia principal
    println!("\nAfter fork resolution, blockchain:");
    for (i, block) in blockchain.blocks.iter().enumerate() {
        println!("Block {}: {:?}", i, block);
    }
    
    println!("\nRemaining forks: {}", blockchain.forks.len());
    
    // Validar a nova cadeia principal
    println!("\nValidating new main chain...");
    if blockchain.is_chain_valid(None) {
        println!("Blockchain is valid!");
    } else {
        println!("Blockchain is invalid!");
    }
}*/
