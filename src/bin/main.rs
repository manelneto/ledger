use std::env;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use tonic::Status;
use tonic::transport::Server;
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
    let service = KademliaService::new(node.clone());

    tokio::spawn(async move {
        Server::builder()
            .add_service(KademliaServer::new(service))
            .serve(address)
            .await
            .unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let bootstrap_node = Node::new(bootstrap_address);
    node.join(bootstrap_node.clone(), difficulty).await?;

    loop {
        println!("\n=== MENU {} ===", address);
        println!("0. EXIT");
        println!("1. PING");
        println!("2. STORE");
        println!("3. FIND NODE");
        println!("4. FIND VALUE");
        println!("5. WHO AM I?");
        print!("\nOption: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim() {
            "0" => break,
            "1" => {
                let port: u16 = prompt_parse("Target Port: ");
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
            }
            "2" => {
                let key = prompt_hex("Key (40 hex chars): ");
                let value = prompt("Value: ").into_bytes();
                match key.try_into() {
                    Ok(key_array) => match node.store(key_array, value).await {
                        Ok(_) => println!("STORE successful!"),
                        Err(e) => println!("STORE error: {}", e),
                    },
                    Err(_) => println!("Key must be exactly 40 hex characters (20 bytes)."),
                }
            }
            "3" => {
                let id = prompt_hex("Target ID (40 hex chars): ");
                let port: u16 = prompt_parse("Target Port: ");
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
            }
            "4" => {
                let key = prompt_hex("Key (40 hex chars): ");
                let port: u16 = prompt_parse("Target Port: ");
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
            }
            "5" => {
                println!("I am {}", node);
                let routing_table_lock = node.get_routing_table();
                let routing_table = routing_table_lock.read().map_err(|_| {
                    Status::internal("MAIN: failed to acquire lock on routing table")
                })?;
                println!("{}", *routing_table);
            }
            _ => println!("Invalid option."),
        }
    }

    Ok(())
}

fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn prompt_hex(msg: &str) -> Vec<u8> {
    loop {
        let input = prompt(msg);
        match hex::decode(input) {
            Ok(bytes) => return bytes,
            Err(_) => println!("Invalid hex input. Please try again."),
        }
    }
}

fn prompt_parse<T: FromStr>(msg: &str) -> T {
    loop {
        let input = prompt(msg);
        match input.parse::<T>() {
            Ok(value) => return value,
            Err(_) => println!("Invalid input. Please try again."),
        }
    }
}
