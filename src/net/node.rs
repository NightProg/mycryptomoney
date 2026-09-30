use std::collections::HashSet;
use std::sync::{atomic, Arc};
use std::sync::atomic::AtomicU64;
use rkyv::rancor::Error;
use sha2::Digest;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};

use crate::chain::{Blockchain, Transaction};
use crate::crypto::keypair::PublicKey;
use crate::net::reader::MessageReader;
use crate::net::server::handle_message;
use crate::utxo_set::UTXOSet;
use crate::wallet::Wallet;
use super::message::{Message, PROTOCOL_VERSION};

pub const MAX_PEERS: usize = 16;

pub static NEXT_PEER_ID: AtomicU64 = AtomicU64::new(1);

pub struct Node {
    pub chain:          Arc<Mutex<Blockchain>>,
    pub utxoset:        Arc<Mutex<UTXOSet>>,
    pub mempool:        Arc<Mutex<Vec<Transaction>>>,
    //pub wallet:         Arc<Mutex<Wallet>>,
    pub seen_blocks:    Arc<Mutex<HashSet<[u8; 32]>>>,
    pub seen_txs:       Arc<Mutex<HashSet<[u8; 32]>>>,
    pub peers:          Arc<Mutex<Vec<PeerConnection>>>,
    pub pending_peers:  Arc<Mutex<Vec<PendingPeer>>>,
    pub incoming_tx:    mpsc::Sender<(u64, NetEvent)>,
    pub listening_port: u16,
}

#[derive(Clone)]
pub struct PeerConnection {
    pub sender: mpsc::Sender<Message>,
    pub addr:   String,
    pub id:     u64,
}

/// A peer whose handshake is not yet complete — not in the peers list.
#[derive(Clone)]
pub struct PendingPeer {
    pub sender: mpsc::Sender<Message>,
    pub id:     u64,
    pub ip:     String,
}

/// Events sent by peer threads to the main node loop.
pub enum NetEvent {
    Message(Message),
    /// Peer disconnected before handshake completed.
    PendingDisconnected,
}

impl Node {
    pub fn new(port: u16) -> (Arc<Self>, mpsc::Receiver<(u64, NetEvent)>) {
        let (tx, rx) = mpsc::channel(1000);

        let node = Arc::new(Node {
            chain:          Arc::new(Mutex::new(Blockchain::new())),
            utxoset:        Arc::new(Mutex::new(UTXOSet::new())),
            mempool:        Arc::new(Mutex::new(vec![])),
            //wallet:         Arc::new(Mutex::new(Wallet::new())),
            seen_blocks:    Arc::new(Mutex::new(HashSet::new())),
            seen_txs:       Arc::new(Mutex::new(HashSet::new())),
            peers:          Arc::new(Mutex::new(vec![])),
            pending_peers:  Arc::new(Mutex::new(vec![])),
            incoming_tx:    tx,
            listening_port: port,
        });

        (node, rx)
    }

    /// Accept an inbound TCP connection.
    /// The peer is placed in pending until the handshake completes.
    pub async fn accept_connection(
        self: &Arc<Self>,
        stream: TcpStream,
        peer_ip: &str,
    ) -> tokio::io::Result<u64> {
        if self.peer_count().await >= MAX_PEERS {
            return Err(tokio::io::Error::new(
                tokio::io::ErrorKind::ConnectionRefused,
                "max peers reached",
            ));
        }

        let ip      = peer_ip.split(':').next().unwrap_or(peer_ip).to_string();
        let peer_id = self.register_pending(stream, ip).await;

        let height = self.chain.lock().await.height();
        self.send_to_pending(peer_id, Message::Version {
            version:        PROTOCOL_VERSION,
            height,
            user_agent:     "mycoin/0.1.0".to_string(),
            listening_port: self.listening_port,
        }).await;

        Ok(peer_id)
    }

    pub async fn is_connected_or_pending(&self, addr: &str) -> bool {
        let peers = self.peers.lock().await;
        if peers.iter().any(|p| p.addr == addr) {
            return true;
        }

        let pending = self.pending_peers.lock().await;
        pending.iter().any(|p| p.ip == addr.split(':').next().unwrap_or(addr))
    }

