//! The one signed form of TECHNICAL 4.3: the device list. Encode, sign and
//! verify only. The node key signs nothing else (AGENTS rule 5): there is
//! no general `sign`.

use crate::{
    Role,
    cbor::{self, Item, Reader},
    error::NetError,
    key::{NodeKey, public_of},
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

const PREFIX: &[u8] = b"watane/devices/1\0";

/// The anchor's list of own machines (TECHNICAL 4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceList {
    pub anchor: NodeKey,
    pub seq: u64,
    /// Milliseconds since the Unix epoch.
    pub time: u64,
    /// Days; 0 for never.
    pub expires_days: u32,
    /// Sorted by node key, no duplicate, holds the anchor.
    pub devices: Vec<(NodeKey, Role)>,
}

fn bad<T>(cause: &str) -> Result<T, NetError> {
    Err(NetError::BadList {
        cause: cause.to_owned(),
    })
}

impl DeviceList {
    fn check(&self) -> Result<(), NetError> {
        if !self.devices.windows(2).all(|w| w[0].0 < w[1].0) {
            return bad("the devices are not sorted by key, or one is listed twice");
        }
        if !self.devices.iter().any(|(k, _)| *k == self.anchor) {
            return bad("the anchor is not in the list");
        }
        Ok(())
    }

    /// The signed bytes: prefix, a zero byte (in the prefix), the CBOR of the
    /// five unsigned items as one array.
    fn signed_bytes(&self) -> Vec<u8> {
        let mut out = PREFIX.to_vec();
        cbor::array(5, &mut out);
        self.items(&mut out);
        out
    }

    fn items(&self, out: &mut Vec<u8>) {
        cbor::bytes(self.anchor.as_bytes(), out);
        cbor::uint(self.seq, out);
        cbor::uint(self.time, out);
        cbor::uint(self.expires_days as u64, out);
        cbor::array(self.devices.len(), out);
        for (node, role) in &self.devices {
            cbor::array(2, out);
            cbor::bytes(node.as_bytes(), out);
            cbor::uint(
                match role {
                    Role::Light => 0,
                    Role::Full => 1,
                },
                out,
            );
        }
    }

    /// The list as a CBOR array of the five items and the signature, signed
    /// by the anchor's node key (`anchor_seed`, the seed of `node.key`).
    pub fn sign(&self, anchor_seed: &[u8; 32]) -> Result<Vec<u8>, NetError> {
        self.check()?;
        if public_of(anchor_seed) != self.anchor {
            return bad("the signing key is not the anchor's");
        }
        let signature = SigningKey::from_bytes(anchor_seed).sign(&self.signed_bytes());
        let mut out = Vec::new();
        cbor::array(6, &mut out);
        self.items(&mut out);
        cbor::bytes(&signature.to_bytes(), &mut out);
        Ok(out)
    }

    /// Decode and verify (`verify_strict`) a signed list.
    pub fn verify(bytes: &[u8]) -> Result<DeviceList, NetError> {
        let mut reader = Reader::new(bytes);
        let item = reader
            .item()
            .or_else(|e| bad(&format!("not deterministic CBOR: {e}")))?;
        if !reader.at_end() {
            return bad("trailing bytes");
        }
        let Item::Array(fields) = item else {
            return bad("not an array");
        };
        let [
            Item::Bytes(anchor),
            Item::Uint(seq),
            Item::Uint(time),
            Item::Uint(expires),
            Item::Array(devices),
            Item::Bytes(signature),
        ] = fields.as_slice()
        else {
            return bad("not the six items of a device list");
        };
        let Some(anchor) = <[u8; 32]>::try_from(*anchor).ok().map(NodeKey::from_bytes) else {
            return bad("the anchor is not 32 bytes");
        };
        let Ok(expires_days) = u32::try_from(*expires) else {
            return bad("the expiry is too large");
        };
        let mut list = DeviceList {
            anchor,
            seq: *seq,
            time: *time,
            expires_days,
            devices: Vec::with_capacity(devices.len()),
        };
        for device in devices {
            let Item::Array(pair) = device else {
                return bad("a device is not an array");
            };
            let [Item::Bytes(node), Item::Uint(role)] = pair.as_slice() else {
                return bad("a device is not [node, role]");
            };
            let Some(node) = <[u8; 32]>::try_from(*node).ok().map(NodeKey::from_bytes) else {
                return bad("a device key is not 32 bytes");
            };
            let role = match role {
                0 => Role::Light,
                1 => Role::Full,
                _ => return bad("a role is neither 0 nor 1"),
            };
            list.devices.push((node, role));
        }
        list.check()?;
        let Ok(signature) = Signature::from_slice(signature) else {
            return bad("the signature is not 64 bytes");
        };
        let Ok(key) = VerifyingKey::from_bytes(list.anchor.as_bytes()) else {
            return bad("the anchor is not a valid key");
        };
        if key.verify_strict(&list.signed_bytes(), &signature).is_err() {
            return bad("the signature does not verify");
        }
        Ok(list)
    }
}
