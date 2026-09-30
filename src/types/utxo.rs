use crate::crypto::keypair::PublicKey;


#[derive(Debug, Clone, PartialEq, Eq, Hash, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize, serde::Deserialize)]
pub struct UTXOKey {
    pub txid: [u8; 32],
    pub vout: usize,
}


#[derive(Debug, Clone, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)]
pub struct UTXOValue {
    pub amount: u64,
    pub owner: PublicKey,
    pub script_pubkey: Vec<u8>,
}