//! Statement bodies (TECHNICAL 4.1): the CBOR encoding of profile, withdraw,
//! list, include and unlist bodies, and the text rules their fields obey.

use crate::{EntryId, Listing, Profile, Withdraw};
use minicbor::{Decoder, Encoder};
impl Profile {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        e.array(1)
            .expect("encoding into a Vec cannot fail")
            .str(&self.name)
            .expect("encoding into a Vec cannot fail");
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
            .expect("encoding into a Vec cannot fail")
            .bytes(self.release.as_bytes())
            .expect("encoding into a Vec cannot fail")
            .str(&self.reason)
            .expect("encoding into a Vec cannot fail");
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
        assert_eq!(
            Profile::decode(&p).expect("encoding into a Vec cannot fail"),
            profile
        );
        assert_eq!(withdraw.encode(), w);
        assert_eq!(
            Withdraw::decode(&w).expect("encoding into a Vec cannot fail"),
            withdraw
        );
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
    text_range(value, 1, max)
}
pub(crate) fn note_text(value: &str) -> Result<(), &'static str> {
    text_range(value, 0, 280)
}
fn text_range(value: &str, min: usize, max: usize) -> Result<(), &'static str> {
    if !(min..=max).contains(&value.chars().count()) {
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
            text("Alice Bob", max).unwrap();
        }
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    const LINK: &str = "watane:47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy@https://alice.example.org/mods/#reading-list";
    #[test]
    fn ac1_body_vectors_step4() {
        for (link, note, head, note_head) in [
            (LINK, "good for books", vec![0x82, 0x78, 0x68], 0x6e),
            (&LINK[..91], "", vec![0x82, 0x78, 0x5b], 0x60),
        ] {
            let mut bytes = head;
            bytes.extend(link.as_bytes());
            bytes.push(note_head);
            bytes.extend(note.as_bytes());
            let listing = Listing {
                link: link.parse().expect("encoding into a Vec cannot fail"),
                note: note.into(),
            };
            assert_eq!(listing.encode(), bytes);
            assert_eq!(
                Listing::decode(&bytes).expect("encoding into a Vec cannot fail"),
                listing
            );
            let mut bad = vec![];
            let mut b = bytes.clone();
            b[0] = 0x83;
            bad.push(b);
            let mut b = bytes.clone();
            b[0] = 0x9f;
            b.push(0xff);
            bad.push(b);
            let mut b = bytes.clone();
            b.push(0);
            bad.push(b);
            let mut b = vec![0x98, 2];
            b.extend(&bytes[1..]);
            bad.push(b);
            let mut b = vec![0x82, 0x79, 0, link.len() as u8];
            b.extend(&bytes[3..]);
            bad.push(b);
            for b in bad {
                assert!(Listing::decode(&b).is_err(), "{b:x?}");
            }
        }
        let mut bytes = vec![0x81, 0x58, 0x20];
        bytes.extend([0x22; 32]);
        let id = EntryId::from_bytes([0x22; 32]);
        assert_eq!(encode_unlist(&id), bytes);
        assert_eq!(
            decode_unlist(&bytes).expect("encoding into a Vec cannot fail"),
            id
        );
        for b in [
            [vec![0x82], bytes[1..].to_vec()].concat(),
            [vec![0x9f], bytes[1..].to_vec(), vec![0xff]].concat(),
            [bytes.clone(), vec![0]].concat(),
            [vec![0x98, 1], bytes[1..].to_vec()].concat(),
            [vec![0x81, 0x59, 0, 32], bytes[3..].to_vec()].concat(),
        ] {
            decode_unlist(&b).unwrap_err();
        }
        let text = &LINK[..90]; // missing trailing slash
        let mut bytes = vec![0x82, 0x78, 90];
        bytes.extend(text.as_bytes());
        bytes.push(0x60);
        Listing::decode(&bytes).unwrap_err();
    }
    #[test]
    fn ac2_text_rules_note() {
        for n in [0, 1, 280, 281] {
            assert_eq!(note_text(&"é".repeat(n)).is_ok(), n <= 280);
        }
        for c in (0..=0x9f)
            .filter_map(char::from_u32)
            .filter(|c| c.is_control())
            .chain(['\u{061c}', '\u{200e}', '\u{200f}'])
            .chain('\u{202a}'..='\u{202e}')
            .chain('\u{2066}'..='\u{2069}')
        {
            assert!(note_text(&format!("a{c}b")).is_err());
        }
        for c in [' ', '\u{00a0}', '\u{2003}'] {
            assert!(note_text(&format!("{c}a")).is_err());
            assert!(note_text(&format!("a{c}")).is_err());
        }
    }
}

impl Listing {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        e.array(2)
            .expect("encoding into a Vec cannot fail")
            .str(&self.link.to_string())
            .expect("encoding into a Vec cannot fail")
            .str(&self.note)
            .expect("encoding into a Vec cannot fail");
        e.into_writer()
    }
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        let mut d = Decoder::new(bytes);
        if d.array().map_err(|_| "listing must be an array")? != Some(2) {
            return Err("listing needs two fields");
        }
        let text = d.str().map_err(|_| "link must be definite UTF-8 text")?;
        let link: crate::Link = text.parse().map_err(|_| "invalid listing link")?;
        if link.to_string() != text {
            return Err("non-canonical listing link");
        }
        let note = d
            .str()
            .map_err(|_| "note must be definite UTF-8 text")?
            .to_owned();
        note_text(&note)?;
        let value = Self { link, note };
        if value.encode() != bytes {
            return Err("non-canonical listing");
        }
        Ok(value)
    }
}
pub(crate) fn encode_unlist(id: &EntryId) -> Vec<u8> {
    let mut e = Encoder::new(Vec::new());
    e.array(1)
        .expect("encoding into a Vec cannot fail")
        .bytes(id.as_bytes())
        .expect("encoding into a Vec cannot fail");
    e.into_writer()
}
pub(crate) fn decode_unlist(bytes: &[u8]) -> Result<EntryId, &'static str> {
    let mut d = Decoder::new(bytes);
    if d.array().map_err(|_| "unlist must be an array")? != Some(1) {
        return Err("unlist needs one field");
    }
    let id = EntryId::from_bytes(
        d.bytes()
            .map_err(|_| "listing must be definite bytes")?
            .try_into()
            .map_err(|_| "listing needs 32 bytes")?,
    );
    if encode_unlist(&id) != bytes {
        return Err("non-canonical unlist");
    }
    Ok(id)
}
