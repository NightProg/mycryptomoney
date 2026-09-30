use rug::Integer;

pub(crate) fn rand(mut bits: u32) -> Integer {
    let mut rand = rug::rand::RandState::new();

    let mut res = Integer::with_capacity(bits as usize);

    while bits > 0 {
        let generate = bits.min(32);

        res = (res << generate) + rand.bits(generate);

        bits -= generate;
    }

    res
}

pub fn rand_below(n: &Integer) -> Integer {
    let bit_len  = n.significant_bits() as usize;
    let byte_len = (bit_len + 7) / 8;

    loop {
        let mut buf = vec![0u8; byte_len];
        rand::fill(&mut buf);

        let candidate = Integer::from_digits(&buf, rug::integer::Order::Msf);

        if candidate >= 1 && candidate < *n {
            return candidate;
        }
    }
}