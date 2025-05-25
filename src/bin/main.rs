use std::env;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{self as tokio_io, AsyncBufReadExt};
use tokio::sync::Notify;
use tonic::Status;
use tonic::transport::Server;
use blockchain::auction::auction_commands::AuctionCommand;
use blockchain::kademlia::kademlia_proto::kademlia_server::KademliaServer;
use blockchain::kademlia::node::Node;
use blockchain::kademlia::service::KademliaService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        println!("Usage: cargo run --bin main <SELF PORT> <BOOTSTRAP PORT> <POW DIFFICULTY>");
        return Ok(());
    }

    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let port: u16 = args[1].parse()?;
    let bootstrap_port: u16 = args[2].parse()?;
    let difficulty: usize = args[3].parse()?;

    let address = SocketAddr::new(ip, port);
    let bootstrap_address = SocketAddr::new(ip, bootstrap_port);

    let node = Node::new(address);
    let shutdown = Arc::new(Notify::new());
    let shutdown_trigger = shutdown.clone();
    let service = KademliaService::new_with_shutdown(node.clone(), shutdown);

    let server = Server::builder()
        .add_service(KademliaServer::new(service))
        .serve_with_shutdown(address, async move {
            shutdown_trigger.notified().await;
        });

    tokio::select! {
        result = server => result?,
        result = menu(node.clone(), ip, address, bootstrap_address, difficulty) => result?,
    }

    println!("Shutting down...");
    Ok(())
}

