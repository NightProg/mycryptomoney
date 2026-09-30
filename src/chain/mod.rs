mod miner;

use std::hash::Hash;
use rug::Integer;
use sha2::Digest;
pub(crate) use crate::types::tx::Transaction;
use crate::utxo_set::UTXOSet;

pub const DIFFICULTY: u64 = 2; // We use a very low difficulty for testing purposes
pub const INITIAL_REWARD: u64 = 50 * 100_000_000;
pub const HALVING_INTERVAL: u64 = 210_000;

fn compute_subsidy(height: u64) -> u64 {
    let halvings = height / HALVING_INTERVAL;

    if halvings >= 64 {
        return 0;
    }

    INITIAL_REWARD >> halvings
}

#[derive(Debug, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize, serde::Deserialize)]
pub struct BlockHeader {
    pub prev_hash: [u8; 32],
    pub merkle_root: [u8; 32],
    pub timestamp: u64,
    pub nonce: u64,
    pub height: u64,
    pub difficulty: u64,
}


#[derive(Debug, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub hash: [u8; 32],
}

impl Block {
    pub fn compute_merkle_root(&self) -> [u8; 32] {
        if self.transactions.is_empty() {
            return [0u8; 32];
        }
        let mut hashes: Vec<[u8; 32]> = self.transactions.iter().map(|tx| tx.hash()).collect();
        while hashes.len() > 1 {
            if hashes.len() % 2 != 0 {
                hashes.push(hashes.last().unwrap().clone());
            }
            let mut new_hashes = Vec::new();
            for i in (0..hashes.len()).step_by(2) {
                let mut hasher = sha2::Sha256::new();
                hasher.update(&hashes[i]);
                hasher.update(&hashes[i + 1]);
                let result = hasher.finalize();
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&result);
                new_hashes.push(hash);
            }
            hashes = new_hashes;
        }
        hashes[0]
    }
}

pub struct Blockchain {
    pub blocks: Vec<Block>,
}

impl Blockchain {
    pub fn new() -> Self {
        Blockchain { blocks: Vec::new() }
    }

    pub fn is_block_valid(&self, block: &Block, utxo_set: &UTXOSet) -> bool {
        if self.blocks.is_empty() {
            return block.header.prev_hash == [0u8; 32];
        }
        let last_block = self.blocks.last().unwrap();
        if block.header.prev_hash != last_block.hash {
            return false;
        }
        if block.header.merkle_root != block.compute_merkle_root() {
            return false;
        }
        if block.transactions.len() == 0 {
            return false;
        }

        for tx in &block.transactions {
            if !utxo_set.verify_tx(tx) {
                return false;
            }
        }
        // check if a transaction is not used more than once in the block
        let mut seen_inputs = std::collections::HashSet::new();
        for tx in &block.transactions {
            for input in &tx.inputs {
                if !seen_inputs.insert((input.key.txid, input.key.vout)) {
                    return false;
                }
            }
        }
        let first_tx = &block.transactions[0];
        if !first_tx.is_coinbase {
            return false;
        }

        let subsidy = compute_subsidy(block.header.height);
        let fee = block.transactions.iter().map(|tx| utxo_set.calculate_fee(tx)).sum::<u64>();
        let out_sum = first_tx.outputs.iter().map(|o| o.amount).sum::<u64>();

        let pow = block.hash.iter().take(block.header.difficulty as usize).all(|&b| b == 0);

        pow && out_sum <= subsidy + fee
    }

    pub fn compute_block_hash(&self, block: &Block) -> [u8; 32] {
        let mut hasher = sha2::Sha256::new();
        hasher.update(&block.header.prev_hash);
        hasher.update(&block.header.merkle_root);
        hasher.update(&block.header.timestamp.to_le_bytes());
        hasher.update(&block.header.nonce.to_le_bytes());
        hasher.update(&block.header.height.to_le_bytes());
        hasher.update(&block.header.difficulty.to_le_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    pub fn is_valid_proof(&self, block: &Block) -> bool {
        let block_hash = self.compute_block_hash(block);
        block_hash.iter().take(block.header.difficulty as usize).all(|&b| b == 0)
    }

    pub fn get_blocks_from(&self, height: u64, count: usize) -> Vec<Block> {
        self.blocks.iter()
            .filter(|b| b.header.height >= height)
            .take(count)
            .cloned()
            .collect()
    }

    pub fn add_block(&mut self, block: Block) {
        self.blocks.push(block);
    }
    
    pub fn height(&self) -> u64 {
        self.blocks.len() as u64
    }


}