use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use sha2::Digest;
use crate::chain::Block;
use crate::crypto::keypair::PublicKey;
use crate::crypto::sign::verify;
use crate::types::tx::Transaction;
use crate::types::utxo::{UTXOKey, UTXOValue};

fn has_unique_elements<T>(iter: T) -> bool
where
    T: IntoIterator,
    T::Item: Eq + Hash,
{
    let mut uniq = HashSet::new();
    iter.into_iter().all(move |x| uniq.insert(x))
}

pub struct UTXOSet {
    pub utxos: HashMap<UTXOKey, UTXOValue>
}

impl UTXOSet {
    pub fn new() -> Self {
        UTXOSet {
            utxos: HashMap::new(),
        }
    }
    
    pub fn get_utxos(&self, public_key: &PublicKey) -> HashMap<UTXOKey, UTXOValue> {
        self.utxos.iter().filter(|(_, v)| &v.owner == public_key).map(|(k, v)| (k.clone(), v.clone())).collect()
    }
    pub fn get_balance(&self, public_key: &PublicKey) -> u64 {
        let mut balance = 0;
        for value in self.utxos.values() {
            if &value.owner == public_key {
                balance += value.amount;
            }
        }
        balance
    }

    pub fn calculate_fee(&self, tx: &Transaction) -> u64 {
        let out_sum = tx.outputs.iter().map(|o| o.amount).sum::<u64>();
        let in_sum = tx.inputs.iter().map(|input| self.utxos[&input.key].amount).sum::<u64>();
        if in_sum > out_sum {
            in_sum - out_sum
        } else {
            0
        }
    }

    pub fn verify_tx(&self, tx: &Transaction) -> bool {
        println!("Verifying transaction with {} inputs and {} outputs", tx.inputs.len(), tx.outputs.len());
        println!("tx is coinbase: {}", tx.is_coinbase);
        if tx.is_coinbase {
            return true;
        }

        let out_sum = tx.outputs.iter().map(|o| o.amount).sum::<u64>();
        let mut in_sum = 0;

        for (idx, input) in tx.inputs.iter().enumerate() {
            if !self.utxos.contains_key(&input.key) {
                println!("UTXO not found for input: {:?}", input.key);
                println!("UTXO Content: {:?}", self.utxos);
                return false;
            }


            if !verify(
                &tx.to_bytes(),
                &tx.signatures[idx],
                input.public_key.clone(),
            ) {
                println!("Signature verification failed for input: {:?}", input.key);
                return false;
            }
            in_sum += self.utxos[&input.key].amount;
        }
        if !has_unique_elements(tx.inputs.iter().map(|i| &i.key)) {
            println!("UTXO does not have unique elements");
            return false;
        }
        if in_sum < out_sum {
            println!("UTXO input sum {} is less than output sum {}", in_sum, out_sum);
            return false;
        }
        true
    }

    pub fn apply_tx(&mut self, tx: &Transaction, block_height: u64) -> Vec<UTXOKey> {
        if !tx.is_coinbase {
            for input in &tx.inputs {
                self.utxos.remove(&input.key);
            }
        }

        let mut new_utxos = Vec::new();

        for (index, output) in tx.outputs.iter().enumerate() {
            let key = UTXOKey {
                txid: if tx.is_coinbase {
                    tx.hash_coinbase(block_height)
                } else {
                    tx.hash()
                },
                vout: index,
            };
            let value = UTXOValue {
                amount: output.amount,
                owner: output.recipient.clone(),
                script_pubkey: vec![],
            };
            self.utxos.insert(key.clone(), value);
            new_utxos.push(key);
        }

        new_utxos.sort_by_key(|k| k.vout);
        new_utxos
    }

    pub fn apply_block(&mut self, block: &Block) {
        for tx in block.transactions.iter() {
            self.apply_tx(tx, block.header.height);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::crypto::keypair::{generate_keypair, PublicKey};
    use crate::types::tx::*;
    use super::*;

    #[test]
    fn test_validate_and_apply_tx() {
        let (private_key, public_key) = generate_keypair();
        let mut utxoset = UTXOSet::new();
        let mut tx = Transaction::new(
            vec![],
            vec![TxOutput {
                amount: 50,
                recipient: public_key.clone(),
            }],
            true,
        );
        tx.sign(&[(private_key.clone(), public_key.clone())]);

        assert!(utxoset.verify_tx(&tx));
        utxoset.apply_tx(&tx, 0);

        let mut double_spend_tx = Transaction::new(
            vec![TxInput {
                key: UTXOKey {
                    txid: tx.hash(),
                    vout: 0,
                },
                public_key: public_key.clone(),
            }],
            vec![TxOutput {
                amount: 50,
                recipient: public_key.clone(),
            }],
            false,
        );
        double_spend_tx.sign(&[(private_key.clone(), public_key.clone())]);
        assert!(utxoset.verify_tx(&double_spend_tx));
        utxoset.apply_tx(&double_spend_tx, 0);

        assert!(!utxoset.verify_tx(&double_spend_tx));

    }
}