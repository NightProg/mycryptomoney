use std::time::Duration;
use mycryptomoney::chain::Blockchain;
use mycryptomoney::net::client::{start_peer_discovery_loop, start_ping_loop};
use mycryptomoney::net::listener::start_listener;
use mycryptomoney::net::message::Message;
use mycryptomoney::net::node::Node;
use mycryptomoney::utxo_set::UTXOSet;
use mycryptomoney::wallet::Wallet;


#[tokio::main]
async fn main() {
    let port = 3000 + rand::random::<u16>();
    println!("Starting server on localhost:{}", port);
    let (node, mut rx) = Node::new(port);

    let node_clone = node.clone();

    tokio::spawn(async move {
        start_listener(port, node_clone).await.unwrap();
    });


    let peer_id = node.clone().connect_to_peer("localhost:56637").await.expect("Échec de connexion au pair");
    let mut x = 0;
    let node_clone = node.clone();
    tokio::spawn(async move {
        node_clone.run(rx).await;
    });

    let node_clone = node.clone();

    tokio::spawn(async move {
        start_peer_discovery_loop(node_clone, 3).await;
    });

    let node_clone = node.clone();
    
    loop {

    }
}