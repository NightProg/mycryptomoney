use std::io::{Read, Write};
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit};
use argon2::password_hash::phc::SaltString;
use argon2::PasswordHasher;
use base64::Engine;
use base64::engine::GeneralPurpose;
use base64::prelude::BASE64_STANDARD;
use rkyv::rancor::Error;
use crate::chain::{Block, Blockchain};
use crate::crypto::keypair::{generate_keypair, PrivateKey, PublicKey};
use crate::types::tx::{Transaction, TxInput, TxOutput};
use crate::types::utxo::{UTXOKey, UTXOValue};
use crate::utxo_set::UTXOSet;

fn derive_key_from_password(password: &str, salt: &SaltString) -> [u8; 32] {
    let argon2 = argon2::Argon2::default();
    let mut key = [0u8; 32];
    argon2.hash_password_into(password.as_bytes(), salt.as_bytes(), &mut key).unwrap();
    key
}

#[derive(Debug, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize, serde::Deserialize)]
pub struct Wallet {
    pub private_key: PrivateKey,
    pub public_key: PublicKey,
    pub amount: u64,
    pub mempool: Vec<Transaction>,
}

impl Wallet {

    pub fn unlock(filename: &str, password: &str) -> Self {
        let mut file = std::fs::File::open(filename).unwrap();
        let mut salt_bytes = [0u8; 16];
        file.read_exact(&mut salt_bytes).unwrap();
        let salt = SaltString::from_b64(&BASE64_STANDARD.encode(salt_bytes)).unwrap();
        let mut nonce_bytes = [0u8; 12];
        file.read_exact(&mut nonce_bytes).unwrap();
        let nonce = aes_gcm::Nonce::from_slice(&nonce_bytes);
        let key = derive_key_from_password(password, &salt);
        let cipher = Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(&key));
        let mut ciphertext = Vec::new();
        file.read_to_end(&mut ciphertext).unwrap();
        let decrypted = cipher.decrypt(nonce, ciphertext.as_ref()).unwrap();
        rkyv::from_bytes::<Wallet, Error>(&decrypted).unwrap()
    }
    pub fn new() -> Self {
        let (private_key, public_key) = generate_keypair();
        Wallet {
            private_key,
            public_key,
            amount: 0,
            mempool: vec![],
        }
    }



    pub fn from_private_key(private_key: PrivateKey) -> Self {
        let public_key = PublicKey::from_private(&private_key);
        Wallet {
            private_key,
            public_key,
            amount: 0,
            //my_utxos: vec![],
            mempool: vec![],
        }
    }

    pub fn scan_utxos(&mut self, block: &Block) {
        /*for tx in block.transactions.iter() {
            for (index, output) in tx.outputs.iter().enumerate() {
                if output.recipient == self.public_key {
                    let utxo_key = UTXOKey { txid: tx.hash(), vout: index };
                    self.my_utxos.push((utxo_key, output.amount));
                    self.amount += output.amount;
                }
            }
        }*/
    }

    pub fn scan_blockchain(&mut self, blockchain: &Blockchain) {
        for block in &blockchain.blocks {
            self.scan_utxos(block);
        }
    }

    pub fn mine_block(&mut self, transaction: &[Transaction], blockchain: &mut Blockchain, utxoset: &mut UTXOSet) -> Option<Block> {
        println!("Mining block with {} transactions", transaction.len());
        blockchain.mine_block(transaction.to_vec(), (self.private_key.clone(), self.public_key.clone()), utxoset)
    }

    pub fn get_balance(&self, utxoset: &UTXOSet) -> u64 {
        utxoset.get_balance(&self.public_key) 
    }


    pub fn get_address(&self) -> String {
        // For simplicity, we will just return the public key as a hex string
        hex::encode(self.public_key.to_bytes())
    }



    pub fn give_to(&mut self, recipient: PublicKey, amount: u64, utxoset: &UTXOSet) -> Option<Transaction> {
        println!("Creating transaction to send {} to {}", amount, hex::encode(recipient.to_bytes()));
        let mut inputs = Vec::new();
        let mut total_input = 0;
        for (utxo_key, utxo_amount) in &utxoset.get_utxos(&self.public_key) {
            if total_input >= amount {
                break;
            }
            inputs.push(TxInput {
                key: utxo_key.clone(),
                public_key: self.public_key.clone(),
            });
            total_input += utxo_amount.amount;
        }

        if total_input < amount {
            panic!("Not enough balance to send, have {}, need {}", total_input, amount);
        }

        println!("Total input amount: {}", total_input);

        let outputs = vec![
            TxOutput { amount, recipient },
            TxOutput { amount: total_input - amount, recipient: self.public_key.clone() },
        ];

        let mut tx = Transaction::new(inputs, outputs, false);
        tx.sign(&[(self.private_key.clone(), self.public_key.clone())]);
        println!("created transaction with hash: {}", hex::encode(tx.hash()));
        
        if utxoset.verify_tx(&tx) {
            Some(tx)
        } else {
            println!("Transaction verification failed, not adding to mempool");
            None
        }
    }

    pub fn broadcast(&mut self, blockchain: &mut Blockchain, utxoset: &mut UTXOSet) {
        let txs = self.mempool.drain(..).collect();
        blockchain.mine_block(txs, (self.private_key.clone(), self.public_key.clone()), utxoset);
    }

    pub fn save_to_file(&self, filename: &str, password: &str) {
        let salt = SaltString::generate();
        let argon2 = argon2::Argon2::default();
        let mut key = [0u8; 32];
        argon2.hash_password_into(password.as_bytes(), salt.as_bytes(), &mut key).unwrap();

        let cipher = Aes256Gcm::new(
            aes_gcm::Key::<Aes256Gcm>::from_slice(&key),
        );

        let mut nonce = aes_gcm::Nonce::from_slice(&[0u8; 12]).clone();
        // fill nonce with random bytes
        rand::fill(nonce.as_mut_slice());

        let serialized = rkyv::to_bytes::<Error>(self).unwrap();
        let ciphertext = cipher.encrypt(&nonce, serialized.as_ref()).unwrap();
        // Save salt and ciphertext to file
        let mut file = std::fs::File::create(filename).unwrap();
        file.write_all(salt.as_bytes()).unwrap();
        file.write_all(nonce.as_slice()).unwrap();
        file.write_all(&ciphertext).unwrap();
        file.flush().unwrap();

    }
}