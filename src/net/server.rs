use std::sync::Arc;
use crate::net::message::{Message, PROTOCOL_VERSION, MAX_BLOCKS_PER_REQUEST};
use crate::net::node::Node;

pub async fn handle_message(node: Arc<Node>, peer_id: u64, message: Message) {
    println!("received message from peer {}: {:?}", peer_id, message);
    match message {

        Message::Version { version, height: peer_height, user_agent, listening_port } => {
            if version != PROTOCOL_VERSION {
                node.send_to_pending(peer_id, Message::Reject {
                    reason: format!("unsupported version {}, expected {}", version, PROTOCOL_VERSION),
                }).await;
                node.remove_pending(peer_id).await;
                return;
            }

            node.send_to_pending(peer_id, Message::VerAck).await;

            node.promote_peer(peer_id, listening_port).await;

            println!("peer {} identified: {} (height {})", peer_id, user_agent, peer_height);

            let our_height = node.chain.lock().await.height();

            if peer_height > our_height {
                println!("IBD: {} blocks behind, requesting from {}", peer_height - our_height, our_height);
                node.send_to_peer(peer_id, Message::GetBlocks {
                    from_height: our_height,
                    max_count:   MAX_BLOCKS_PER_REQUEST,
                }).await;
            }
        }

        Message::VerAck => {
            println!("peer {}: handshake complete", peer_id);
        }

        Message::Ping(nonce) => {
            node.send_to_peer(peer_id, Message::Pong(nonce)).await;
        }

        Message::Pong(nonce) => {
            println!("peer {}: pong (nonce {})", peer_id, nonce);
        }

        Message::GetBlocks { from_height, max_count } => {
            let count  = max_count.min(MAX_BLOCKS_PER_REQUEST) as usize;
            let blocks = node.chain.lock().await.get_blocks_from(from_height, count);

            if blocks.is_empty() { return; }

            println!("sending {} blocks from height {} to peer {}", blocks.len(), from_height, peer_id);
            node.send_to_peer(peer_id, Message::Blocks(blocks)).await;
        }

        Message::Blocks(blocks) => {
            if blocks.is_empty() { return; }

            println!("received {} blocks from peer {}", blocks.len(), peer_id);

            let mut applied = 0usize;

            for block in &blocks {
                if node.has_seen_block(&block.hash).await { continue; }

                let is_valid = {
                    let chain   = node.chain.lock().await;
                    let utxoset = node.utxoset.lock().await;
                    chain.is_block_valid(block, &utxoset)
                };

                if is_valid {
                    node.utxoset.lock().await.apply_block(block);
                    node.chain.lock().await.add_block(block.clone());
                    remove_confirmed_txs(&node, block).await;
                    node.mark_block_seen(block.hash).await;
                    applied += 1;
                } else {
                    eprintln!("invalid block in IBD batch, stopping");
                    break;
                }
            }

            println!("{} blocks applied", applied);

            if blocks.len() == MAX_BLOCKS_PER_REQUEST as usize {
                let our_height = node.chain.lock().await.height();
                node.send_to_peer(peer_id, Message::GetBlocks {
                    from_height: our_height,
                    max_count:   MAX_BLOCKS_PER_REQUEST,
                }).await;
            }
        }

        Message::NewBlock(block) => {
            println!("received new block #{} from peer {}", block.header.height, peer_id);
            if node.has_seen_block(&block.hash).await { return; }

            let is_valid = {
                let chain   = node.chain.lock().await;
                let utxoset = node.utxoset.lock().await;
                chain.is_block_valid(&block, &utxoset)
            };

            if !is_valid {
                eprintln!("invalid block from peer {}", peer_id);
                node.send_to_peer(peer_id, Message::Reject {
                    reason: "invalid block".to_string(),
                }).await;
                return;
            }

            node.utxoset.lock().await.apply_block(&block);
            node.chain.lock().await.add_block(block.clone());
            remove_confirmed_txs(&node, &block).await;
            node.mark_block_seen(block.hash).await;

            println!("new block #{} accepted (peer {})", node.chain.lock().await.height(), peer_id);

            node.broadcast_except(Message::NewBlock(block), peer_id).await;
        }

        Message::NewTx(tx) => {
            println!("received new transaction from peer {}", peer_id);
            if node.has_seen_tx(&tx.hash()).await { return; }

            let is_valid = node.utxoset.lock().await.verify_tx(&tx);

            if !is_valid {
                eprintln!("invalid tx from peer {}", peer_id);
                node.send_to_peer(peer_id, Message::Reject {
                    reason: "invalid transaction".to_string(),
                }).await;
                return;
            }

            node.mempool.lock().await.push(tx.clone());
            node.mark_tx_seen(tx.hash()).await;

            println!("tx {:?} added to mempool", &tx.hash()[..4]);

            node.broadcast_except(Message::NewTx(tx), peer_id).await;
        }

        Message::GetPeers => {
            let addrs = node.peer_addrs().await;
            node.send_to_peer(peer_id, Message::Peers(addrs)).await;
        }

        Message::Peers(addrs) => {
            for addr in addrs {
                println!("peer {} knows about {}", peer_id, addr);
                let local_addrs = format!("127.0.0.1:{}", node.listening_port);
                if addr == local_addrs {
                    continue;
                }
                if !node.is_connected_or_pending(&addr).await && node.peer_count().await < crate::net::node::MAX_PEERS {
                    let node_clone = node.clone();
                    let addr_clone = addr.clone();
                    tokio::spawn(async move {
                        if let Err(e) = node_clone.connect_to_peer(&addr_clone).await {
                            eprintln!("failed to connect to {}: {}", addr_clone, e);
                        }
                    });
                }
            }
        }

        Message::Reject { reason } => {
            eprintln!("peer {} rejected our message: {}", peer_id, reason);
        }
    }
}

async fn remove_confirmed_txs(node: &Arc<Node>, block: &crate::chain::Block) {
    let confirmed: std::collections::HashSet<[u8; 32]> = block
        .transactions
        .iter()
        .map(|tx| tx.hash())
        .collect();

    node.mempool
        .lock()
        .await
        .retain(|tx| !confirmed.contains(&tx.hash()));
}