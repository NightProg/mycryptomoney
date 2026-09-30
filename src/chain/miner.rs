use std::time::{Instant, SystemTime, UNIX_EPOCH};
use crate::chain::{compute_subsidy, Block, BlockHeader, Blockchain, DIFFICULTY};
use crate::crypto::keypair::{PrivateKey, PublicKey};
use crate::types::tx::{Transaction, TxOutput};
use crate::utxo_set::UTXOSet;

impl Blockchain {
    pub fn mine_block(&mut self, transactions: Vec<Transaction>, keypair: (PrivateKey, PublicKey), utxoset: &mut UTXOSet) -> Option<Block> {
        let prev_hash = if let Some(last_block) = self.blocks.last() {
            last_block.hash
        } else {
            [0u8; 32]
        };

        let height = self.blocks.len() as u64;
        let difficulty = DIFFICULTY;

        let mut block = Block {
            header: BlockHeader {
                prev_hash,
                merkle_root: [0u8; 32],
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                nonce: 0,
                height,
                difficulty,
            },
            transactions,
            hash: [0u8; 32],
        };

        let instant = Instant::now();


        let subsidy = compute_subsidy(height);
        let fee = block.transactions.iter().map(|tx| utxoset.calculate_fee(tx)).sum::<u64>();
        // Add coinbase transaction
        let mut coinbase_tx = Transaction::new(
            vec![],
            vec![TxOutput {
                amount: subsidy + fee,
                recipient: keypair.1.clone(),
            }],
            true,
        );
        coinbase_tx.sign(&[keypair]);
        block.transactions.insert(0, coinbase_tx.clone());
        block.header.merkle_root = block.compute_merkle_root();
        while !self.is_valid_proof(&block) {
            block.header.nonce += 1;

            if block.header.nonce % 100000 == 0 {
                println!("Mining... Nonce: {}, Time: {}s", block.header.nonce, instant.elapsed().as_secs());
            }
        }
        block.hash = self.compute_block_hash(&block);


        if self.is_block_valid(&block, utxoset) {
            self.blocks.push(block.clone());
            for tx in &self.blocks.last().unwrap().transactions {
                utxoset.apply_tx(tx, block.header.height);
            }
            
            Some(block)
        } else {
            None
        }
    }


}

#[cfg(test)]
mod tests {
    use crate::crypto::keypair::{generate_keypair, PublicKey};
    use crate::types::tx::*;
    use crate::utxo_set::UTXOSet;
    use super::*;

    #[test]
    fn test_mine_block() {
        let (private_key, public_key) = generate_keypair();
        let mut blockchain = Blockchain::new();
        let mut utxoset = UTXOSet::new();

        let tx1 = Transaction::new(
            vec![],
            vec![TxOutput {
                amount: 50,
                recipient: public_key.clone(),
            }],
            true,
        );
        let tx2 = Transaction::new(
            vec![],
            vec![TxOutput {
                amount: 30,
                recipient: public_key.clone(),
            }],
            true,
        );

        blockchain.mine_block(vec![tx1, tx2], (private_key, public_key), &mut utxoset);
        assert_eq!(blockchain.blocks.len(), 1);
        assert_eq!(blockchain.blocks[0].transactions.len(), 3); // 2 transactions + 1 coinbase
    }
}