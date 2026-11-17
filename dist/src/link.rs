//! `Link` (TECHNICAL 4.2): parse and format the text that points at a channel,
//! its web base and, optionally, one mod.

use crate::{DistError, PublicKey, Slug};
use std::fmt;
use std::str::FromStr;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    channel: PublicKey,
    base: url::Url,
    slug: Option<Slug>,
}
impl Link {
    pub fn new(channel: PublicKey, base: &str, slug: Option<&Slug>) -> Result<Self, DistError> {
        let bad = |cause| DistError::BadUrl {
            url: base.into(),
            cause,
        };
        let mut url = url::Url::parse(base).map_err(|_| bad("invalid absolute URL"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(bad("scheme must be http or https"));
        }
        if url.host_str().is_none() {
            return Err(bad("host required"));
        }
        if !url.username().is_empty()
            || url.password().is_some()
            || base.split_once("://").is_some_and(|(_, rest)| {
                rest.split(['/', '?', '#'])
                    .next()
                    .is_some_and(|authority| authority.contains('@'))
            })
        {
            return Err(bad("userinfo refused"));
        }
        if url.query().is_some() {
            return Err(bad("query refused"));
        }
        if url.fragment().is_some() {
            return Err(bad("fragment refused"));
        }
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        let link = Self {
            channel,
            base: url,
            slug: slug.cloned(),
        };
        if link.to_string().len() > 2048 {
            return Err(DistError::BadLink {
                cause: "link exceeds 2048 bytes",
            });
        }
        Ok(link)
    }
    pub fn channel(&self) -> PublicKey {
        self.channel
    }
    pub fn base(&self) -> &str {
        self.base.as_str()
    }
    pub fn slug(&self) -> Option<&Slug> {
        self.slug.as_ref()
    }
}
impl FromStr for Link {
    type Err = DistError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = |cause| DistError::BadLink { cause };
        if s.len() > 2048 {
            return Err(bad("link exceeds 2048 bytes"));
        }
        let rest = s
            .strip_prefix("watane:")
            .ok_or(bad("missing watane: prefix"))?;
        let (key, rest) = rest.split_once('@').ok_or(bad("missing @ separator"))?;
        let channel = key.parse().map_err(|_| bad("invalid channel key"))?;
        let (base, slug) = match rest.split_once('#') {
            Some((base, slug)) => (
                base,
                Some(slug.parse::<Slug>().map_err(|_| bad("invalid slug"))?),
            ),
            None => (rest, None),
        };
        Self::new(channel, base, slug.as_ref())
    }
}
impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "watane:{}@{}", self.channel, self.base)?;
        if let Some(slug) = &self.slug {
            write!(f, "#{slug}")?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const KEY: &str = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy";
    #[test]
    fn ac1_link_table() {
        let valid = [
            (
                "https://alice.example.org/mods",
                "https://alice.example.org/mods/",
                None,
            ),
            (
                "https://alice.example.org/mods/#reading-list",
                "https://alice.example.org/mods/",
                Some("reading-list"),
            ),
            ("http://localhost", "http://localhost/", None),
            ("http://127.0.0.1:8080/x", "http://127.0.0.1:8080/x/", None),
            ("https://[::1]/", "https://[::1]/", None),
            ("https://EXAMPLE.org", "https://example.org/", None),
            ("https://example.org:443/", "https://example.org/", None),
            ("http://example.org:80/", "http://example.org/", None),
            ("https://example.org/a/b", "https://example.org/a/b/", None),
            (
                "https://example.org/a%20b",
                "https://example.org/a%20b/",
                None,
            ),
            ("https://example.org/#a", "https://example.org/", Some("a")),
            (
                "http://example.org/#abc-b2",
                "http://example.org/",
                Some("abc-b2"),
            ),
        ];
        for (tail, base, slug) in valid {
            let l: Link = format!("watane:{KEY}@{tail}").parse().unwrap();
            assert_eq!(l.channel().to_string(), KEY);
            assert_eq!(l.base(), base);
            assert_eq!(l.slug().map(Slug::as_str), slug);
            assert_eq!(l.to_string().parse::<Link>().unwrap(), l);
        }
        let channel = KEY.parse().unwrap();
        assert!(matches!(
            Link::new(channel, "https://example.org/#fragment", None),
            Err(DistError::BadUrl {
                cause: "fragment refused",
                ..
            })
        ));
        Link::new(channel, "https://@example.org", None).unwrap_err();
        let invalid = [
            (
                format!("{KEY}@https://example.org"),
                "missing watane: prefix",
            ),
            (
                format!("Watane:{KEY}@https://example.org"),
                "missing watane: prefix",
            ),
            (
                "watane:bad@https://example.org".into(),
                "invalid channel key",
            ),
            (
                format!("watane:{KEY}https://example.org"),
                "missing @ separator",
            ),
            (
                format!("watane:{KEY}@https://u@example.org"),
                "userinfo refused",
            ),
            (
                format!("watane:{KEY}@https://u:p@example.org"),
                "userinfo refused",
            ),
            (
                format!("watane:{KEY}@https://example.org/?q=1"),
                "query refused",
            ),
            (
                format!("watane:{KEY}@ftp://example.org"),
                "scheme must be http or https",
            ),
            (format!("watane:{KEY}@https://"), "invalid absolute URL"),
            (format!("watane:{KEY}@/relative"), "invalid absolute URL"),
            (
                format!("watane:{KEY}@https://example.org/#"),
                "invalid slug",
            ),
            (
                format!("watane:{KEY}@https://example.org/#Bad"),
                "invalid slug",
            ),
            (
                format!("watane:{KEY}@https://example.org/#a#b"),
                "invalid slug",
            ),
            (
                format!("watane:{KEY}@https://example.org/{}", "a".repeat(2048)),
                "link exceeds 2048 bytes",
            ),
        ];
        for (s, expected) in invalid {
            match s.parse::<Link>().unwrap_err() {
                DistError::BadLink { cause } | DistError::BadUrl { cause, .. } => {
                    assert_eq!(cause, expected, "{s}")
                }
                e => panic!("{e}"),
            }
        }
    }
}
