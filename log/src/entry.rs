use std::{fmt, str::FromStr};
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId([u8; 32]);
impl EntryId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&data_encoding::HEXLOWER.encode(&self.0))
    }
}
impl fmt::Debug for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntryId({self})")
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an entry id is 64 lower-case hex characters")]
pub struct IdTextError;
impl FromStr for EntryId {
    type Err = IdTextError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 {
            return Err(IdTextError);
        }
        let bytes = data_encoding::HEXLOWER
            .decode(s.as_bytes())
            .map_err(|_| IdTextError)?;
        Ok(Self(bytes.try_into().map_err(|_| IdTextError)?))
    }
}

use crate::{KeyTextError, MAX_BODY, MAX_ENTRY, MAX_KIND, MAX_SEQ, PublicKey};
use ed25519_dalek::{Signature as EdSignature, Signer, SigningKey, VerifyingKey};
use minicbor::{Decoder, Encoder};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("the entry has {0} bytes; the limit is 65728")]
    TooLarge(usize),
    #[error("not an entry: {0}")]
    Malformed(&'static str),
    #[error("not the deterministic encoding of an entry")]
    NotDeterministic,
    #[error("the author is not a valid key: {0}")]
    Author(KeyTextError),
    #[error("the kind is not 1 to 32 bytes of [a-z0-9-] starting with a letter")]
    Kind,
    #[error("the body has {0} bytes; the limit is 65536")]
    BodyTooLarge(usize),
    #[error("seq or time is above 2^53 - 1")]
    OutOfRange,
    #[error("seq 0 has a previous id, or a later seq has none")]
    Prev,
    #[error("the signature does not verify under the strict rule")]
    Signature,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
    raw: Vec<u8>,
    id: EntryId,
    author: PublicKey,
    seq: u64,
    prev: Option<EntryId>,
    time: u64,
    kind: String,
    body: Vec<u8>,
}
impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("id", &self.id)
            .field("author", &self.author)
            .field("seq", &self.seq)
            .field("kind", &self.kind)
            .finish()
    }
}
// Raw fields allow the deterministic check to run before key and grammar checks.
struct Fields {
    author: [u8; 32],
    seq: u64,
    prev: Option<EntryId>,
    time: u64,
    kind: String,
    body: Vec<u8>,
}
impl Fields {
    fn encode(&self, signature: Option<&[u8; 64]>) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        e.array(if signature.is_some() { 7 } else { 6 }).unwrap();
        e.bytes(&self.author).unwrap().u64(self.seq).unwrap();
        match self.prev {
            Some(p) => {
                e.bytes(p.as_bytes()).unwrap();
            }
            None => {
                e.null().unwrap();
            }
        }
        e.u64(self.time)
            .unwrap()
            .str(&self.kind)
            .unwrap()
            .bytes(&self.body)
            .unwrap();
        if let Some(sig) = signature {
            e.bytes(sig).unwrap();
        }
        e.into_writer()
    }
    fn signed(&self) -> Vec<u8> {
        let mut out = b"watane/entry/1\0".to_vec();
        out.extend(self.encode(None));
        out
    }
}
fn byte_string(d: &mut Decoder<'_>, name: &'static str) -> Result<Vec<u8>, DecodeError> {
    let mut out = Vec::new();
    for chunk in d.bytes_iter().map_err(|_| DecodeError::Malformed(name))? {
        out.extend_from_slice(chunk.map_err(|_| DecodeError::Malformed(name))?);
    }
    Ok(out)
}
pub(crate) fn valid_kind(kind: &str) -> bool {
    !kind.is_empty()
        && kind.len() <= MAX_KIND
        && kind.as_bytes()[0].is_ascii_lowercase()
        && kind
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
impl Entry {
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        use DecodeError::*;
        if bytes.len() > MAX_ENTRY {
            return Err(TooLarge(bytes.len()));
        }
        let mut d = Decoder::new(bytes);
        if d.array().map_err(|_| Malformed("array"))? != Some(7) {
            return Err(Malformed("array"));
        }
        let author = byte_string(&mut d, "author")?
            .try_into()
            .map_err(|_| Malformed("author"))?;
        let seq = d.u64().map_err(|_| Malformed("seq"))?;
        let prev = if d.datatype().map_err(|_| Malformed("prev"))? == minicbor::data::Type::Null {
            d.null().map_err(|_| Malformed("prev"))?;
            None
        } else {
            Some(EntryId::from_bytes(
                byte_string(&mut d, "prev")?
                    .try_into()
                    .map_err(|_| Malformed("prev"))?,
            ))
        };
        let time = d.u64().map_err(|_| Malformed("time"))?;
        let mut kind = String::new();
        for chunk in d.str_iter().map_err(|_| Malformed("kind"))? {
            kind.push_str(chunk.map_err(|_| Malformed("kind"))?);
        }
        let body = byte_string(&mut d, "body")?;
        let sig: [u8; 64] = byte_string(&mut d, "sig")?
            .try_into()
            .map_err(|_| Malformed("sig"))?;
        if d.position() != bytes.len() {
            return Err(Malformed("end"));
        }
        let fields = Fields {
            author,
            seq,
            prev,
            time,
            kind,
            body,
        };
        if fields.encode(Some(&sig)) != bytes {
            return Err(NotDeterministic);
        }
        let author = PublicKey::from_bytes(&author).map_err(Author)?;
        if !valid_kind(&fields.kind) {
            return Err(Kind);
        }
        if fields.body.len() > MAX_BODY {
            return Err(BodyTooLarge(fields.body.len()));
        }
        if seq > MAX_SEQ || time > MAX_SEQ {
            return Err(OutOfRange);
        }
        if (seq == 0) != prev.is_none() {
            return Err(Prev);
        }
        VerifyingKey::from_bytes(author.as_bytes())
            .expect("validated author")
            .verify_strict(&fields.signed(), &EdSignature::from_bytes(&sig))
            .map_err(|_| Signature)?;
        Ok(Self {
            raw: bytes.to_vec(),
            id: EntryId::from_bytes(*blake3::hash(bytes).as_bytes()),
            author,
            seq,
            prev,
            time,
            kind: fields.kind,
            body: fields.body,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.raw
    }
    pub fn id(&self) -> EntryId {
        self.id
    }
    pub fn author(&self) -> PublicKey {
        self.author
    }
    pub fn seq(&self) -> u64 {
        self.seq
    }
    pub fn prev(&self) -> Option<EntryId> {
        self.prev
    }
    pub fn time(&self) -> u64 {
        self.time
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}
// Append validates the kind, body and sequence before signing.
pub(crate) fn sign_entry(
    key: &SigningKey,
    seq: u64,
    prev: Option<EntryId>,
    time: u64,
    kind: &str,
    body: &[u8],
) -> Entry {
    let fields = Fields {
        author: *key.verifying_key().as_bytes(),
        seq,
        prev,
        time,
        kind: kind.to_owned(),
        body: body.to_vec(),
    };
    let sig = key.sign(&fields.signed()).to_bytes();
    Entry::decode(&fields.encode(Some(&sig))).expect("append supplies valid fields")
}

#[cfg(test)]
#[path = "../tests/oracle/mod.rs"]
mod oracle;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac5_private_signer_matches_independent_golden_vectors() {
        let key = SigningKey::from_bytes(&oracle::SEED);
        let first = sign_entry(&key, 0, None, 1767225600000, "profile", b"hello");
        assert_eq!(first.bytes(), oracle::hex(oracle::GOLDEN0));
        let prev = EntryId::from_bytes(oracle::hex_array(oracle::ID0));
        let second = sign_entry(&key, 1, Some(prev), 1767225601000, "release", &[1, 2, 3]);
        assert_eq!(second.bytes(), oracle::hex(oracle::GOLDEN1));
    }
}
