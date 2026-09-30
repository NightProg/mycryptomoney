use rug::Integer;
use rug::ops::RemRounding;
use sha2::Digest;
use crate::crypto::keypair::PublicKey;
use crate::rand::{rand, rand_below};

pub type Signature = ([u8; 32], [u8; 32]);

pub fn sign(message: &[u8], private_key_bytes: &[u8; 32]) -> Signature {
    let private_key = Integer::from_digits(private_key_bytes, rug::integer::Order::Msf);
    let z = {
        let mut hash = sha2::Sha256::new();
        hash.update(message);
        Integer::from_digits(&hash.finalize(), rug::integer::Order::Msf) % PublicKey::order()
    };
    let k = rand_below(&PublicKey::order());
    let r = PublicKey::generator().scalar_multiply(&k).x().clone() % PublicKey::order();
    let k_inv = k.clone().invert(&PublicKey::order()).unwrap();
    let s = (z + r.clone() * private_key) * k_inv % PublicKey::order();
    if s == Integer::from(0) || r == Integer::from(0) {
        return sign(message, private_key_bytes);
    }

    ({
        let mut r_bytes = [0u8; 32];
        let r_digits = r.to_digits::<u8>(rug::integer::Order::Msf);
        r_bytes[32 - r_digits.len()..].copy_from_slice(&r_digits);
        r_bytes
    },
    {
        let mut s_bytes = [0u8; 32];
        let s_digits = s.to_digits::<u8>(rug::integer::Order::Msf);
        s_bytes[32 - s_digits.len()..].copy_from_slice(&s_digits);
        s_bytes
    })
}

pub fn verify(message: &[u8], (r, s): &Signature, public_key: PublicKey) -> bool {
    let z = {
        let mut hash = sha2::Sha256::new();
        hash.update(message);
        Integer::from_digits(&hash.finalize(), rug::integer::Order::Msf) % PublicKey::order()
    };
    let (r, s) = (
        Integer::from_digits(r, rug::integer::Order::Msf),
        Integer::from_digits(s, rug::integer::Order::Msf),
    );
    let s_inv = s.clone().invert(&PublicKey::order()).unwrap();

    let u1 = (z * s_inv.clone()) % PublicKey::order();
    let u2 = (r.clone() * s_inv) % PublicKey::order();
    let point = PublicKey::generator().scalar_multiply(&u1).add(&public_key.scalar_multiply(&u2));
    point.x().rem_euc(PublicKey::order()) == r
}

#[cfg(test)]
mod tests {
    use crate::crypto::keypair::generate_keypair;
    use super::*;
    #[test]
    fn test_sign_and_verify() {
        let (private_key, public_key) = generate_keypair();
        let message = b"Hello, world!";
        let signature = sign(message, &private_key);
        assert!(verify(message, &signature, public_key));
    }

    #[test]
    fn test_invalid_signature() {
        let (private_key, public_key) = generate_keypair();
        let message = b"Hello, world!";
        let signature = sign(message, &private_key);
        let tampered_message = b"Hello, blockchain!";
        assert!(!verify(tampered_message, &signature, public_key));
    }


    #[test]
    fn test_invalid_public_key() {
        let (private_key, public_key) = generate_keypair();
        let message = b"Hello, world!";
        let signature = sign(message, &private_key);
        let fake_public_key = generate_keypair().1;
        assert!(!verify(message, &signature, fake_public_key));
    }
    

}