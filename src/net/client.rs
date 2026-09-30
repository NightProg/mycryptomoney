use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use crate::net::message::Message;
use crate::net::node::Node;

pub async fn bootstrap(node: Arc<Node>, addr: &str) {
    println!("bootstrapping -> {}", addr);

    match node.connect_to_peer(addr).await {
        Ok(peer_id) => {
            node.send_to_peer(peer_id, Message::GetPeers).await;
        }
        Err(e) => {
            eprintln!("failed to connect to bootstrap {}: {}", addr, e);
        }
    }
}

pub async fn bootstrap_many(node: Arc<Node>, addrs: &[&str]) {
    let mut handles = vec![];

    for addr in addrs {
        let node_clone = node.clone();
        let addr_owned = addr.to_string();
        handles.push(tokio::spawn(async move {
            bootstrap(node_clone, &addr_owned).await;
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }
}

pub async fn start_ping_loop(node: Arc<Node>) {
    let mut interval = time::interval(Duration::from_secs(30));
    let mut nonce: u64 = 0;

    loop {
        interval.tick().await;
        nonce = nonce.wrapping_add(1);

        if node.peer_count().await > 0 {
            node.broadcast(Message::Ping(nonce)).await;
        }
    }
}

pub async fn start_peer_discovery_loop(node: Arc<Node>, min_peers: usize) {
    let mut interval = time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await;

        if node.peer_count().await < min_peers {
            node.broadcast(Message::GetPeers).await;
        }
    }
}