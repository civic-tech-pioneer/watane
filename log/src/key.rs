use ed25519_dalek::VerifyingKey;
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeyTextError {
    #[error("a key text has 52 characters, not {0}")]
    Length(usize),
    #[error("a key text has only z-base-32 characters")]
    Alphabet,
    #[error("the unused bits of the key text are not zero")]
    UnusedBits,
    #[error("the bytes are not a canonical Ed25519 point")]
    NotCanonical,
    #[error("the key is a point of small order")]
    SmallOrder,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PublicKey([u8; 32]);
fn encoding() -> data_encoding::Encoding {
    let mut spec = data_encoding::Specification::new();
    spec.symbols.push_str("ybndrfg8ejkmcpqxot1uwisza345h769");
    spec.encoding().expect("fixed alphabet")
}
impl PublicKey {
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, KeyTextError> {
        let key = VerifyingKey::from_bytes(bytes).map_err(|_| KeyTextError::NotCanonical)?;
        if key.to_edwards().compress().as_bytes() != bytes {
            return Err(KeyTextError::NotCanonical);
        }
        if key.is_weak() {
            return Err(KeyTextError::SmallOrder);
        }
        Ok(Self(*bytes))
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn fingerprint(&self) -> String {
        self.to_string()[..10].to_owned()
    }
}
impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&encoding().encode(&self.0))
    }
}
impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({self})")
    }
}
impl FromStr for PublicKey {
    type Err = KeyTextError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 52 {
            return Err(KeyTextError::Length(s.len()));
        }
        if !s
            .bytes()
            .all(|b| b"ybndrfg8ejkmcpqxot1uwisza345h769".contains(&b))
        {
            return Err(KeyTextError::Alphabet);
        }
        let decoded = encoding()
            .decode(s.as_bytes())
            .map_err(|_| KeyTextError::UnusedBits)?;
        Self::from_bytes(&decoded.try_into().expect("52 symbols hold 32 bytes"))
    }
}
