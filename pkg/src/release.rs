use crate::{
    Grant, InterfaceName, Mod, NameError, Package, PackageId, Provider, PublisherKey, ReleaseRef,
    Slug, Version,
};
use minicbor::{Decoder, Encoder, data::Type};
use std::convert::Infallible;

pub const RELEASE_KIND: &str = "release";
pub const MAX_RELEASE_BYTES: usize = 32_768;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub slug: Slug,
    pub version: Version,
    pub name: String,
    pub summary: String,
    pub package: PackageId,
    pub size: u64,
    pub source: bool,
    pub offers: Vec<InterfaceName>,
    pub events: Vec<InterfaceName>,
    pub grants: Vec<Grant>,
    pub origin: Option<ReleaseRef>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReleaseError {
    #[error("the mod depends on the local provider {slug}; publish it first")]
    LocalProvider { slug: Slug },
    #[error("bad release body: {0}")]
    Malformed(String),
    #[error("the release body is not in canonical form")]
    NotCanonical,
    #[error("the release body is {size} bytes; the limit is 32768")]
    TooLarge { size: usize },
    #[error("the release and the package differ in `{field}`")]
    Mismatch { field: &'static str },
}

impl Release {
    pub fn new(m: &Mod, package: &Package) -> Result<Self, ReleaseError> {
        let grants = m.grants();
        for grant in &grants {
            if let Grant::Call { provider, .. } | Grant::Subscribe { provider, .. } = grant
                && provider.publisher.is_none()
            {
                return Err(ReleaseError::LocalProvider {
                    slug: provider.slug.clone(),
                });
            }
        }
        Ok(Self {
            slug: m.slug().clone(),
            version: m.version().clone(),
            name: m.name().into(),
            summary: m.summary().into(),
            package: package.id(),
            size: package.len(),
            source: package.has_source(),
            offers: m.world().exports.clone(),
            events: m.world().events.clone(),
            grants,
            origin: m.origin().cloned(),
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut encoder = Encoder::new(Vec::new());
        // Vec's writer is infallible; only primitive CBOR operations are used.
        let _ = self.encode_into(&mut encoder);
        encoder.into_writer()
    }

    fn encode_into(
        &self,
        e: &mut Encoder<Vec<u8>>,
    ) -> Result<(), minicbor::encode::Error<Infallible>> {
        e.array(12)?
            .u8(1)?
            .str(self.slug.as_str())?
            .str(&self.version.to_string())?
            .str(&self.name)?
            .str(&self.summary)?
            .bytes(self.package.as_bytes())?
            .u64(self.size)?
            .bool(self.source)?;
        for interfaces in [&self.offers, &self.events] {
            e.array(interfaces.len() as u64)?;
            for interface in interfaces {
                e.str(&interface.to_string())?;
            }
        }
        e.array(self.grants.len() as u64)?;
        for grant in &self.grants {
            match grant {
                Grant::Storage => {
                    e.array(1)?.u8(0)?;
                }
                Grant::Http { host } => {
                    e.array(2)?.u8(1)?.str(host.as_str())?;
                }
                Grant::Call {
                    interface,
                    provider,
                }
                | Grant::Subscribe {
                    interface,
                    provider,
                } => {
                    e.array(3)?
                        .u8(if matches!(grant, Grant::Call { .. }) {
                            2
                        } else {
                            3
                        })?
                        .str(&interface.to_string())?;
                    match &provider.publisher {
                        Some(key) => {
                            e.bytes(key.as_bytes())?;
                        }
                        None => {
                            e.null()?;
                        }
                    }
                }
            }
        }
        match &self.origin {
            Some(origin) => {
                e.array(3)?
                    .bytes(origin.publisher.as_bytes())?
                    .str(origin.slug.as_str())?
                    .str(&origin.version.to_string())?;
            }
            None => {
                e.null()?;
            }
        }
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ReleaseError> {
        if bytes.len() > MAX_RELEASE_BYTES {
            return Err(ReleaseError::TooLarge { size: bytes.len() });
        }
        let mut r = Reader(Decoder::new(bytes));
        r.exact_array(12, "release")?;
        if r.0.u64().map_err(|e| malformed(format!("format: {e}")))? != 1 {
            return Err(malformed("format must be 1"));
        }
        let slug: Slug = r.name("slug")?;
        let version: Version = r.name("version")?;
        let name = r.text("name")?.to_owned();
        let summary = r.text("summary")?.to_owned();
        for (field, text, max) in [("name", &name, 64), ("summary", &summary, 280)] {
            if let Some(message) = crate::meta::text_violation(field, text, max) {
                return Err(malformed(message));
            }
        }
        let package = PackageId::from_bytes(r.key("package id")?);
        let size = r.0.u64().map_err(|e| malformed(format!("size: {e}")))?;
        let source = r.0.bool().map_err(|e| malformed(format!("source: {e}")))?;
        let offers = r.interfaces("offers", "offer", &slug, &version)?;
        let events = r.interfaces("events", "event", &slug, &version)?;
        let count = r.array(81, "grants")?;
        let mut grants = Vec::new();
        for _ in 0..count {
            let len = r.array(3, "grant")?;
            let tag =
                r.0.u64()
                    .map_err(|e| malformed(format!("grant kind: {e}")))?;
            let expected = match tag {
                0 => 1,
                1 => 2,
                2 | 3 => 3,
                _ => return Err(malformed("unknown grant kind")),
            };
            if len != expected {
                return Err(malformed("grant array has wrong length"));
            }
            let grant = match tag {
                0 => Grant::Storage,
                1 => Grant::Http {
                    host: r.name("host")?,
                },
                _ => {
                    let interface: InterfaceName = r.name("interface")?;
                    if interface.namespace() != "mods" || interface.package() == slug.as_str() {
                        return Err(malformed("grant interface must be mods: of another slug"));
                    }
                    let provider_slug: Slug = interface
                        .package()
                        .parse()
                        .map_err(|e: NameError| malformed(e.to_string()))?;
                    if r.is_null()? {
                        return Err(ReleaseError::LocalProvider {
                            slug: provider_slug,
                        });
                    }
                    let provider = Provider {
                        slug: provider_slug,
                        publisher: Some(PublisherKey::from_bytes(r.key("grant publisher")?)),
                    };
                    if tag == 2 {
                        Grant::Call {
                            interface,
                            provider,
                        }
                    } else {
                        Grant::Subscribe {
                            interface,
                            provider,
                        }
                    }
                }
            };
            grants.push(grant);
        }
        ascending(&grants, "grants")?;
        let origin = if r.is_null()? {
            r.0.null().map_err(|e| malformed(format!("origin: {e}")))?;
            None
        } else {
            r.exact_array(3, "origin")?;
            Some(ReleaseRef {
                publisher: PublisherKey::from_bytes(r.key("origin publisher")?),
                slug: r.name("origin slug")?,
                version: r.name("origin version")?,
            })
        };
        let result = Self {
            slug,
            version,
            name,
            summary,
            package,
            size,
            source,
            offers,
            events,
            grants,
            origin,
        };
        if result.encode() != bytes {
            return Err(ReleaseError::NotCanonical);
        }
        Ok(result)
    }

    pub fn check(&self, m: &Mod, package: &Package) -> Result<(), ReleaseError> {
        let expected = Self::new(m, package)?;
        macro_rules! compare {
            ($($field:ident),+ $(,)?) => { $(if self.$field != expected.$field { return Err(ReleaseError::Mismatch { field: stringify!($field) }); })+ };
        }
        compare!(
            slug, version, name, summary, package, size, source, offers, events, grants, origin
        );
        Ok(())
    }
}

fn malformed(message: impl Into<String>) -> ReleaseError {
    ReleaseError::Malformed(message.into())
}
fn ascending<T: Ord>(values: &[T], field: &str) -> Result<(), ReleaseError> {
    if values.windows(2).any(|w| w[0] >= w[1]) {
        return Err(malformed(format!("{field} must be strictly ascending")));
    }
    Ok(())
}

// Each collection length is bounded before iteration or allocation. No recursive CBOR parsing.
struct Reader<'a>(Decoder<'a>);
impl<'a> Reader<'a> {
    fn interfaces(
        &mut self,
        field: &str,
        item: &str,
        slug: &Slug,
        version: &Version,
    ) -> Result<Vec<InterfaceName>, ReleaseError> {
        let count = self.array(32, field)?;
        let mut list: Vec<InterfaceName> = Vec::new();
        for _ in 0..count {
            let interface: InterfaceName = self.name(item)?;
            if interface.namespace() != "mods"
                || interface.package() != slug.as_str()
                || interface.version() != *version
            {
                return Err(malformed(format!(
                    "{field} must belong to mods:{slug}@{version}"
                )));
            }
            list.push(interface);
        }
        ascending(&list, field)?;
        Ok(list)
    }

    fn array(&mut self, max: u64, field: &str) -> Result<u64, ReleaseError> {
        let len = self
            .0
            .array()
            .map_err(|e| malformed(format!("{field}: {e}")))?
            .ok_or(ReleaseError::NotCanonical)?;
        if len > max {
            return Err(malformed(format!("{field} array exceeds {max} items")));
        }
        Ok(len)
    }
    fn exact_array(&mut self, count: u64, field: &str) -> Result<(), ReleaseError> {
        if self.array(count, field)? != count {
            return Err(malformed(format!(
                "{field} must be an array of {count} items"
            )));
        }
        Ok(())
    }
    fn definite(&self, field: &str) -> Result<(), ReleaseError> {
        match self
            .0
            .datatype()
            .map_err(|e| malformed(format!("{field}: {e}")))?
        {
            Type::StringIndef | Type::BytesIndef => Err(ReleaseError::NotCanonical),
            _ => Ok(()),
        }
    }
    fn text(&mut self, field: &str) -> Result<&'a str, ReleaseError> {
        self.definite(field)?;
        self.0.str().map_err(|e| malformed(format!("{field}: {e}")))
    }
    fn name<T: std::str::FromStr<Err = NameError>>(
        &mut self,
        field: &str,
    ) -> Result<T, ReleaseError> {
        self.text(field)?
            .parse()
            .map_err(|e: NameError| malformed(format!("{field}: {e}")))
    }
    fn key(&mut self, field: &str) -> Result<[u8; 32], ReleaseError> {
        self.definite(field)?;
        self.0
            .bytes()
            .map_err(|e| malformed(format!("{field}: {e}")))?
            .try_into()
            .map_err(|_| malformed(format!("{field} must be 32 bytes")))
    }
    fn is_null(&self) -> Result<bool, ReleaseError> {
        Ok(self.0.datatype().map_err(|e| malformed(e.to_string()))? == Type::Null)
    }
}
