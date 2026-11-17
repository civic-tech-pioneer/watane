//! Grants, providers and requirements (TECHNICAL 5): what a mod may be given and
//! the version rule a provider must satisfy.

use crate::{HttpHost, InterfaceName, PublisherKey, Slug, Version, compatible};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Provider {
    pub slug: Slug,
    pub publisher: Option<PublisherKey>,
}
impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.publisher {
            Some(key) => write!(f, "{key}/{}", self.slug),
            None => write!(f, "local/{}", self.slug),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", try_from = "RawGrant")]
pub enum Grant {
    Storage,
    Http {
        host: HttpHost,
    },
    Call {
        interface: InterfaceName,
        provider: Provider,
    },
    Subscribe {
        interface: InterfaceName,
        provider: Provider,
    },
    Manage,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum RawGrant {
    Storage,
    Http {
        host: HttpHost,
    },
    Call {
        interface: InterfaceName,
        provider: Provider,
    },
    Subscribe {
        interface: InterfaceName,
        provider: Provider,
    },
    Manage,
}
impl TryFrom<RawGrant> for Grant {
    type Error = &'static str;
    fn try_from(raw: RawGrant) -> Result<Self, Self::Error> {
        let grant = match raw {
            RawGrant::Storage => Self::Storage,
            RawGrant::Manage => Self::Manage,
            RawGrant::Http { host } => Self::Http { host },
            RawGrant::Call {
                interface,
                provider,
            } => Self::Call {
                interface,
                provider,
            },
            RawGrant::Subscribe {
                interface,
                provider,
            } => Self::Subscribe {
                interface,
                provider,
            },
        };
        if let Self::Call {
            interface,
            provider,
        }
        | Self::Subscribe {
            interface,
            provider,
        } = &grant
            && (interface.namespace() != "mods" || interface.package() != provider.slug.as_str())
        {
            return Err("grant interface must be mods:<provider slug>/interface@version");
        }
        Ok(grant)
    }
}
impl fmt::Display for Grant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage => f.write_str("storage"),
            Self::Manage => f.write_str("manage"),
            Self::Http { host } => write!(f, "http {host}"),
            Self::Call {
                interface,
                provider,
            } => write!(f, "call {interface} from {provider}"),
            Self::Subscribe {
                interface,
                provider,
            } => write!(f, "subscribe {interface} from {provider}"),
        }
    }
}
impl Grant {
    pub fn same_authority(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Call {
                    interface: a,
                    provider: p,
                },
                Self::Call {
                    interface: b,
                    provider: q,
                },
            )
            | (
                Self::Subscribe {
                    interface: a,
                    provider: p,
                },
                Self::Subscribe {
                    interface: b,
                    provider: q,
                },
            ) => {
                p == q
                    && a.namespace() == b.namespace()
                    && a.package() == b.package()
                    && a.interface() == b.interface()
            }
            _ => self == other,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Requirement {
    pub provider: Provider,
    pub version: Version,
}
impl Requirement {
    pub fn satisfied_by(&self, provider: &Provider, version: &Version) -> bool {
        self.provider == *provider && compatible(&self.version, version)
    }
}
