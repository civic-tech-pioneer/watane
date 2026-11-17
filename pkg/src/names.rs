use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
#[error("invalid {kind} `{text}`: {reason}")]
pub struct NameError {
    pub kind: &'static str,
    pub text: String,
    pub reason: &'static str,
}

fn error(kind: &'static str, text: &str, reason: &'static str) -> NameError {
    NameError {
        kind,
        text: text.chars().take(80).collect(),
        reason,
    }
}

macro_rules! text_serde {
    ($($ty:ty),+ $(,)?) => {$(
        impl Serialize for $ty {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                String::deserialize(deserializer)?.parse().map_err(serde::de::Error::custom)
            }
        }
    )+};
}

fn identifier(s: &str) -> bool {
    s.split('-').all(|word| {
        word.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            && word
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);
impl Slug {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl FromStr for Slug {
    type Err = NameError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const KEYWORDS: &str = "use type func u8 u16 u32 u64 s8 s16 s32 s64 f32 f64 char resource own borrow record flags variant enum bool string option result future stream error-context list map as from static interface tuple world import export package constructor include with async";
        if s.len() > 40 || !identifier(s) || KEYWORDS.split_whitespace().any(|w| w == s) {
            return Err(error(
                "slug",
                s,
                "use 1..=40 bytes of WIT identifier words, excluding keywords",
            ));
        }
        Ok(Self(s.into()))
    }
}
impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}
impl FromStr for Version {
    type Err = NameError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || {
            error(
                "version",
                s,
                "use three u64 decimal numbers without leading zeros",
            )
        };
        let parts: Vec<_> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(invalid());
        }
        let number = |p: &str| {
            if p.is_empty()
                || !p.bytes().all(|b| b.is_ascii_digit())
                || (p.len() > 1 && p.starts_with('0'))
            {
                return Err(invalid());
            }
            p.parse::<u64>().map_err(|_| invalid())
        };
        Ok(Self {
            major: number(parts[0])?,
            minor: number(parts[1])?,
            patch: number(parts[2])?,
        })
    }
}
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
pub fn compatible(wanted: &Version, offered: &Version) -> bool {
    offered.major == wanted.major
        && if wanted.major == 0 {
            offered.minor == wanted.minor && offered.patch >= wanted.patch
        } else {
            offered >= wanted
        }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterfaceName(String);
impl FromStr for InterfaceName {
    type Err = NameError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || {
            error(
                "interface name",
                s,
                "use namespace:package/interface@version, at most 128 bytes",
            )
        };
        if s.len() > 128 {
            return Err(invalid());
        }
        let (path, version) = s.split_once('@').ok_or_else(invalid)?;
        let (package, interface) = path.split_once('/').ok_or_else(invalid)?;
        let (namespace, package) = package.split_once(':').ok_or_else(invalid)?;
        if ![namespace, package, interface]
            .iter()
            .all(|p| identifier(p))
            || version.parse::<Version>().is_err()
        {
            return Err(invalid());
        }
        Ok(Self(s.into()))
    }
}
impl InterfaceName {
    // Construction validates each separator and the version.
    pub fn namespace(&self) -> &str {
        self.0.split(':').next().unwrap_or_default()
    }
    pub fn package(&self) -> &str {
        self.0.split([':', '/']).nth(1).unwrap_or_default()
    }
    pub fn interface(&self) -> &str {
        self.0.split(['/', '@']).nth(1).unwrap_or_default()
    }
    pub fn version(&self) -> Version {
        self.0
            .split('@')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(Version {
                major: 0,
                minor: 0,
                patch: 0,
            })
    }
    pub fn satisfied_by(&self, offered: &Self) -> bool {
        self.namespace() == offered.namespace()
            && self.package() == offered.package()
            && self.interface() == offered.interface()
            && compatible(&self.version(), &offered.version())
    }
}
impl fmt::Display for InterfaceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HttpHost(String);
impl HttpHost {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl FromStr for HttpHost {
    type Err = NameError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || {
            error(
                "HTTP host",
                s,
                "use a lowercase DNS name and optional port 1..=65535 without leading zeros",
            )
        };
        let name = if let Some((host, port)) = s.split_once(':') {
            if port.starts_with('0')
                || !port.bytes().all(|b| b.is_ascii_digit())
                || port.parse::<u16>().ok().is_none_or(|p| p == 0)
            {
                return Err(invalid());
            }
            host
        } else {
            s
        };
        let alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
        if name.len() > 253
            || !name.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label.bytes().next().is_some_and(alnum)
                    && label.bytes().last().is_some_and(alnum)
                    && label.bytes().all(|b| alnum(b) || b == b'-')
            })
        {
            return Err(invalid());
        }
        Ok(Self(s.into()))
    }
}
impl fmt::Display for HttpHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// RFC 4648 base32 and z-base-32 use the same bit layout.
const ZBASE32: &[u8; 32] = b"ybndrfg8ejkmcpqxot1uwisza345h769";
const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublisherKey([u8; 32]);
impl PublisherKey {
    pub fn from_bytes(b: [u8; 32]) -> Self {
        Self(b)
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl fmt::Display for PublisherKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in data_encoding::BASE32_NOPAD.encode(&self.0).bytes() {
            if let Some(i) = BASE32.iter().position(|&c| c == b) {
                write!(f, "{}", ZBASE32[i] as char)?;
            }
        }
        Ok(())
    }
}
impl FromStr for PublisherKey {
    type Err = NameError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 52 {
            return Err(error("publisher key", s, "length must be 52"));
        }
        if s.bytes().any(|b| b.is_ascii_uppercase()) {
            return Err(error("publisher key", s, "uppercase is forbidden"));
        }
        let mapped: Option<Vec<u8>> = s
            .bytes()
            .map(|b| ZBASE32.iter().position(|&c| c == b).map(|i| BASE32[i]))
            .collect();
        let mapped = mapped.ok_or_else(|| error("publisher key", s, "invalid alphabet"))?;
        let bytes = data_encoding::BASE32_NOPAD
            .decode(&mapped)
            .map_err(|_| error("publisher key", s, "unused bits must be zero"))?;
        let bytes = bytes
            .try_into()
            .map_err(|_| error("publisher key", s, "length must be 52"))?;
        Ok(Self(bytes))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageId([u8; 32]);
impl PackageId {
    pub fn of(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }
    pub fn from_bytes(b: [u8; 32]) -> Self {
        Self(b)
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl FromStr for PackageId {
    type Err = NameError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || error("package id", s, "use exactly 64 lowercase hex characters");
        if s.len() != 64 {
            return Err(invalid());
        }
        let bytes = data_encoding::HEXLOWER
            .decode(s.as_bytes())
            .map_err(|_| invalid())?;
        Ok(Self(bytes.try_into().map_err(|_| invalid())?))
    }
}
impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&data_encoding::HEXLOWER.encode(&self.0))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReleaseRef {
    pub publisher: PublisherKey,
    pub slug: Slug,
    pub version: Version,
}
impl FromStr for ReleaseRef {
    type Err = NameError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || error("release reference", s, "use publisher/slug@version");
        let (publisher, rest) = s.split_once('/').ok_or_else(invalid)?;
        let (slug, version) = rest.split_once('@').ok_or_else(invalid)?;
        Ok(Self {
            publisher: publisher.parse().map_err(|_| invalid())?,
            slug: slug.parse().map_err(|_| invalid())?,
            version: version.parse().map_err(|_| invalid())?,
        })
    }
}
impl fmt::Display for ReleaseRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}@{}", self.publisher, self.slug, self.version)
    }
}
text_serde!(
    Slug,
    Version,
    InterfaceName,
    HttpHost,
    PublisherKey,
    PackageId,
    ReleaseRef
);
