use crate::*;
use std::path::PathBuf;
#[derive(Debug, thiserror::Error)]
pub enum DistError {
    #[error("this home holds no identity; it can read and store channels but never sign")]
    NoIdentity,
    #[error("own head {seq} is not stored")]
    HeadNotStored { seq: u64 },
    #[error("channel {} has a gap at {seq}", channel.fingerprint())]
    Gap { channel: PublicKey, seq: u64 },
    #[error("no live stored release names package {id}")]
    NotNamed { id: PackageId },
    #[error("at most {max} kept channels are allowed")]
    TooManyKept { max: usize },
    #[error("home is locked at {}", path.display().to_string().escape_debug())]
    Locked { path: PathBuf },
    #[error("{}", path.display().to_string().escape_debug())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("corrupt {}: {}", path.display().to_string().escape_debug(), cause.escape_debug())]
    Corrupt { path: PathBuf, cause: String },
    #[error(transparent)]
    Log(watane_log::Error),
    #[error(transparent)]
    Package(#[from] PackageError),
    #[error(transparent)]
    Release(#[from] ReleaseError),
    #[error("statement refused")]
    Refused,
    #[error("confirm the restored own head before signing")]
    OwnHeadUnconfirmed,
    #[error("invalid name: {}", cause.escape_debug())]
    BadName { cause: &'static str },
    #[error("invalid reason: {}", cause.escape_debug())]
    BadReason { cause: &'static str },
    #[error("release {release} already exists at seq {seq}")]
    VersionExists { release: ReleaseRef, seq: u64 },
    #[error("publish provider {provider} first")]
    UnreleasedProvider { provider: Slug },
    #[error("package has {size} bytes; limit {max}")]
    PackageTooLarge { size: u64, max: u64 },
    #[error("release {release} is not in the own channel")]
    NotOwnRelease { release: ReleaseRef },
    #[error("release {release} already withdrawn at seq {seq}")]
    AlreadyWithdrawn { release: ReleaseRef, seq: u64 },
    #[error("entry {entry} stored but site not written: {cause}")]
    SiteNotWritten {
        entry: EntryId,
        cause: Box<DistError>,
    },
    #[error("export conflict at {}: {}", path.display().to_string().escape_debug(), cause.escape_debug())]
    ExportConflict { path: PathBuf, cause: &'static str },
    #[error("no site URL configured")]
    NoSiteUrl,
    #[error("invalid link: {}", cause.escape_debug())]
    BadLink { cause: &'static str },
    #[error("invalid URL {}: {}", url.escape_debug(), cause.escape_debug())]
    BadUrl { url: String, cause: &'static str },
    #[error("cannot follow the own channel")]
    OwnChannel,
    #[error("this link is not your channel: found {}", found.fingerprint())]
    NotOwnLink { found: PublicKey },
    #[error(
        "channel {} is not followed; {}; follow the publisher's link to settle it",
        channel.fingerprint(),
        if *local_only {
            "the listings that name a base for it point at a local address"
        } else {
            "no followed channel, chosen index or listing names a base for it"
        }
    )]
    NotFollowed {
        channel: PublicKey,
        local_only: bool,
    },
    #[error("channel {} is not followed, so there is nothing to unfollow", channel.fingerprint())]
    NothingToUnfollow { channel: PublicKey },
    #[error("channel {} compromised at seq {seq}", channel.fingerprint())]
    Compromised { channel: PublicKey, seq: u64 },
    #[error("invalid publisher key {key}: {cause}")]
    BadKey {
        key: PublisherKey,
        cause: KeyTextError,
    },
    #[error("channel {} has no release of {slug}", channel.fingerprint())]
    NoRelease { channel: PublicKey, slug: Slug },
    #[error("unknown release {release}")]
    UnknownRelease { release: ReleaseRef },
    #[error(
        "release {release} is not on any base that serves its channel: the {bases} base(s) read \
         hold an older copy of the channel; follow the publisher's own link, or try again when the \
         mirrors catch up"
    )]
    StaleBases { release: ReleaseRef, bases: usize },
    #[error("release {release} withdrawn: {}", reason.escape_debug())]
    Withdrawn { release: ReleaseRef, reason: String },
    #[error("not found: {}", url.escape_debug())]
    NotFound { url: String },
    #[error("HTTP {status} from {}", url.escape_debug())]
    Http { url: String, status: u16 },
    #[error("request timed out: {}", url.escape_debug())]
    Timeout { url: String },
    #[error("body from {} exceeds {limit} bytes", url.escape_debug())]
    TooLarge { url: String, limit: u64 },
    #[error("network error at {}: {}", url.escape_debug(), cause.escape_debug())]
    Network { url: String, cause: String },
    #[error("invalid entry from {}: {cause}", url.escape_debug())]
    Invalid {
        url: String,
        cause: watane_log::DecodeError,
    },
    #[error("{}: expected channel {}, found {}", url.escape_debug(), expected.fingerprint(), found.fingerprint())]
    WrongChannel {
        url: String,
        expected: PublicKey,
        found: PublicKey,
    },
    #[error("{}: expected seq {expected}, found {found}", url.escape_debug())]
    WrongSeq {
        url: String,
        expected: u64,
        found: u64,
    },
    #[error("entry from {} differs from head", url.escape_debug())]
    HeadMismatch { url: String },
    #[error("channel {}: too many entries from {} to {served}", channel.fingerprint(), stored.map(|seq| seq.to_string()).unwrap_or_else(|| "none".into()))]
    TooManyEntries {
        channel: PublicKey,
        stored: Option<u64>,
        served: u64,
    },
    #[error("{}: hash mismatch: expected {expected}, found {found}", url.escape_debug())]
    HashMismatch {
        url: String,
        expected: PackageId,
        found: PackageId,
    },
    #[error("invalid package from {}: {}", url.escape_debug(), cause.to_string().escape_debug())]
    BadPackage { url: String, cause: PackageError },
    #[error("package mismatch at {}: {}", url.escape_debug(), cause.to_string().escape_debug())]
    PackageMismatch { url: String, cause: ReleaseError },
    #[error("invalid note: {}", cause.escape_debug())]
    BadNote { cause: &'static str },
    #[error("invalid query: {}", cause.escape_debug())]
    BadQuery { cause: &'static str },
    #[error("cannot list the own channel")]
    OwnListing,
    #[error("already listed at seq {seq}")]
    AlreadyListed { seq: u64 },
    #[error("entry {entry} is not a live listing")]
    NotListed { entry: EntryId },
    #[error("channel {} is not a chosen index", channel.fingerprint())]
    NotIndex { channel: PublicKey },
}

impl From<watane_log::Error> for DistError {
    fn from(error: watane_log::Error) -> Self {
        match error {
            watane_log::Error::NoIdentity => Self::NoIdentity,
            other => Self::Log(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac2_channel_errors_show_fingerprints() {
        let key: PublicKey = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy"
            .parse()
            .unwrap();
        let other = PublicKey::from_bytes(&[1; 32]).unwrap();
        let cases = [
            DistError::NotIndex { channel: key },
            DistError::WrongChannel {
                url: "http://example.org/head".into(),
                expected: key,
                found: other,
            },
            DistError::NotOwnLink { found: key },
            DistError::NotFollowed {
                channel: key,
                local_only: false,
            },
            DistError::NotFollowed {
                channel: key,
                local_only: true,
            },
            DistError::NothingToUnfollow { channel: key },
            DistError::Compromised {
                channel: key,
                seq: 1,
            },
            DistError::NoRelease {
                channel: key,
                slug: "reading-list".parse().unwrap(),
            },
            DistError::TooManyEntries {
                channel: key,
                stored: None,
                served: 2,
            },
        ];
        for error in cases {
            let text = error.to_string();
            assert!(text.contains(&key.fingerprint()), "{text}");
            assert!(!text.contains(&key.to_string()), "{text}");
            if let DistError::WrongChannel { found, .. } = error {
                assert!(text.contains(&found.fingerprint()), "{text}");
                assert!(!text.contains(&found.to_string()), "{text}");
            }
        }
    }

    #[test]
    fn ac1_nothing_to_unfollow_text() {
        let key = PublicKey::from_bytes(&[1; 32]).unwrap();
        assert_eq!(
            DistError::NothingToUnfollow { channel: key }.to_string(),
            format!(
                "channel {} is not followed, so there is nothing to unfollow",
                key.fingerprint()
            )
        );
    }

    fn chain(error: &dyn std::error::Error) -> String {
        let mut text = error.to_string();
        let mut source = error.source();
        while let Some(error) = source {
            text.push_str(": ");
            text.push_str(&error.to_string());
            source = error.source();
        }
        text
    }

    #[test]
    fn ac1_s21c_untrusted_text_is_escaped_unquoted() {
        let cases = [
            (
                DistError::Locked {
                    path: "/h/\n\u{1b}".into(),
                },
                r"home is locked at /h/\n\u{1b}",
            ),
            (
                DistError::Corrupt {
                    path: "/h/\n".into(),
                    cause: "bad\u{1b}".into(),
                },
                r"corrupt /h/\n: bad\u{1b}",
            ),
            (
                DistError::Network {
                    url: "http://x/\n".into(),
                    cause: "bad\u{1b}".into(),
                },
                r"network error at http://x/\n: bad\u{1b}",
            ),
            (
                DistError::BadName { cause: "bad\n" },
                r"invalid name: bad\n",
            ),
            (
                DistError::BadPackage {
                    url: "http://x/\n".into(),
                    cause: PackageError::TooLarge { size: 67108865 },
                },
                r"invalid package from http://x/\n: the package is 67108865 bytes; the limit is 67108864",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn ac2_s21c_wrappers_do_not_repeat_sources() {
        let log = watane_log::Error::EmptyPassphrase;
        let expected = log.to_string();
        assert_eq!(chain(&DistError::Log(log)), expected);
        let release = ReleaseError::NotCanonical;
        let expected = release.to_string();
        assert_eq!(chain(&DistError::Release(release)), expected);
        let error = DistError::SiteNotWritten {
            entry: "0000000000000000000000000000000000000000000000000000000000000000"
                .parse()
                .unwrap(),
            cause: Box::new(DistError::Refused),
        };
        assert!(std::error::Error::source(&error).is_none());
        assert!(error.to_string().ends_with(": statement refused"));
    }

    #[test]
    fn ac3_s21c_io_not_found_and_package_chain() {
        let error = DistError::Io {
            path: "/h/dist/site/head".into(),
            source: std::io::Error::other("disk broke"),
        };
        assert_eq!(error.to_string(), "/h/dist/site/head");
        assert_eq!(chain(&error), "/h/dist/site/head: disk broke");
        assert_eq!(chain(&error).matches("disk broke").count(), 1);
        assert_eq!(
            DistError::NotFound {
                url: "http://x.invalid/a".into()
            }
            .to_string(),
            "not found: http://x.invalid/a"
        );
        let cause = PackageError::TooLarge { size: 67108865 };
        let expected = cause.to_string();
        let text = chain(&DistError::Package(cause));
        assert_eq!(text, expected);
        assert!(!text.contains(['{', '(']));
    }

    #[test]
    fn ac1_s21b_head_mismatch_escapes_url() {
        let error = DistError::HeadMismatch {
            url: "http://example.invalid/\u{1b}\nentries/2".into(),
        };
        let text = error.to_string();
        assert!(text.contains("http://example.invalid/\\u{1b}\\nentries/2"));
        assert!(!text.contains(['\u{1b}', '\n']));
    }

    #[test]
    fn ac3_stale_bases_text() {
        let key = PublicKey::from_bytes(&[1; 32]).unwrap();
        let error = DistError::StaleBases {
            release: ReleaseRef {
                publisher: PublisherKey::from_bytes(*key.as_bytes()),
                slug: "ratings".parse().unwrap(),
                version: "1.0.0".parse().unwrap(),
            },
            bases: 1,
        };
        let release = match &error {
            DistError::StaleBases { release, .. } => release.to_string(),
            _ => unreachable!(),
        };
        assert_eq!(
            error.to_string(),
            format!(
                "release {release} is not on any base that serves its channel: the 1 base(s) \
                 read hold an older copy of the channel; follow the publisher's own link, or \
                 try again when the mirrors catch up"
            )
        );
    }
}
