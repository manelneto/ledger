use std::env;
use tonic::Request;

use kademlia_proto::kademlia_client::KademliaClient;
use kademlia_proto::ShutdownRequest;

mod kademlia_proto {
    tonic::include_proto!("kademlia");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: cargo run --bin shutdown <PORT_1> ... <PORT_N>");
        std::process::exit(1);
    }

    for port in args {
        let addr = format!("http://127.0.0.1:{}", port);
        println!("Attempting graceful shutdown on {}", addr);

        match KademliaClient::connect(addr.clone()).await {
            Ok(mut client) => {
                println!("Connected to {}", addr);
                let request = Request::new(ShutdownRequest {});
                match client.shutdown(request).await {
                    Ok(_) => println!("Shutdown successful on port {}", port),
                    Err(e) => eprintln!("Failed to shutdown on port {}: {}", port, e),
                }
            }
            Err(e) => eprintln!("Could not connect to {}: {}", addr, e),
        }
    }
}