    /// Initiate an outbound TCP connection.
    /// The peer is placed in pending until the handshake completes.
    pub async fn connect_to_peer(
        self: &Arc<Self>,
        addr: &str,
    ) -> tokio::io::Result<u64> {
        if self.peer_count().await >= MAX_PEERS {
            return Err(tokio::io::Error::new(
                tokio::io::ErrorKind::ConnectionRefused,
                "max peers reached",
            ));
        }

        let stream  = TcpStream::connect(addr).await?;
        let ip      = addr.split(':').next().unwrap_or(addr).to_string();
        let peer_id = self.register_pending(stream, ip).await;

        let height = self.chain.lock().await.height();
        self.send_to_pending(peer_id, Message::Version {
            version:        PROTOCOL_VERSION,
            height,
            user_agent:     "mycoin/0.1.0".to_string(),
            listening_port: self.listening_port,
        }).await;

        Ok(peer_id)
    }


    pub async fn mine_block(self: &Arc<Self>, wallet: &mut Wallet) {
        let mut utxoset = self.utxoset.lock().await;
        let mut mempool = self.mempool.lock().await;
        let mut chain   = self.chain.lock().await;
        let block = wallet.mine_block(&mempool, &mut chain, &mut utxoset);

        if let Some(block) = block {
            wallet.scan_utxos(&block);
            drop(chain);
            drop(utxoset);
            self.mark_block_seen(block.hash).await;
            self.broadcast(Message::NewBlock(block)).await;
            // clear mempool of transactions that were included in the block
            mempool.clear()
        } else {
            eprintln!("no transactions to mine");
        }

    }

    /// Spawn the read/write loop for a new connection and add it to pending.
    /// The peer stays pending until `promote_peer` is called after handshake.
    async fn register_pending(
        self: &Arc<Self>,
        stream: TcpStream,
        ip: String,
    ) -> u64 {
        let (mut reader, mut writer) = stream.into_split();
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<Message>(100);

        let peer_id    = NEXT_PEER_ID.fetch_add(1, atomic::Ordering::Relaxed);
        let node_tx    = self.incoming_tx.clone();
        let node_clone = self.clone();



        tokio::spawn(async move {
            let mut reader = MessageReader::new(reader);
            loop {
                tokio::select! {
                    result = reader.next_message() => {
                        match result {
                            Ok(msg) => {
                                let _ = node_tx.send((peer_id, NetEvent::Message(msg))).await;
                            }
                            Err(e) => {
                                eprintln!("peer {} disconnected: {}", peer_id, e);
                                let _ = node_tx.send((peer_id, NetEvent::PendingDisconnected)).await;
                                break;
                            }
                        }
                    }
                    Some(msg) = outbound_rx.recv() => {
                        if let Err(e) = send_message(&mut writer, &msg).await {
                            eprintln!("send error peer {}: {}", peer_id, e);
                            break;
                        }
                    }
                }
            }

            node_clone.remove_pending(peer_id).await;
            node_clone.disconnect_peer(peer_id).await;
        });

        self.pending_peers.lock().await.push(PendingPeer {
            sender: outbound_tx,
            id:     peer_id,
            ip,
        });

        peer_id
    }

    /// Called after a successful handshake.
    /// Moves the peer from pending to the active peers list using the
    /// listening_port announced in their Version message.
    pub async fn promote_peer(&self, peer_id: u64, listening_port: u16) {
        let pending = {
            let mut pending = self.pending_peers.lock().await;
            let pos = pending.iter().position(|p| p.id == peer_id);
            match pos {
                Some(i) => pending.remove(i),
                None    => return,
            }
        };

        let addr = format!("{}:{}", pending.ip, listening_port);

        self.peers.lock().await.push(PeerConnection {
            sender: pending.sender,
            addr:   addr.clone(),
            id:     peer_id,
        });

        println!("peer {} promoted after handshake ({})", peer_id, addr);
    }

    pub async fn remove_pending(&self, peer_id: u64) {
        self.pending_peers.lock().await.retain(|p| p.id != peer_id);
    }

    pub async fn send_to_pending(&self, peer_id: u64, msg: Message) {
        let pending = self.pending_peers.lock().await;
        if let Some(p) = pending.iter().find(|p| p.id == peer_id) {
            let _ = p.sender.send(msg).await;
        }
    }

