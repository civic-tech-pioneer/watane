use crate::{EntryId, Profile, Withdraw};
use minicbor::{Decoder, Encoder};
impl Profile {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        e.array(1).unwrap().str(&self.name).unwrap();
        e.into_writer()
    }
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        let mut d = Decoder::new(bytes);
        if d.array().map_err(|_| "profile must be an array")? != Some(1) {
            return Err("profile needs one field");
        }
        let name = d
            .str()
            .map_err(|_| "name must be definite UTF-8 text")?
            .to_owned();
        text(&name, 64)?;
        let value = Self { name };
        if value.encode() != bytes {
            return Err("non-canonical profile");
        }
        Ok(value)
    }
}
impl Withdraw {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        e.array(2)
            .unwrap()
            .bytes(self.release.as_bytes())
            .unwrap()
            .str(&self.reason)
            .unwrap();
        e.into_writer()
    }
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        let mut d = Decoder::new(bytes);
        if d.array().map_err(|_| "withdraw must be an array")? != Some(2) {
            return Err("withdraw needs two fields");
        }
        let release = EntryId::from_bytes(
            d.bytes()
                .map_err(|_| "release must be definite bytes")?
                .try_into()
                .map_err(|_| "release needs 32 bytes")?,
        );
        let reason = d
            .str()
            .map_err(|_| "reason must be definite UTF-8 text")?
            .to_owned();
        text(&reason, 280)?;
        let value = Self { release, reason };
        if value.encode() != bytes {
            return Err("non-canonical withdraw");
        }
        Ok(value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntryId;
    #[test]
    fn ac2_body_vectors() {
        let profile = Profile {
            name: "Alice".into(),
        };
        let p = vec![0x81, 0x65, 0x41, 0x6c, 0x69, 0x63, 0x65];
        let withdraw = Withdraw {
            release: EntryId::from_bytes([0x11; 32]),
            reason: "bug".into(),
        };
        let mut w = vec![0x82, 0x58, 0x20];
        w.extend([0x11; 32]);
        w.extend([0x63, 0x62, 0x75, 0x67]);
        assert_eq!(profile.encode(), p);
        assert_eq!(Profile::decode(&p).unwrap(), profile);
        assert_eq!(withdraw.encode(), w);
        assert_eq!(Withdraw::decode(&w).unwrap(), withdraw);
        for (bytes, is_profile) in [(&p, true), (&w, false)] {
            let mut bad = Vec::new();
            let mut b = bytes.clone();
            b[0] += 1;
            bad.push(b);
            let mut b = bytes.clone();
            b[0] = 0x9f;
            b.push(0xff);
            bad.push(b);
            let mut b = bytes.clone();
            b.push(0);
            bad.push(b);
            let mut b = vec![0x98, bytes[0] - 0x80];
            b.extend_from_slice(&bytes[1..]);
            bad.push(b);
            let mut b = bytes.clone();
            b[0] = 0xa1;
            bad.push(b);
            let mut b = bytes.clone();
            b[1] = 0x18;
            bad.push(b);
            if is_profile {
                let mut b = vec![0x81, 0x78, 5];
                b.extend_from_slice(&bytes[2..]);
                bad.push(b);
                bad.push(vec![0x81, 0x7f, 0x61, b'a', 0xff]);
            } else {
                let mut b = vec![0x82, 0x59, 0, 32];
                b.extend_from_slice(&bytes[3..]);
                bad.push(b);
                bad.push(vec![0x82, 0x5f, 0xff, 0x61, b'a']);
            }
            for b in bad {
                assert!(
                    if is_profile {
                        Profile::decode(&b).is_err()
                    } else {
                        Withdraw::decode(&b).is_err()
                    },
                    "{b:x?}"
                );
            }
        }
    }
}

pub(crate) fn text(value: &str, max: usize) -> Result<(), &'static str> {
    if !(1..=max).contains(&value.chars().count()) {
        return Err("invalid character count");
    }
    if value.chars().any(|c| c.is_control() || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) { return Err("control or direction character refused"); }
    if value.starts_with(char::is_whitespace) || value.ends_with(char::is_whitespace) {
        return Err("leading or trailing whitespace refused");
    }
    Ok(())
}
#[cfg(test)]
mod text_tests {
    use super::*;
    #[test]
    fn ac3_text_rules() {
        for max in [64, 280] {
            for n in [0, 1, 64, 65, 280, 281] {
                assert_eq!(
                    text(&"é".repeat(n), max).is_ok(),
                    (1..=max).contains(&n),
                    "{n}/{max}"
                );
            }
            for c in (0..=0x9f)
                .filter_map(char::from_u32)
                .filter(|c| c.is_control())
                .chain(['\u{061c}', '\u{200e}', '\u{200f}'])
                .chain(('\u{202a}'..='\u{202e}').chain('\u{2066}'..='\u{2069}'))
            {
                assert!(text(&format!("A{c}b"), max).is_err(), "{c:?}");
            }
            for c in [' ', '\t', '\n', '\u{00a0}', '\u{2003}'] {
                assert!(text(&format!("{c}Alice"), max).is_err());
                assert!(text(&format!("Alice{c}"), max).is_err());
            }
            assert!(text("Alice Bob", max).is_ok());
        }
    }
}
