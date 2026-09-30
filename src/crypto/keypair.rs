use rkyv::Place;
use rug::Integer;
use rug::ops::{Pow, RemRounding};

pub type PrivateKey = [u8; 32];

pub fn generate_keypair() -> (PrivateKey, PublicKey) {
    let private_key = crate::rand::rand_below(&PublicKey::order());
    let public_key = PublicKey::generator().scalar_multiply(&private_key);
    let mut private_key_bytes = [0u8; 32];
    let private_key_digits = private_key.to_digits::<u8>(rug::integer::Order::Msf);
    private_key_bytes[32 - private_key_digits.len()..].copy_from_slice(&private_key_digits);
    (private_key_bytes, public_key)
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, serde::Serialize, serde::Deserialize)]
pub struct PublicKey {
    pub x: [u8; 32],
    pub y: [u8; 32],
}

impl PublicKey {
    pub fn from_private(private_key: &PrivateKey) -> Self {
        PublicKey::generator().scalar_multiply(&Integer::from_digits(private_key, rug::integer::Order::Msf))
    }

    pub fn from_integer(x: &Integer, y: &Integer) -> Self {
        let mut x_bytes = [0u8; 32];
        let mut y_bytes = [0u8; 32];
        let x_digits = x.to_digits::<u8>(rug::integer::Order::Msf);
        let y_digits = y.to_digits::<u8>(rug::integer::Order::Msf);
        x_bytes[32 - x_digits.len()..].copy_from_slice(&x_digits);
        y_bytes[32 - y_digits.len()..].copy_from_slice(&y_digits);
        PublicKey { x: x_bytes, y: y_bytes }
    }

    pub fn prime() -> Integer {
        Integer::from_str_radix(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F",
            16
        ).unwrap()
    }


    pub fn order() -> Integer {
        Integer::from_str_radix("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141", 16).unwrap()
    }
    pub const fn generator() -> Self {
        PublicKey {
            x: [0x79, 0xBE, 0x66, 0x7E, 0xF9, 0xDC, 0xBB, 0xAC, 0x55, 0xA0, 0x62, 0x95, 0xCE, 0x87, 0x0B, 0x07,
                0x02, 0x9B, 0xFC, 0xDB, 0x2D, 0xCE, 0x28, 0xD9, 0x59, 0xF2, 0x81, 0x5B, 0x16, 0xF8, 0x17, 0x98],
            y: [0x48, 0x3A, 0xDA, 0x77, 0x26, 0xA3, 0xC4, 0x65, 0x5D, 0xA4, 0xFB, 0xFC, 0x0E, 0x11, 0x08, 0xA8,
                0xFD, 0x17, 0xB4, 0x48, 0xA6, 0x85, 0x54, 0x19, 0x9C, 0x47, 0xD0, 0x8F, 0xFB, 0x10, 0xD4, 0xB8],
        }
    }
    pub fn infinity() -> Self {
        let mut x = [0u8; 32];
        let mut y = [0u8; 32];
        x[31] = 0xFF;
        y[31] = 0xFF;
        PublicKey { x, y }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.x);
        bytes.extend_from_slice(&self.y);
        bytes
    }

    pub fn x(&self) -> Integer {
        Integer::from_digits(&self.x, rug::integer::Order::Msf)
    }

    pub fn y(&self) -> Integer {
        Integer::from_digits(&self.y, rug::integer::Order::Msf)
    }

    pub fn to_compressed_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        if self.y().is_even() {
            bytes.push(0x02);
        } else {
            bytes.push(0x03);
        }
        bytes.extend_from_slice(&self.x);
        bytes
    }

    pub fn double(&self) -> Self {
        let p = PublicKey::prime();
        let x = self.x();
        let y = self.y();

        let num = (Integer::from(3) * x.clone().pow(2)).rem_euc(&p);

        let den = (Integer::from(2) * y.clone()).rem_euc(&p);

        let l = (num * den.invert(&p).unwrap()).rem_euc(&p);

        let x3 = (l.clone().pow(2) - Integer::from(2) * x.clone())
            .rem_euc(&p);

        let y3 = (l * (x.clone() - x3.clone()) - y.clone())
            .rem_euc(&p);

        PublicKey::from_integer(&x3, &y3)
    }

    pub fn add(&self, other: &PublicKey) -> Self {
        let p = PublicKey::prime();
        let x = self.x();
        let y = self.y();
        let ox = other.x();
        let oy = other.y();
        if self.is_infinity()  { return other.clone(); }
        if other.is_infinity() { return self.clone(); }

        if x == ox && y == oy{
            return self.double();
        }

        if x == ox {
            return PublicKey::infinity();
        }

        // lambda = (y2 - y1) · (x2 - x1)⁻¹ mod p
        let num = (oy.clone() - y.clone()).rem_euc(&p);
        let den = (ox.clone() - x.clone()).rem_euc(&p);
        let l   = num * den.invert(&p).unwrap() % &p;

        // x3 = l² - x1 - x2 mod p
        let x3 = (l.clone().pow(2) - x.clone() - ox.clone())
            .rem_euc(&p);

        // y3 = l(x1 - x3) - y1 mod p
        let y3 = (l * (x.clone() - x3.clone()) - y.clone())
            .rem_euc(&p);

        PublicKey::from_integer(&x3, &y3)
    }

    pub fn scalar_multiply(&self, scalar: &Integer) -> Self {
        let mut result  = PublicKey::infinity();
        let mut addend  = self.clone();
        let mut k       = scalar.clone();

        while k > 0 {
            if k.is_odd() {
                result = result.add(&addend);
            }
            addend  = addend.double();
            k >>= 1;  // division par 2 via bit shift
        }

        result
    }

    pub fn is_infinity(&self) -> bool {
        self.x[31] == 0xFF && self.y[31] == 0xFF
            && self.x[..31] == [0u8; 31]
            && self.y[..31] == [0u8; 31]
    }
}