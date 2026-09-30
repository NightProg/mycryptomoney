use crate::chain::Block;
use crate::types::tx::Transaction;

#[derive(Debug, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize, serde::Deserialize)]
pub enum Message {
    Version {
        version:        u32,
        height:         u64,
        user_agent:     String,
        listening_port: u16,
    },
    VerAck,
    Ping(u64),
    Pong(u64),
    GetBlocks {
        from_height: u64,
        max_count:   u32,
    },
    Blocks(Vec<Block>),
    NewTx(Transaction),
    NewBlock(Block),
    GetPeers,
    Peers(Vec<String>),
    Reject {
        reason: String,
    },
}

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_BLOCKS_PER_REQUEST: u32 = 500;