use crate::{
    error::NetError,
    key::{NodeKey, unzbase32, zbase32},
};
use std::{fmt, net::SocketAddr, str::FromStr};
use url::Url;

const PREFIX: &str = "watane-pair:";
const MAX_HINTS: usize = 8;
const MAX_TEXT: usize = 1024;

/// Where to dial the anchor for the first connection (TECHNICAL 4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hint {
    /// An `https` URL of a relay.
    Relay(Url),
    Addr(SocketAddr),
}

/// The pairing code: the anchor's node key, a one-time secret, and hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairCode {
    pub anchor: NodeKey,
    pub secret: [u8; 16],
    pub hints: Vec<Hint>,
}

impl fmt::Display for PairCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{PREFIX}{}:{}", self.anchor, zbase32(&self.secret))?;
        for hint in &self.hints {
            match hint {
                Hint::Relay(url) => write!(f, ";relay={url}")?,
                Hint::Addr(addr) => write!(f, ";addr={addr}")?,
            }
        }
        Ok(())
    }
}

impl FromStr for PairCode {
    type Err = NetError;
    fn from_str(text: &str) -> Result<Self, NetError> {
        if text.len() > MAX_TEXT {
            return Err(NetError::BadCode);
        }
        let rest = text.strip_prefix(PREFIX).ok_or(NetError::BadCode)?;
        let mut parts = rest.split(';');
        let head = parts.next().ok_or(NetError::BadCode)?;
        let (key, secret) = head.split_once(':').ok_or(NetError::BadCode)?;
        let anchor = key.parse::<NodeKey>().map_err(|_| NetError::BadCode)?;
        let secret: [u8; 16] = unzbase32(secret, 16)
            .ok_or(NetError::BadCode)?
            .try_into()
            .map_err(|_| NetError::BadCode)?;
        let mut hints = Vec::new();
        for part in parts {
            if hints.len() == MAX_HINTS {
                return Err(NetError::BadCode);
            }
            let hint = if let Some(url) = part.strip_prefix("relay=") {
                let url = Url::parse(url).map_err(|_| NetError::BadCode)?;
                if url.scheme() != "https" || url.host().is_none() {
                    return Err(NetError::BadCode);
                }
                Hint::Relay(url)
            } else if let Some(addr) = part.strip_prefix("addr=") {
                Hint::Addr(addr.parse().map_err(|_| NetError::BadCode)?)
            } else {
                return Err(NetError::BadCode);
            };
            hints.push(hint);
        }
        Ok(PairCode {
            anchor,
            secret,
            hints,
        })
    }
}