    pub async fn send_to_peer(&self, peer_id: u64, msg: Message) {
        let peers = self.peers.lock().await;
        if let Some(peer) = peers.iter().find(|p| p.id == peer_id) {
            let _ = peer.sender.send(msg).await;
        }
    }

    pub async fn broadcast(&self, msg: Message) {
        let peers = self.peers.lock().await;
        for peer in peers.iter() {
            let _ = peer.sender.send(msg.clone()).await;
        }
    }

    pub async fn broadcast_except(&self, msg: Message, except_id: u64) {
        let peers = self.peers.lock().await;
        for peer in peers.iter().filter(|p| p.id != except_id) {
            let _ = peer.sender.send(msg.clone()).await;
        }
    }

    pub async fn disconnect_peer(&self, peer_id: u64) {
        self.peers.lock().await.retain(|p| p.id != peer_id);
    }

    pub async fn has_seen_block(&self, hash: &[u8; 32]) -> bool {
        self.seen_blocks.lock().await.contains(hash)
    }

    pub async fn mark_block_seen(&self, hash: [u8; 32]) {
        self.seen_blocks.lock().await.insert(hash);
    }

    pub async fn has_seen_tx(&self, txid: &[u8; 32]) -> bool {
        self.seen_txs.lock().await.contains(txid)
    }

    pub async fn mark_tx_seen(&self, txid: [u8; 32]) {
        self.seen_txs.lock().await.insert(txid);
    }

    pub async fn peer_count(&self) -> usize {
        self.peers.lock().await.len()
    }

    pub async fn peer_addrs(&self) -> Vec<String> {
        self.peers.lock().await.iter().map(|p| p.addr.clone()).collect()
    }

    pub async fn send_coins(&self, recipient: PublicKey, amount: u64, wallet: &mut Wallet) {
        let mut utxoset = self.utxoset.lock().await;

        if let Some(tx) = wallet.give_to(recipient, amount, &mut utxoset) {
            drop(utxoset);
            self.mempool.lock().await.push(tx.clone());
            self.broadcast(Message::NewTx(tx)).await;
        } else {
            println!("insufficient balance to send {} coins", amount);
            eprintln!("insufficient balance");
        }
    }

    pub async fn run(self: Arc<Self>, mut rx: mpsc::Receiver<(u64, NetEvent)>) {
        while let Some((peer_id, event)) = rx.recv().await {
            match event {
                NetEvent::Message(msg) => {
                    let node = self.clone();
                    tokio::spawn(async move {
                        handle_message(node, peer_id, msg).await;
                    });
                }
                NetEvent::PendingDisconnected => {
                    self.remove_pending(peer_id).await;
                }
            }
        }
    }
}

pub async fn send_message(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    message: &Message,
) -> tokio::io::Result<()> {
    let bytes = rkyv::to_bytes::<Error>(message)
        .map_err(|e| tokio::io::Error::new(tokio::io::ErrorKind::InvalidData, e.to_string()))?;

    let sha    = sha2::Sha256::digest(&bytes);
    let length = (bytes.len() as u64 + 32).to_be_bytes();

    writer.write_all(&length).await?;
    writer.write_all(&sha).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;

    Ok(())
}

pub async fn receive_message(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
) -> tokio::io::Result<Message> {
    let mut length_buf = [0u8; 8];
    reader.read_exact(&mut length_buf).await?;
    let length = u64::from_be_bytes(length_buf) as usize;

    if length < 32 || length > 10_000_000 {
        return Err(tokio::io::Error::new(
            tokio::io::ErrorKind::InvalidData,
            format!("invalid length: {}", length),
        ));
    }

    let mut sha_buf = [0u8; 32];
    reader.read_exact(&mut sha_buf).await?;

    let mut payload = vec![0u8; length - 32];
    reader.read_exact(&mut payload).await?;

    let computed = sha2::Sha256::digest(&payload);
    if computed[..] != sha_buf {
        return Err(tokio::io::Error::new(
            tokio::io::ErrorKind::InvalidData,
            "checksum mismatch",
        ));
    }

    rkyv::from_bytes::<Message, Error>(&payload)
        .map_err(|e| tokio::io::Error::new(tokio::io::ErrorKind::InvalidData, e.to_string()))
}