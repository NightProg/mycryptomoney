use std::sync::Arc;
use tokio::net::TcpListener;
use crate::net::node::{Node, MAX_PEERS};

pub async fn start_listener(port: u16, node: Arc<Node>) -> std::io::Result<()> {
    let addr     = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;

    println!("listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((socket, peer_addr)) => {
                if node.peer_count().await >= MAX_PEERS {
                    eprintln!("connection refused (max peers): {}", peer_addr);
                    continue;
                }

                println!("incoming connection: {}", peer_addr);

                let node_clone = node.clone();
                let addr_str   = peer_addr.to_string();

                tokio::spawn(async move {
                    if let Err(e) = node_clone.accept_connection(socket, &addr_str).await {
                        eprintln!("connection error {}: {}", addr_str, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("accept error: {}", e);
            }
        }
    }
}