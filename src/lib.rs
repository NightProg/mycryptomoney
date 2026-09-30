pub mod crypto;
pub(crate) mod rand;
pub mod types;
pub mod utxo_set;
pub mod chain;
pub mod wallet;
pub mod net;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
