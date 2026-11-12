use crate::*;
use std::path::PathBuf;
#[derive(Debug, thiserror::Error)]
pub enum DistError {
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
    Log(#[from] watane_log::Error),
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
    #[error("this link is not your channel: found {found}")]
    NotOwnLink { found: PublicKey },
    #[error("channel {channel} is not followed")]
    NotFollowed { channel: PublicKey },
    #[error("channel {channel} compromised at seq {seq}")]
    Compromised { channel: PublicKey, seq: u64 },
    #[error("invalid publisher key {key}: {cause}")]
    BadKey {
        key: PublisherKey,
        cause: KeyTextError,
    },
    #[error("channel {channel} has no release of {slug}")]
    NoRelease { channel: PublicKey, slug: Slug },
    #[error("unknown release {release}")]
    UnknownRelease { release: ReleaseRef },
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
    #[error("{}: expected channel {expected}, found {found}", url.escape_debug())]
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
    #[error("channel {channel}: too many entries from {} to {served}", stored.map(|seq| seq.to_string()).unwrap_or_else(|| "none".into()))]
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
