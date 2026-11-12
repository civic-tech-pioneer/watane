use crate::*;
use std::path::PathBuf;
#[derive(Debug, thiserror::Error)]
pub enum DistError {
    #[error("home is locked at {path:?}")]
    Locked { path: PathBuf },
    #[error("{path:?}: {source:?}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("corrupt {path:?}: {cause:?}")]
    Corrupt { path: PathBuf, cause: String },
    #[error("{0}")]
    Log(#[from] watane_log::Error),
    #[error("invalid package: {0:?}")]
    Package(#[from] PackageError),
    #[error("invalid release: {0:?}")]
    Release(#[from] ReleaseError),
    #[error("statement refused")]
    Refused,
    #[error("confirm the restored own head before signing")]
    OwnHeadUnconfirmed,
    #[error("invalid name: {cause:?}")]
    BadName { cause: &'static str },
    #[error("invalid reason: {cause:?}")]
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
    #[error("export conflict at {path:?}: {cause:?}")]
    ExportConflict { path: PathBuf, cause: &'static str },
    #[error("no site URL configured")]
    NoSiteUrl,
    #[error("invalid link: {cause:?}")]
    BadLink { cause: &'static str },
    #[error("invalid URL {url:?}: {cause:?}")]
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
    #[error("release {release} withdrawn: {reason:?}")]
    Withdrawn { release: ReleaseRef, reason: String },
    #[error("not found: {url:?}")]
    NotFound { url: String },
    #[error("HTTP {status} from {url:?}")]
    Http { url: String, status: u16 },
    #[error("request timed out: {url:?}")]
    Timeout { url: String },
    #[error("body from {url:?} exceeds {limit} bytes")]
    TooLarge { url: String, limit: u64 },
    #[error("network error at {url:?}: {cause:?}")]
    Network { url: String, cause: String },
    #[error("invalid entry from {url:?}: {cause}")]
    Invalid {
        url: String,
        cause: watane_log::DecodeError,
    },
    #[error("{url:?}: expected channel {expected}, found {found}")]
    WrongChannel {
        url: String,
        expected: PublicKey,
        found: PublicKey,
    },
    #[error("{url:?}: expected seq {expected}, found {found}")]
    WrongSeq {
        url: String,
        expected: u64,
        found: u64,
    },
    #[error("entry from {url:?} differs from head")]
    HeadMismatch { url: String },
    #[error("channel {channel}: too many entries from {stored:?} to {served}")]
    TooManyEntries {
        channel: PublicKey,
        stored: Option<u64>,
        served: u64,
    },
    #[error("{url:?}: hash mismatch: expected {expected}, found {found}")]
    HashMismatch {
        url: String,
        expected: PackageId,
        found: PackageId,
    },
    #[error("invalid package from {url:?}: {cause:?}")]
    BadPackage { url: String, cause: PackageError },
    #[error("package mismatch at {url:?}: {cause:?}")]
    PackageMismatch { url: String, cause: ReleaseError },
}

#[cfg(test)]
mod tests {
    use super::*;

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
