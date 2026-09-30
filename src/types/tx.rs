use std::collections::HashSet;
use rug::Integer;
use sha2::Digest;
use crate::crypto::keypair::{PrivateKey, PublicKey};
use crate::crypto::sign::{sign, Signature};
use crate::types::utxo::UTXOKey;


#[derive(Debug, Clone, Hash, PartialEq, Eq, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize, serde::Deserialize)]
pub struct TxInput {
    pub key: UTXOKey,
    pub public_key: PublicKey,
}

#[derive(Debug, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize, serde::Deserialize)]
pub struct TxOutput {
    pub amount: u64,
    pub recipient: PublicKey,
}


#[derive(Debug, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize, serde::Deserialize)]
pub struct Transaction {
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub signatures: Vec<Signature>,
    pub is_coinbase: bool,
}

impl Transaction {
    pub fn new(inputs: Vec<TxInput>, outputs: Vec<TxOutput>, is_coinbase: bool) -> Self {
        Transaction {
            inputs: inputs.clone(),
            outputs,
            signatures: vec![([0; 32], [0; 32]); inputs.len()],
            is_coinbase,
        }
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for input in &self.inputs {
            bytes.extend_from_slice(&input.key.txid);
            bytes.extend_from_slice(&input.key.vout.to_le_bytes());
            bytes.extend_from_slice(&input.public_key.to_bytes());
        }
        for output in &self.outputs {
            bytes.extend_from_slice(&output.amount.to_le_bytes());
            bytes.extend_from_slice(&output.recipient.to_bytes());
        }
        bytes
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = sha2::Sha256::new();
        let bytes = self.to_bytes();
 
        hasher.update(bytes);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
    
    pub fn hash_coinbase(&self, height: u64) -> [u8; 32] {
        let mut hasher = sha2::Sha256::new();
        let mut bytes = self.to_bytes();
        bytes.extend_from_slice(&height.to_le_bytes());
        hasher.update(bytes);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    pub fn sign_input(&mut self, index: usize, private_key: &PrivateKey) {
        let message = self.to_bytes();
        let signature = sign(&message, private_key);
        self.signatures[index] = signature;
    }

    pub fn sign(&mut self, keypair: &[(PrivateKey, PublicKey)]) {
        for i in 0..self.inputs.len() {
            let (priv_key, pub_key) = keypair.iter().find(|(_, pk)| pk == &self.inputs[i].public_key).unwrap();
            if self.inputs[i].public_key == *pub_key {
                self.sign_input(i, priv_key);
            }
        }
    }

}