async fn menu(node: Node, ip: IpAddr, address: SocketAddr, bootstrap_address: SocketAddr, difficulty: usize) -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let bootstrap_node = Node::new(bootstrap_address);
    if let Err(e) = node.join(bootstrap_node.clone(), difficulty).await {
        eprintln!("JOIN error: {}", e);
    }

    let stdin = tokio_io::BufReader::new(tokio_io::stdin());
    let mut lines = stdin.lines();

    loop {
        println!("\n=== MENU {} ===", address);
        println!("0. EXIT");
        println!("1. PING");
        println!("2. STORE");
        println!("3. FIND NODE");
        println!("4. FIND VALUE");
        println!("5. WHO AM I?");
        println!("6. CREATE AUCTION");
        println!("7. START AUCTION");
        println!("8. END AUCTION");
        println!("9. PLACE BID");
        print!("\nOption: ");
        io::stdout().flush().unwrap();

        let input = match lines.next_line().await? {
            Some(line) => line.trim().to_string(),
            None => continue,
        };

        match input.as_str() {
            "0" => return Ok(()),
            "1" => {
                let port: u16 = prompt_parse("Target Port: ").await;
                let target = Node::new(SocketAddr::new(ip, port));
                match node.ping(&target).await {
                    Ok(true) => println!("{} is alive!", target),
                    _ => {
                        println!("{} is not alive!", target);
                        let routing_table_lock = node.get_routing_table();
                        let mut routing_table = routing_table_lock.write().map_err(|_| {
                            Status::internal("MAIN: failed to acquire lock on routing table")
                        })?;
                        routing_table.remove(&target);
                    }
                }
            },
            "2" => {
                let key = prompt_hex("Key (40 hex chars): ").await;
                let value = prompt("Value: ").await.into_bytes();
                match key.try_into() {
                    Ok(key_array) => match node.store(key_array, value).await {
                        Ok(_) => println!("STORE successful!"),
                        Err(e) => println!("STORE error: {}", e),
                    },
                    Err(_) => println!("Key must be exactly 40 hex characters (20 bytes)."),
                }
            },
            "3" => {
                let id = prompt_hex("Target ID (40 hex chars): ").await;
                let port: u16 = prompt_parse("Target Port: ").await;
                let target = Node::new(SocketAddr::new(ip, port));
                match id.try_into() {
                    Ok(id_array) => {
                        match node.find_node(target, id_array).await {
                            Ok(nodes) => {
                                println!("FIND_NODE successful!");
                                for node in nodes {
                                    println!("{}", node);
                                }
                            }
                            Err(e) => println!("FIND_NODE error: {}", e),
                        };
                    }
                    Err(_) => println!("ID must be exactly 40 hex characters (20 bytes)."),
                }
            },
            "4" => {
                let key = prompt_hex("Key (40 hex chars): ").await;
                let port: u16 = prompt_parse("Target Port: ").await;
                let target = Node::new(SocketAddr::new(ip, port));
                match key.try_into() {
                    Ok(key_array) => {
                        match node.find_value(target, key_array).await {
                            Ok((Some(value), _)) => {
                                println!("FIND_VALUE successful!");
                                println!("Value: {:?}", String::from_utf8_lossy(&value));
                            }
                            Ok((None, nodes)) => {
                                println!("Value not found. Closest nodes:");
                                for node in nodes {
                                    println!("{}", node);
                                }
                            }
                            Err(e) => println!("FIND_VALUE error: {}", e),
                        }
                    }
                    Err(_) => println!("Key must be exactly 40 hex characters (20 bytes)."),
                }
            },
            "5" => {
                println!("I am {}", node);
                let routing_table_lock = node.get_routing_table();
                let routing_table = routing_table_lock.read().map_err(|_| {
                    Status::internal("MAIN: failed to acquire lock on routing table")
                })?;
                println!("{}", *routing_table);
            },
            "6" => {
                let id = prompt("Auction ID: ").await;
                let title = prompt("Title: ").await;
                let description = prompt("Description: ").await;
                let command = AuctionCommand::CreateAuction { id: id.clone(), title, description };

                let serialized = format!("AUCTION_{}", serde_json::to_string(&command)?);
                let key = sha256_truncate(&id);

                match node.store(key, serialized.into_bytes()).await {
                    Ok(_) => println!("Auction created and stored successfully!"),
                    Err(e) => println!("Error storing auction: {}", e),
                }
            },
            "7" => {
                let id = prompt("Auction ID: ").await;
                let command = AuctionCommand::StartAuction { id: id.clone() };

                let serialized = format!("AUCTION_{}", serde_json::to_string(&command)?);
                let key = sha256_truncate(&format!("{}_start", id));

                match node.store(key, serialized.into_bytes()).await {
                    Ok(_) => println!("Auction started successfully!"),
                    Err(e) => println!("Error starting auction: {}", e),
                }
            },
            "8" => {
                let id = prompt("Auction ID: ").await;
                let command = AuctionCommand::EndAuction { id: id.clone() };

                let serialized = format!("AUCTION_{}", serde_json::to_string(&command)?);
                let key = sha256_truncate(&format!("{}_end", id));

                match node.store(key, serialized.into_bytes()).await {
                    Ok(_) => println!("Auction ended successfully!"),
                    Err(e) => println!("Error ending auction: {}", e),
                }
            },
            "9" => {
                let id = prompt("Auction ID: ").await;
                let amount: u64 = prompt_parse("Bid amount (integer): ").await;
                let command = AuctionCommand::Bid { id: id.clone(), amount };

                let serialized = format!("AUCTION_{}", serde_json::to_string(&command)?);
                let key = sha256_truncate(&format!("{}_bid_{}", id, amount));

                match node.store(key, serialized.into_bytes()).await {
                    Ok(_) => println!("Bid placed successfully!"),
                    Err(e) => println!("Error placing bid: {}", e),
                }
            },
            _ => println!("Invalid option."),
        }
    }
}

async fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    let mut reader = tokio_io::BufReader::new(tokio_io::stdin());
    reader.read_line(&mut input).await.unwrap();
    input.trim().to_string()
}

async fn prompt_hex(msg: &str) -> Vec<u8> {
    loop {
        let input = prompt(msg).await;
        match hex::decode(&input) {
            Ok(bytes) => return bytes,
            Err(_) => println!("Invalid hex input. Please try again."),
        }
    }
}

async fn prompt_parse<T: FromStr>(msg: &str) -> T {
    loop {
        let input = prompt(msg).await;
        match input.parse::<T>() {
            Ok(value) => return value,
            Err(_) => println!("Invalid input. Please try again."),
        }
    }
}

fn sha256_truncate(input: &str) -> [u8; 20] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    let mut hash = [0u8; 20];
    hash.copy_from_slice(&result[..20]);
    hash
}

