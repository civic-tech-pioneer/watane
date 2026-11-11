#![allow(dead_code)]
pub const SEED: [u8; 32] =
    hex_array("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
pub const PUBLIC: [u8; 32] =
    hex_array("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
const fn nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        _ => panic!("hex"),
    }
}
pub const fn hex_array<const N: usize>(s: &str) -> [u8; N] {
    let mut out = [0; N];
    let mut i = 0;
    while i < N {
        out[i] = nibble(s.as_bytes()[i * 2]) * 16 + nibble(s.as_bytes()[i * 2 + 1]);
        i += 1;
    }
    out
}
pub fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| nibble(p[0]) * 16 + nibble(p[1]))
        .collect()
}
pub fn zbase32(bytes: &[u8]) -> String {
    let alphabet = b"ybndrfg8ejkmcpqxot1uwisza345h769";
    (0..(bytes.len() * 8).div_ceil(5))
        .map(|i| {
            let mut n = 0;
            for j in 0..5 {
                let bit = i * 5 + j;
                n <<= 1;
                if bit < bytes.len() * 8 {
                    n |= (bytes[bit / 8] >> (7 - bit % 8)) & 1;
                }
            }
            alphabet[n as usize] as char
        })
        .collect()
}
pub fn sign(seed: [u8; 32], msg: &[u8]) -> [u8; 64] {
    *ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(seed))
        .sk
        .sign(msg, None)
}

pub mod cbor {
    pub fn head(major: u8, n: u64, out: &mut Vec<u8>) {
        if n < 24 {
            out.push(major << 5 | n as u8);
        } else if n <= 255 {
            out.extend([major << 5 | 24, n as u8]);
        } else if n <= 65535 {
            out.push(major << 5 | 25);
            out.extend((n as u16).to_be_bytes());
        } else if n <= u32::MAX as u64 {
            out.push(major << 5 | 26);
            out.extend((n as u32).to_be_bytes());
        } else {
            out.push(major << 5 | 27);
            out.extend(n.to_be_bytes());
        }
    }
    pub fn bytes(bytes: &[u8], out: &mut Vec<u8>) {
        head(2, bytes.len() as u64, out);
        out.extend(bytes);
    }
    pub fn fields(
        author: &[u8; 32],
        seq: u64,
        prev: Option<&[u8; 32]>,
        time: u64,
        kind: &str,
        body: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        bytes(author, &mut out);
        head(0, seq, &mut out);
        match prev {
            None => out.push(0xf6),
            Some(p) => bytes(p, &mut out),
        }
        head(0, time, &mut out);
        head(3, kind.len() as u64, &mut out);
        out.extend(kind.as_bytes());
        bytes(body, &mut out);
        out
    }
    pub fn signed(
        author: &[u8; 32],
        seq: u64,
        prev: Option<&[u8; 32]>,
        time: u64,
        kind: &str,
        body: &[u8],
    ) -> Vec<u8> {
        let mut out = b"watane/entry/1\0".to_vec();
        out.push(0x86);
        out.extend(fields(author, seq, prev, time, kind, body));
        out
    }
    pub fn entry(
        author: &[u8; 32],
        seq: u64,
        prev: Option<&[u8; 32]>,
        time: u64,
        kind: &str,
        body: &[u8],
        sig: &[u8; 64],
    ) -> Vec<u8> {
        let mut out = vec![0x87];
        out.extend(fields(author, seq, prev, time, kind, body));
        bytes(sig, &mut out);
        out
    }
}
pub fn entry(seq: u64, prev: Option<&[u8; 32]>, time: u64, kind: &str, body: &[u8]) -> Vec<u8> {
    cbor::entry(
        &PUBLIC,
        seq,
        prev,
        time,
        kind,
        body,
        &sign(SEED, &cbor::signed(&PUBLIC, seq, prev, time, kind, body)),
    )
}
pub const GOLDEN0: &str = "875820d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a00f61b0000019b76daa8006770726f66696c654568656c6c6f58401783608165a9ea936ffa3b538b1e0918f9a47521ed025a22a37e5082710b4c1ddae418c709d8e90d7fa273e1be68aeeb772bfb6c44bc1c2c28dd570c7b8c4c02";
pub const GOLDEN1: &str = "875820d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a01582026e9f56befea2e5c43ba15e0fbfb28ac527d50d703ea3db5b0a6d89732a2fa201b0000019b76daabe86772656c656173654301020358402b5caac5bf68e6075addc9d84f83f737ed5c9c42bbbe904d9cb55f8c9696246ebb3b3bd03c2d3a1d2cd28fd986d507d3acbc66b89cd61cf886b16b9db53c9107";
pub const ID0: &str = "26e9f56befea2e5c43ba15e0fbfb28ac527d50d703ea3db5b0a6d89732a2fa20";
pub const ID1: &str = "4f193a17cf9a3f16a29b3a58da0ac358cf6ff3eef28669a2ce8e4e4367abd176";
