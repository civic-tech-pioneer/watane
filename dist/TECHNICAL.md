# dist: technical specification

How `dist` does what `FUNCTIONAL.md` says. Requirements are cited as
`DIST-F-<n>`. Section 13 lists the decisions made here; section 12 lists
the questions for `spec/`.

## 1. Crate

| Item | Value |
|---|---|
| Package | `watane-dist`, lib `watane_dist`, no binary |
| Edition, toolchain | 2024, stable, `rust-version = "1.91"` (needs `File::try_lock`, stable since 1.89) |
| Lints | `unsafe_code = "forbid"`; `cargo clippy --all-targets -- -D warnings` is clean |
| Threads | No thread, no timer, no async runtime. Every call is blocking. |
| Network | Outgoing HTTP GET only. `dist` never listens and never uploads. |

```text
dist/
  Cargo.toml
  src/
    lib.rs        Dist, Options; re-exports; the only `pub` items (section 3)
    error.rs      DistError
    confirm.rs    Confirm, Decision, SignRequest, Statement, Profile, Withdraw, Listing
    link.rs       Link: parse and format (section 4.2)
    body.rs       profile, withdraw, list, include and unlist bodies; text rules (section 4.1)
    view.rs       derive(): entries -> ChannelView; the statement rules (section 4.1)
    own.rs        identity, sign_and_append, publish, withdraw, set_name, restore; list, include, unlist
    site.rs       export: the site folder (section 4.3); atomic file writes
    http.rs       Reader: GET with limits and timeouts (section 4.5)
    follow.rs     follows.json, site.json, read_channel, follow, refresh
    fetch.rs      package cache, unpacked() (section 5.2), fetch, open_link
    index.rs      the read set (5.9); choose_index, drop_index, indexes             (step 4)
    search.rs     Query, InterfaceQuery, Hit; search (5.10)                       (step 4)
  tests/
    support/mod.rs  TestHost, RecordingConfirm, fixtures
    fixtures/reading-list/   a page-only mod folder (mod.toml, wit/world.wit, ui/index.html)
    site.rs  follow.rs  fetch.rs  restore.rs  j3_dist.rs
    index.rs  search.rs  j4_dist.rs                                             (step 4)
```

Inner modules are private (`mod x;`, not `pub mod`). Only `lib.rs` exports.

## 2. Dependencies

Versions are those that compiled in `../colos` (`Cargo.toml` of the
workspace and of `skeleton/`), except `ureq` and `tiny_http`, checked on
crates.io on 2026-11-10.

```toml
[dependencies]
watane-log = { path = "../log" }
watane-pkg = { path = "../pkg" }
ureq = "3.5"            # default features: rustls (ring, webpki-roots) and gzip
url = "2.5"
minicbor = { version = "2.3", default-features = false, features = ["alloc"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tempfile = "3.27"       # the temporary unpack folder (section 5.2)
thiserror = "2.0"

[dev-dependencies]
blake3 = "1.8"          # test oracle only; dist hashes with watane_pkg::PackageId::of
tiny_http = "0.12"
```

Why `ureq`: a blocking client with a global timeout per request
(`RequestBuilder::config().timeout_global`), a body limit
(`Body::with_config().limit(n).read_to_vec()`), rustls, and no async
runtime. `dist` makes sequential reads only, so async gives nothing.
`reqwest` would add tokio and hyper to a library that needs neither.

## 3. Public API

This section is the only place with the exact signatures of `dist`. A
release is named by its `ReleaseRef` (decision D-A1); an `EntryId` appears
only where the own channel names its own entries.

```rust
pub use watane_log::{DecodeError, Entry, EntryId, KeyTextError, PublicKey};
pub use watane_pkg::{Package, PackageError, PackageId, PublisherKey, Release, ReleaseError, ReleaseRef, Slug, Version};

/// One per home. Send + Sync; share it as Arc<Dist>. Every method blocks:
/// call it from a blocking thread, never from inside an async task.
pub struct Dist { /* private */ }

impl Dist {
    /// Opens `Log` at `<home>/log/` (it creates the identity at the first
    /// open, DIST-F-1) and `<home>/dist/`. No network. Fails with Locked when
    /// another Dist holds the home.
    pub fn open(home: &Path, confirm: Arc<dyn Confirm>, options: Options) -> Result<Dist, DistError>;

    pub fn identity(&self) -> PublicKey;
    pub fn fingerprint(&self) -> String;                       // PublicKey::fingerprint
    pub fn backup(&self, passphrase: &str) -> Result<Vec<u8>, DistError>;
    /// Replaces the identity with the one in the backup, while the own
    /// channel has no statement (DIST-F-3). Afterwards the own head is
    /// unconfirmed, and every own statement fails with OwnHeadUnconfirmed.
    pub fn restore(&self, backup: &[u8], passphrase: &str) -> Result<PublicKey, DistError>;
    /// Confirms the own head after a restore. With `own`, reads and verifies
    /// the own channel from that link first (DIST-F-3, DIST-F-4). With None,
    /// the participant stated in the app that this identity never published.
    pub fn confirm_own(&self, own: Option<&Link>) -> Result<(), DistError>;
    pub fn own_confirmed(&self) -> Result<bool, DistError>;

    // Own statements. Each one asks `confirm` exactly once (section 5.2).
    pub fn set_name(&self, name: &str) -> Result<Entry, DistError>;
    pub fn publish(&self, package: Vec<u8>) -> Result<Published, DistError>;
    pub fn withdraw(&self, release: &ReleaseRef, reason: &str) -> Result<Entry, DistError>;

    // Site folder and links.
    pub fn site_dir(&self) -> PathBuf;                         // <home>/dist/site
    pub fn set_site_url(&self, url: &str) -> Result<(), DistError>;
    pub fn own_link(&self, slug: Option<&Slug>) -> Result<Link, DistError>;
    pub fn export(&self, dir: &Path) -> Result<Exported, DistError>;

    // Other channels.
    pub fn follow(&self, link: &Link) -> Result<RefreshReport, DistError>;
    pub fn unfollow(&self, channel: &PublicKey) -> Result<(), DistError>;
    pub fn refresh(&self) -> Vec<RefreshReport>;               // one per channel read: followed and chosen, then listed (5.9)
    pub fn refresh_channel(&self, channel: &PublicKey) -> Result<RefreshReport, DistError>;

    // Views, derived from the stored entries on each call (section 5.6).
    pub fn own(&self) -> Result<ChannelView, DistError>;
    pub fn followed(&self) -> Result<Vec<ChannelView>, DistError>;
    pub fn channel(&self, channel: &PublicKey) -> Result<ChannelView, DistError>;
    /// The release of `slug` with the highest seq that is not withdrawn.
    pub fn latest(&self, channel: &PublicKey, slug: &Slug) -> Result<Option<ReleaseInfo>, DistError>;

    // Packages (section 5.7).
    pub fn fetch(&self, release: &ReleaseRef) -> Result<Fetched, DistError>;
    pub fn open_link(&self, link: &Link) -> Result<Fetched, DistError>;  // the link must have a slug

    // Indexes (step 4, section 5.9).
    /// Marks the channel of `link` (no slug) as a chosen index, reads it and
    /// then every channel of its read set (DIST-F-40). Fails like `follow`
    /// when the first read of the index fails; then nothing changes.
    pub fn choose_index(&self, link: &Link) -> Result<Vec<RefreshReport>, DistError>;
    /// Removes the index mark. The channel stays when it is followed.
    pub fn drop_index(&self, channel: &PublicKey) -> Result<(), DistError>;
    pub fn indexes(&self) -> Result<Vec<IndexView>, DistError>;     // chosen indexes, key order
    // Own index statements. Each asks `confirm` exactly once (section 5.2).
    pub fn list(&self, link: &Link, note: &str) -> Result<Entry, DistError>;      // a link with a slug
    pub fn include(&self, link: &Link, note: &str) -> Result<Entry, DistError>;   // a link with no slug
    pub fn unlist(&self, listing: &EntryId) -> Result<Entry, DistError>;          // an own live list or include

    // Search (step 4, section 5.10). Reads only stored channels: no network.
    pub fn search(&self, query: &Query) -> Result<Vec<Hit>, DistError>;
}

#[derive(Clone, Debug)]
pub struct Options {
    pub connect_timeout: Duration,   // default 10 s
    pub entry_timeout: Duration,     // default 30 s: one whole GET of `head` or `entries/<n>`
    pub package_timeout: Duration,   // default 300 s: one whole GET of `packages/<id>`
    pub max_entry_file: u64,         // default watane_log::MAX_ENTRY (65_728 bytes)
    pub max_package: u64,            // default watane_pkg::MAX_PACKAGE_BYTES (64 MiB)
    pub max_entries_per_read: u64,   // default 10_000 entry files per channel per read
    pub max_listed: usize,           // default 512 listed channels in all (5.9)
    pub max_listed_per_index: usize, // default 128 listed channels per chosen index
    pub max_listed_entries: u64,     // default 1_000 entry files per read of a listed channel
    pub listed_local: bool,          // default false; tests set true to list 127.0.0.1 hosts
}
impl Default for Options { /* the defaults above */ }

pub trait Confirm: Send + Sync {
    /// Blocks until the participant answers in the app. dist holds its
    /// own-channel lock during the call, so an implementation must not call
    /// set_name, publish or withdraw of the same Dist (it would wait forever).
    fn confirm(&self, request: &SignRequest) -> Decision;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision { Approve, Refuse }

#[derive(Clone, Debug, PartialEq)]
pub struct SignRequest {
    pub channel: PublicKey,
    pub fingerprint: String,
    pub statement: Statement,      // decoded from the exact body bytes that dist signs
}
#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    Profile(Profile),
    /// `reference`: own key, slug and version. `release`: pkg's body (PKG-F-20).
    Release { reference: ReleaseRef, release: Release },
    /// `reference` and `release`: the release that it withdraws.
    Withdraw { withdraw: Withdraw, reference: ReleaseRef, release: Release },
    List(Listing),                                     // step 4: the link has a slug
    Include(Listing),                                  // step 4: the link has no slug
    /// `listing`: the own `list` or `include` that it ends; `include` says which kind.
    Unlist { entry: EntryId, listing: Listing, include: bool },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile { pub name: String }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Withdraw { pub release: EntryId, pub reason: String }   // the body (4.1)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing { pub link: Link, pub note: String }             // the body of list and include (4.1)
impl Statement { pub fn kind(&self) -> &'static str; }  // "profile" | "release" | "withdraw" | "list" | "include" | "unlist"

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link { /* channel: PublicKey, base: url::Url, slug: Option<Slug> */ }
impl Link {
    pub fn new(channel: PublicKey, base: &str, slug: Option<&Slug>) -> Result<Link, DistError>;
    pub fn channel(&self) -> PublicKey;
    pub fn base(&self) -> &str;                // normalized, ends with "/"
    pub fn slug(&self) -> Option<&Slug>;
}
impl std::str::FromStr for Link { type Err = DistError; }   // DistError::BadLink
impl std::fmt::Display for Link {}                          // parse(display(l)) == l

#[derive(Clone, Debug)]
pub struct ChannelView {
    pub channel: PublicKey,
    pub fingerprint: String,
    pub name: Option<String>,          // the last valid profile
    pub base: Option<String>,          // followed: the base of its link; own: the site URL
    pub head: Option<u64>,
    pub releases: Vec<ReleaseInfo>,    // seq order; withdrawn ones included
    pub bad: Vec<BadStatement>,        // seq order
    pub status: Status,
    pub followed: bool,
    pub index: bool,                   // a chosen index (step 4)
    pub listed_by: Vec<PublicKey>,     // chosen indexes whose read set holds it (5.9), key order
    pub listings: Vec<ListingInfo>,    // its own list and include statements, seq order; unlisted ones included
    pub last_attempt: Option<u64>,     // ms since the Unix epoch; None for the own channel
    pub last_success: Option<u64>,     // ms since the Unix epoch; None for the own channel
}
#[derive(Clone, Debug)]
pub struct ListingInfo {
    pub entry: EntryId, pub seq: u64, pub include: bool,
    pub listing: Listing, pub unlisted: Option<u64>,   // seq of the unlist
}
#[derive(Clone, Debug)]
pub struct IndexView {
    pub view: ChannelView,             // index is true
    pub included: Vec<PublicKey>,      // the live includes that are read (depth 1)
    pub mods: usize,                   // listed mods with a stored, verified release
    pub skipped: Vec<Skipped>,         // channels of its walk not read, walk order, each once (5.9)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped { pub channel: PublicKey, pub cause: &'static str } // "own channel" | "local address" | "over the listing limit"
#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    pub reference: ReleaseRef,         // channel key, release.slug, release.version
    pub channel: PublicKey, pub entry: EntryId, pub seq: u64,
    pub release: Release, pub withdrawn: Option<Withdrawal>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Withdrawal { pub seq: u64, pub reason: String }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadStatement { pub seq: u64, pub kind: String, pub cause: String }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status { Ok, Compromised { seq: u64 } }   // seq of the first contradicting position

#[derive(Debug)]                             // info.reference names the new release
pub struct Published { pub entry: Entry, pub info: ReleaseInfo, pub link: Option<Link>, pub site: Exported }
#[derive(Debug)]
pub struct Exported { pub dir: PathBuf, pub head: Option<u64>, pub written: Vec<PathBuf> } // written: relative paths

#[derive(Debug)]
pub struct RefreshReport { pub channel: PublicKey, pub outcome: Outcome, pub changes: Vec<Change> }
#[derive(Debug)]
pub enum Outcome {
    Current { head: u64 },                   // all entries up to the served head are stored
    Behind { served: u64, stored: u64 },     // the host serves an older head (DIST-F-27)
    Compromised { seq: u64 },                // now or earlier; no request was made after it
    Failed { cause: DistError },             // the entries read before the failure are kept
}
#[derive(Clone, Debug)]
pub enum Change {                            // only for entries stored by this read, seq order
    Name { seq: u64, name: String },
    Release(ReleaseInfo),
    Withdrawn { seq: u64, release: ReleaseInfo },   // release.withdrawn is Some
    Bad(BadStatement),
    Listed(ListingInfo),                            // step 4: a new list or include
    Unlisted { seq: u64, listing: ListingInfo },    // step 4: listing.unlisted is Some
}

// Search (step 4, section 5.10).
#[derive(Clone, Debug, Default)]
pub struct Query {
    pub text: String,                        // at most 256 bytes; words separated by whitespace
    pub publisher: Option<PublicKey>,
    pub interface: Option<InterfaceQuery>,
    pub origin: Option<(PublicKey, Slug)>,   // step 5: the release names a release of this mod as its origin (5.10)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceQuery { /* ns:pkg/iface, version: Option<Version> */ }
impl std::str::FromStr for InterfaceQuery { type Err = DistError; }   // BadQuery; an InterfaceName with or without "@<version>"
#[derive(Clone, Debug)]
pub struct Hit {
    pub info: ReleaseInfo,                   // the newest release of the mod that is not withdrawn
    pub publisher_name: Option<String>,
    pub link: Option<Link>,                  // the publisher key, the base it is read from, the slug; None for the own channel
    pub status: Status,                      // of the publisher's channel
    pub checked_at: Option<u64>,             // last_success of the publisher's channel; None for the own channel
    pub unchecked_since: Option<u64>,        // Some(last_success) when the last read of that channel failed (DIST-F-42)
    pub via: Vec<Via>,                       // why it is in the search set; Own, Followed, then Index in key order
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Via {
    Own,
    Followed,
    /// A chosen index lists it, or its owner published it. `through`: the
    /// included index whose statement or channel it comes from. `note`: of the list statement, or "".
    Index { index: PublicKey, through: Option<PublicKey>, note: String },
}

#[derive(Debug)]
pub struct Fetched {
    pub package: Package,                    // verified (section 5.7)
    pub entry: Entry,                        // the release statement, as stored
    pub info: ReleaseInfo,
    pub publisher_name: Option<String>,
    pub checked_at: Option<u64>,             // last_success of the channel; None for the own channel
    /// Step 5: the publisher's channel, the base the release was read from, and the slug;
    /// None for the own channel. The caller follows it after an install (5.11).
    pub link: Option<Link>,
}
```

### 3.1 Errors

Direct `PublicKey` fields in error text use `PublicKey::fingerprint()` (10
characters); their fields retain the full key. Invalid key input keeps the
text the participant typed.

One enum. `Display` writes untrusted text (URLs, paths, names, causes from a
host) escaped with `escape_debug()` and unquoted; paths use `path.display()`
before escaping. A variant never repeats the error exposed by `source()`;
`Log`, `Package` and `Release` are transparent. `Io` displays only the escaped
path and exposes its `io::Error` as its source. `SiteNotWritten` displays its
cause and does not expose it as a source.

```rust
#[derive(Debug, thiserror::Error)]
pub enum DistError {
    // Home and files
    Locked { path: PathBuf },
    Io { path: PathBuf, #[source] source: std::io::Error },
    Corrupt { path: PathBuf, cause: String },          // follows.json, site.json
    Log(#[from] watane_log::Error),                    // shown unchanged; it names its cause
    Package(#[from] PackageError),                     // publish: the bytes are no valid package
    Release(#[from] ReleaseError),                     // publish: Release::new refused (not LocalProvider)
    // Own statements
    Refused,
    OwnHeadUnconfirmed,                                // after restore, before confirm_own
    BadName { cause: &'static str },
    BadReason { cause: &'static str },
    VersionExists { release: ReleaseRef, seq: u64 },
    UnreleasedProvider { provider: Slug },             // from ReleaseError::LocalProvider
    PackageTooLarge { size: u64, max: u64 },
    NotOwnRelease { release: ReleaseRef },
    AlreadyWithdrawn { release: ReleaseRef, seq: u64 },
    SiteNotWritten { entry: EntryId, cause: Box<DistError> },   // signed and stored; the next export repairs
    ExportConflict { path: PathBuf, cause: &'static str },
    NoSiteUrl,
    // Links and channels
    BadLink { cause: &'static str },
    BadUrl { url: String, cause: &'static str },
    OwnChannel,
    NotOwnLink { found: PublicKey },   // confirm_own: "this link is not your channel" (DD-16)
    NotFollowed { channel: PublicKey },
    Compromised { channel: PublicKey, seq: u64 },
    BadKey { key: PublisherKey, cause: KeyTextError },  // the bytes are no valid log key
    NoRelease { channel: PublicKey, slug: Slug },
    UnknownRelease { release: ReleaseRef },
    Withdrawn { release: ReleaseRef, reason: String },
    // Reads from a web host
    NotFound { url: String },
    Http { url: String, status: u16 },
    Timeout { url: String },
    TooLarge { url: String, limit: u64 },
    Network { url: String, cause: String },
    Invalid { url: String, cause: watane_log::DecodeError },  // Entry::decode refused the file
    WrongChannel { url: String, expected: PublicKey, found: PublicKey },
    WrongSeq { url: String, expected: u64, found: u64 },
    HeadMismatch { url: String },    // entries/<head seq> differs from head (DD-15)
    TooManyEntries { channel: PublicKey, stored: Option<u64>, served: u64 },
    HashMismatch { url: String, expected: PackageId, found: PackageId },
    BadPackage { url: String, cause: PackageError },  // from_bytes or unpack refused it
    PackageMismatch { url: String, cause: ReleaseError },   // Mismatch { field } (PKG-F-23)
    // Indexes and search (step 4)
    BadNote { cause: &'static str },
    BadQuery { cause: &'static str },
    OwnListing,                              // list or include of the own channel
    AlreadyListed { seq: u64 },              // a live listing of the same mod or index
    NotListed { entry: EntryId },            // unlist: no live own list or include
    NotIndex { channel: PublicKey },         // drop_index of a channel that is no chosen index
}
```

## 4. Data formats

### 4.1 Statement kinds and bodies (W15)

The envelope (channel, seq, prev, time, kind, body, signature) is `log`'s.
`dist` chooses the kind and the body.

| Kind (exact ASCII) | Body |
|---|---|
| `profile` | CBOR array of 1: `[name: text]` |
| `release` (`watane_pkg::RELEASE_KIND`) | the bytes of `Release::encode()`; read with `Release::decode` |
| `withdraw` | CBOR array of 2: `[release: bytes(32), reason: text]`. `release` is the entry id of a `release` statement of the same channel at a lower seq. |
| `list` (step 4) | CBOR array of 2: `[link: text, note: text]`. `link` is a link of 4.2 **with** a slug: the mod `<key>/<slug>` and where its channel is read. |
| `include` (step 4) | CBOR array of 2: `[link: text, note: text]`. `link` is a link of 4.2 **with no** slug: another index. |
| `unlist` (step 4) | CBOR array of 1: `[listing: bytes(32)]`, the entry id of a `list` or `include` statement of the same channel at a lower seq. |

CBOR rules for `profile`, `withdraw`, `list`, `include` and `unlist`
(RFC 8949 section 4.2.1): definite lengths, shortest heads, UTF-8 text.
Encode with `minicbor::Encoder`. A decoder refuses another major type, another array length, trailing bytes,
and any input that does not encode back to the same bytes. A change of a
body later is a new kind, not a new field.

Byte vectors (the test oracle, computed by hand):

| Value | Bytes (hex) |
|---|---|
| `Profile { name: "Alice" }` | `81 65 41 6c 69 63 65` |
| `Withdraw { release: [0x11; 32], reason: "bug" }` | `82 58 20` then 32 × `11` then `63 62 75 67` |
| `list`: link `watane:47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy@https://alice.example.org/mods/#reading-list` (104 bytes), note `good for books` | `82 78 68`, the 104 link bytes, `6e`, the 14 note bytes |
| `include`: the same link with no `#reading-list` (91 bytes), note `""` | `82 78 5b`, the 91 link bytes, `60` |
| `unlist`: `[0x22; 32]` | `81 58 20` then 32 × `22` |

The link text in a body is the `Display` form of `Link` (4.2): a decoder
refuses a link that does not format back to the same text.

Text rules (`body.rs`, one function for every field):

| Field | Length (Unicode scalar values) | Refused characters |
|---|---|---|
| name | 1 to 64 | `char::is_control`; U+061C, U+200E, U+200F, U+202A–U+202E, U+2066–U+2069; whitespace at the start or the end |
| reason | 1 to 280 | the same |
| note (step 4) | 0 to 280 | the same |

`dist` checks these rules before a confirmation (DIST-F-9) and again when it
reads a body of another channel.

**Statement rules** (`view::derive`, one function for the own channel and
for every other channel). Walk the entries in seq order:

| # | Condition | Effect |
|---|---|---|
| 1 | kind is not one of the six of this table | ignored, not reported (a later kind) |
| 2 | body does not decode, or a text rule fails | `BadStatement { cause }` |
| 3 | `profile` | `name` becomes this name |
| 4 | `release` with a slug and version that an earlier release of this channel has | `BadStatement { cause: "version <v> of <slug> already released at seq <n>" }`. So one `ReleaseRef` names at most one release. |
| 5 | `release` | a new `ReleaseInfo` |
| 6 | `withdraw` whose `release` is no earlier `release` entry of this channel | `BadStatement { cause: "withdraw names no earlier release" }` |
| 7 | `withdraw` of a release that is already withdrawn | `BadStatement { cause: "already withdrawn at seq <n>" }` |
| 8 | `withdraw` | sets `withdrawn` of that release |
| 9 | `list` whose link has no slug, or `include` whose link has a slug | `BadStatement { cause: "list needs a mod link" / "include needs a channel link" }` |
| 10 | `list` or `include` whose link key is this channel's key | `BadStatement { cause: "lists its own channel" }`: its own releases count already (5.9) |
| 11 | `list` of a key and slug, or `include` of a key, with a live listing in this channel | `BadStatement { cause: "already listed at seq <n>" }` |
| 12 | `list` or `include` | a new `ListingInfo` |
| 13 | `unlist` whose `listing` is no earlier `list` or `include` entry of this channel, or one already unlisted | `BadStatement { cause: "unlist names no live listing" }` |
| 14 | `unlist` | sets `unlisted` of that listing |

A listing is **live** while it has no `unlisted`. A `list` says only "this
mod is worth finding" and where to read its publisher's channel. It names
no version and no package id: the versions, the withdrawals and the bytes
come only from the publisher's own channel (DD-18).

### 4.2 Link

```abnf
link  = "watane:" key "@" base [ "#" slug ]
key   = 52zbase32          ; the text form of log::PublicKey
base  = absolute-URL       ; scheme http or https, with a host; no userinfo, no query, no fragment
slug  = <the Slug text of pkg/TECHNICAL.md 3>   ; read with Slug::from_str
```

Parse: the text is at most 2048 bytes and starts with `watane:` (case
sensitive). The key is the text up to the first `@`, read with
`PublicKey::from_str`. The rest splits at the first `#` into base and slug.
`url::Url::parse` reads the base. If its path does not end with `/`, dist
appends `/`. Format: `watane:{key}@{base}`, then `#{slug}` when present.

Example: `watane:47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy@https://alice.example.org/mods/#reading-list`

Without a slug, a link names a channel. With a slug, it names the newest
release of that mod that is not withdrawn (`Dist::latest`). The key in the
link is the identity: the base is only where to read.

### 4.3 Site folder

```text
<site>/
  head             the newest entry of the own channel: the same bytes as entries/<head seq>
  entries/<seq>    each entry; <seq> in decimal ASCII, no leading zero: 0, 1, 2, ...
  packages/<id>    the package of each release that is not withdrawn; <id> = the PackageId text (64 lowercase hex)
```

- An entry file holds exactly `Entry::bytes()`. A package file holds
  exactly the package bytes. No other file. No file extension. A reader
  ignores `Content-Type`.
- The URL of a file is `Url::join(base, "<relative path>")`.
- An empty own channel gives no file.
- Write each file as `<parent>/.<name>.tmp`, fsync, rename. Order:
  packages, entries in seq order, `head` last.
- An existing file with equal bytes is not written again. An existing
  `packages/<id>` whose `PackageId::of` is not `<id>` is replaced.
- Refuse with `ExportConflict` (DIST-F-20): `entries/<n>` exists with other
  bytes; `head` exists and is not an entry, is an entry of another key, or
  has a seq above the own head.
- Delete only withdrawn package files from the built-in `site/` (DD-12), never delete files from external exports, and never write outside `head`, `entries/`, `packages/`.

### 4.4 Storage in the home

`dist` owns `<home>/dist/`, and it is the only opener of `<home>/log/`,
which `log` owns: the identity key and the channel store of the own and
every read channel (`log/TECHNICAL.md` 4.7). `host` reserves both (section
10). Directories are mode 0700; files that `dist` writes are mode 0600,
except in `site/` (0644, 0755).

```text
<home>/dist/
  lock             empty; Dist::open holds File::try_lock on it until drop
  packages/<id>    verified package bytes, own and fetched; <id> as in 4.3
  tmp/             temporary unpack folders (5.2); open empties it
  site/            the site folder of the own channel (4.3)
  follows.json     the followed channels
  site.json        the site URL
```

`follows.json` (serde, `BTreeMap` for a stable order):

```json
{
  "version": 1,
  "channels": {
    "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy": {
      "base": "https://alice.example.org/mods/",
      "followed_at": 1762780000000,
      "last_attempt": 1762783600000,
      "last_success": 1762783600000
    }
  }
}
```

Step 4 adds two fields to a channel record, with serde defaults, so the
version stays 1 and a step 3 file reads unchanged:
`"followed": bool` (default `true`: every step 3 record is a follow) and
`"index": bool` (default `false`). A record with both `false` is a
**listed** channel: dist reads it only because the read set of a chosen
index holds it (5.9), and removes the record when no read set holds it any
more. A listed record also has `"bases": [text]`: all distinct bases that
listings give (5.9), at most 4, default `[]`; its `base` equals the first
element. The base that serves the highest head moves to the front.

`site.json`: `{"version":1,"url":"https://alice.example.org/mods/"}`.
Times are milliseconds since the Unix epoch, from `SystemTime::now()`. A
missing file is empty. Another `version` or bad JSON is `Corrupt`. Both
files are written as temp file, fsync, rename.

### 4.5 Reads from a web host

| Read | URL | Body limit | Timeout of the whole GET | Extra header |
|---|---|---|---|---|
| head | `<base>head` | `max_entry_file` | `entry_timeout` | `Cache-Control: no-cache` |
| entry | `<base>entries/<n>` | `max_entry_file` | `entry_timeout` | none |
| package | `<base>packages/<id>` | `release.size`, at most `max_package` | `package_timeout` | none |

One `ureq::Agent` per `Dist`:

```rust
let agent: ureq::Agent = ureq::Agent::config_builder()
    .http_status_as_error(false)
    .max_redirects(5)
    .timeout_connect(Some(options.connect_timeout))
    .user_agent(concat!("watane-dist/", env!("CARGO_PKG_VERSION")))
    .build()
    .into();
// per read:
let mut response = agent.get(url.as_str())
    .config().timeout_global(Some(timeout)).build()
    .call()?;                                       // + .header(...) for head
let bytes = response.body_mut().with_config().limit(limit).read_to_vec()?;
```

| Result | DistError |
|---|---|
| status 200 | the bytes |
| status 404 or 410 | `NotFound` |
| other status | `Http` |
| `ureq::Error::Timeout(_)` | `Timeout` |
| `ureq::Error::BodyExceedsLimit(_)` | `TooLarge` |
| any other `ureq::Error` | `Network { cause: error.to_string() }` |

Redirects to another host are followed: every byte is verified, so the
host is never trusted. The proxy comes from the environment (`ureq`
default). Both `http` and `https` are allowed (decision DD-6).

## 5. Behaviour

### 5.1 Open

Create `<home>/dist/` (0700) and its `packages/` and `site/`. Take the
lock, or fail with `Locked`. Remove `tmp/` and create it again (a crash
can leave a folder there). `Log::open(<home>/log)`. Read `follows.json`
and `site.json`. Make no request.

### 5.2 Sign and append (the only path to `Log::append`)

`own.rs::sign_and_append(guard, kind, body)`, where `guard` is the
`MutexGuard` of the own-channel mutex. The caller took it before its own
checks, so nothing changes the own channel between check and append.

1. `log.status(own)` is compromised: `Compromised`.
2. `log.own_head_confirmed()` is false: `OwnHeadUnconfirmed`.
3. Decode `body` again into a `Statement` and build the `SignRequest`
   (the `ReleaseRef` of a release: own key, slug and version of its body).
4. `confirm.confirm(&request)`. `Refuse` gives `Refused`; nothing changed.
5. `log.append(kind, &body)`.
6. `export(site_dir)`. An error gives `SiteNotWritten { entry, cause }`.

`set_name(name)`: text rules; lock; `sign_and_append("profile", body)`.

`withdraw(reference, reason)`: text rules; lock; the publisher is the own
key and the own view has a release with that slug and version
(`NotOwnRelease`), not withdrawn (`AlreadyWithdrawn`); the body names its
entry id; `sign_and_append("withdraw", body)`.

`unpacked(&package) -> Result<(TempDir, Mod), PackageError>` (private):
`pkg` reads a mod only from a folder, so `Release::new` and
`Release::check` need the package on disk. `TempDir::new_in(<home>/dist/tmp)`,
then `package.unpack(&tmp.path().join("mod"))`, which also runs `Mod::read`
and `check_component`. The folder is removed when the `TempDir` drops.

`publish(bytes)`:

1. `bytes.len() > max_package`: `PackageTooLarge`.
2. `Package::from_bytes(bytes)`, then `unpacked` (`Package`).
3. `Release::new(&m, &package)`. `ReleaseError::LocalProvider { slug }`
   (PKG-F-21) gives `UnreleasedProvider { provider: slug }`; any other
   error gives `Release`.
4. Lock. The own view has a release with this slug and version:
   `VersionExists`.
5. Body = `release.encode()`.
6. `sign_and_append("release", body)`, with one addition: between its
   steps 4 and 5, write `packages/<id>`. If the append fails, remove that
   file when this call created it.
7. Return `Published`, with `link = own_link(Some(slug))` when the site
   URL is set.

### 5.3 Export

Read all own entries (`log.entries(own, 0, usize::MAX)`), derive the view, write the
files of section 4.3 for the entries and the packages of releases that are
not withdrawn. Package bytes come from `<home>/dist/packages/<id>`;
`PackageId::of` is checked before the write.

### 5.4 Read a channel (`follow.rs::read_channel(key, base)`)

Used by follow, refresh and `confirm_own`.

1. GET `head`. `Entry::decode` (`Invalid` on error). `entry.author() != key`:
   `WrongChannel`.
2. `stored = log.head(&key)` (its `seq`, or none).
3. If `head.seq() > stored`: GET `entries/<n>` for `n` from `stored + 1`
   (or 0) up to `head.seq() - 1`, at most `max_entries_per_read` files. Each
   file: decode (`Invalid`), author (`WrongChannel`), `seq() == n`
   (`WrongSeq`). Stop at the first failure and keep the files before it.
   Add the head entry only when every file before it was read. Then GET
   `entries/<head.seq()>`: its bytes must equal `head`'s, else `HeadMismatch`
   naming its URL (DD-15); a failure here is a step-3 failure.
4. `log.ingest(&key, &entries)` in one call, also when `head.seq() <= stored`
   (then the input is only the head entry).
5. Outcome, in this order: `ingest.status` is compromised: `Compromised`
   (its seq is `fork.chain.seq()`). `ingest.gap` is set (only possible
   when another call stored entries meanwhile): `Failed { WrongSeq }` for
   the gap seq. A failure in step 3:
   `Failed { cause }`. The `max_entries_per_read` cap was hit:
   `Failed { TooManyEntries }`. `head.seq < stored` and no fork: `Behind`.
   Else `Current`.
6. Changes: derive the view; report each item whose seq is above the old
   stored head (section 3, `Change`).
7. Update `follows.json` when the key has a record: `last_attempt = now`;
   `last_success = now` for `Current` only. (Step 4 changes this from
   "`Current` and `Behind`": a host that serves an older state cannot show
   newer withdrawals, so the freshness mark must not call it checked.)

`follow(link)`: own key: `OwnChannel`. Already compromised: `Compromised`.
`read_channel`. `Failed` or `Compromised` at the first follow: return the
cause as `Err` and do not add the channel (DIST-F-23). So `follow` never
returns `Ok` with `Failed` or `Compromised`; only `refresh` and
`refresh_channel` report them inside `Ok` (DD-17). Else add or update the
entry in `follows.json` (the base of the newest link wins; the key does
not change).

`refresh()`: for each followed key in key order: compromised gives
`Outcome::Compromised` with no request (DIST-F-26); else `read_channel`.
From step 4, this is pass (a) of 5.9.

### 5.5 Restore

`restore(backup, passphrase)`: lock; `log.restore_identity(backup,
passphrase)` (it refuses with `IdentityInUse` when the own channel has a
statement); remove `site.json` (the site URL belonged to the replaced
identity); return the new key. The own head is now unconfirmed.

`confirm_own(own)`: lock.
- With a link: `own.channel() != identity()`: `NotOwnLink` (DD-16).
  `read_channel(identity, base)` must give `Current`; else its cause, and
  the head stays unconfirmed. Then `log.confirm_own_head()`, write
  `site.json` with the base, and export the site folder.
- With None: `log.confirm_own_head()`. The app calls this only after the
  participant states that this identity never published. If that is
  false, the next statement forks the own channel (A19, question Q3).

### 5.6 Views

`channel(key)`: `log.entries(&key, 0, usize::MAX)`, then `view::derive`
(4.1), `log.status`, and the `follows.json` record. No cache: channels are
small, and one derivation keeps one rule.

### 5.7 Fetch (verification order)

`fetch(reference)`:

1. `channel = PublicKey::from_bytes(reference.publisher.as_bytes())`
   (`BadKey`). `log.status(&channel)` compromised: `Compromised`.
   Step 5: a channel that is not the own one and has no record is read
   first from its provider bases (5.11), so that step 2 sees its entries.
2. Derive the view. The release statement with this slug and version (rule
   4 of 4.1: at most one): none gives `UnknownRelease`; withdrawn gives
   `Withdrawn { reason }`.
3. Cache: if `packages/<id>` exists and its `PackageId::of` equals
   `release.package`, use it (no request). If it differs, delete the file.
4. Else the channel must have a record in `follows.json` (followed, a
   chosen index, or listed), or a provider base (5.11; step 5); else
   `NotFollowed`. GET the package from the record's `base`, or from the
   provider base that served the highest head, with its limit
   (`release.size`, at most `max_package`) and timeout.
5. `PackageId::of(&bytes) != release.package`: `HashMismatch`.
6. `Package::from_bytes`, then `unpacked` (`BadPackage`).
7. `release.check(&m, &package)`: pkg confirms that the release body and
   the package agree, or names the first field that differs (PKG-F-23):
   `PackageMismatch { cause }`.
8. Write the cache file. Return `Fetched` with the stored release entry.
   `Fetched.link` is `Link::new(channel, base, Some(slug))` with the base
   of step 4 (the record's `base` on a cache hit; for a cache hit of a
   channel with no record, the first provider base, or `None` when there
   is none); `None` for the own channel.

Nothing reaches the caller before step 7. `host` checks the signature of
the entry and the package id again (its own SPEC); that is a second check,
not a replacement.

`open_link(link)`: no slug: `BadLink`. Own key: no read (cache only).
Not followed: `follow(link)`. Followed: `refresh_channel` (a `Failed`
outcome is not an error here; the stored channel is used and `checked_at`
shows its age). Then `latest(slug)` (`NoRelease`) and
`fetch(&info.reference)`.

### 5.8 Updates and withdrawals as the follower sees them

`host` records an installed mod with its `ReleaseRef`. With that:

- **Withdrawn**: the release of `channel(publisher)` with that reference
  has `withdrawn` set. A refresh reports it once as `Change::Withdrawn`.
- **Update available**: `latest(publisher, slug)` has a seq above the seq
  of the installed release. A refresh reports it once as `Change::Release`.
- **Compromised**: the view status. `fetch` of any release fails.
- **Age**: `last_success` of the channel.

The decision to update without a question (F-INST-7) and the offer after a
withdrawal (F-INST-8) are step 4, in `host` and `app`. Step 3 provides the
data only.

### 5.9 Indexes and the read set (step 4)

An index is a channel, not a second kind of channel (W20). Every channel
can hold `list`, `include` and `unlist` statements; a channel becomes an
index for this home only when the participant chooses it. The own channel
always counts as a chosen index, but has no record: `ChannelView.index` is
`false` for it and `drop_index(own)` is `NotIndex`.

**Records.** `follows.json` (4.4) holds one record per channel that is
followed, chosen, or listed. `followed()` returns only records with
`followed: true`. `channel`, `refresh_channel` and `fetch` work for any
record. `ChannelView.base` is the record's `base`.

**Read set** of a chosen index `I` (one level of includes, W21), walked in
this order:

1. `I` itself: all its releases.
2. Each live `list` of `I`, in seq order: the channel of its link, for that
   slug.
3. Each live `include` of `I`, in seq order: the included channel `J`, all
   its releases.
4. For each such `J`, in the order of step 3, each live `list` of `J`, in
   seq order: that channel, for that slug. The `include` statements of `J`
   are ignored: dist never reads a channel because of them.

The own key is never in a read set: a listing of it is skipped. A chosen
index that is compromised contributes its own releases up to the fork and
nothing else: its stored listings are ignored, and no channel is read
because of it. An included channel that is compromised gives no listing
either.

**Listed records.** A channel in some read set that is neither followed
nor chosen has a listed record (`followed: false`, `index: false`). Its
`bases` are the distinct bases of the listings that name it, at most 4:
first learned in the order of the walk (own channel first, then chosen
indexes in key order), then reordered by the read below. A followed or chosen channel is read only from its own `base`: a
listing never changes it. dist creates a listed record when the walk first
reaches the channel, before it reads it, and removes it after a refresh in
which no read set holds it.

**Read of a listed channel.** `read_channel` (5.4) from every base in
`bases`, in order, on each read (at most 4 reads per listed channel per
refresh). Each base's entries are ingested as usual; later bases start from
the head stored so far. `Current` means current against the stored head,
so a mirror serving exactly that head must not stop the remaining reads:
another base may hold a newer withdrawal. The outcome is the best one
(`Current` > `Behind` > `Failed`), with the highest served head breaking
ties. A fork still stops further reads and reports `Compromised` (5.4).
`last_success` is set to `last_attempt` when any base gives `Current`;
otherwise the previous success is retained. The base that served the
highest head moves to the front of `bases` (first read wins a tie). Newly
learned bases are appended after retained bases; every base is checked.
The limits of 4.5 and the channel caps below still apply, with at most
`max_listed_entries` (default 1000) entry files per base read.

**Local addresses.** Unless `Options::listed_local` is set (tests only;
default `false`), dist skips a listed base whose host is `localhost`, ends
in `.localhost`, or is an IP literal that is loopback, unspecified,
private (10/8, 172.16/12, 192.168/16, fc00::/7) or link-local. A third
party's listing must not make dist send requests into the participant's
own network. A followed or chosen channel may use any base (DD-6): the
participant gave that link. A host name that resolves to a local address
is not checked (section 6, "Not protected").

**Cap.** At most `max_listed` (default 512) listed channels in all, and at
most `max_listed_per_index` (default 128) for each chosen index, counted
in the walk order (an included `J` counts too). A channel past either cap
gets no record and is not read; `IndexView.skipped` names each channel
that the walk of that index skipped, once, with its cause: `own channel`,
`local address` or `over the listing limit` (S29: a count told the
participant nothing). So no
one index can use up the whole cap.

**`refresh()`** returns one report per channel read, in pass order, key
order within a pass. Pass (a): the followed channels and the chosen
indexes (5.4, compromised ones give `Outcome::Compromised` with no
request). Pass (b): the channels of steps 2 and 3 of every walk. Pass (c):
the channels of step 4. A channel is read at most once per refresh. The
channel budgets also span passes: if pass (b) discovers lists that would exceed the remaining budget,
those channels wait for the next refresh; no unread record is created. Then
remove the listed records that no read set holds.

**`choose_index(link)`**: a link with a slug is `BadLink`; own key is
`OwnChannel`. `read_channel` as `follow` does (a failed or compromised
first read is the `Err`, and nothing changes). Then set `index: true` and
the link's base (as `follow` does: the base of the newest link wins), keep
`followed` as it was (a new record has `followed: false`), and run passes
(b) and (c) for the whole read set. Returns the reports. A second choose
of a chosen index reads it again.

**`unfollow(channel)`** from step 4: no record with `followed: true` is
`NotFollowed`. Set `followed: false`; remove the record when it is no
chosen index and no read set holds it.

**`drop_index(channel)`**: not a chosen index: `NotIndex`. Set `index:
false`; remove the record when it is not followed and no other read set
holds it; remove the listed records that only it held.

**`list(link, note)`**, **`include(link, note)`**: text rules for the note
(`BadNote`); link shape (`BadLink`); own key (`OwnListing`); a live
listing of the same mod or index in the own channel (`AlreadyListed`).
For `list`, the channel must be stored with a release of that slug that is
not withdrawn: if it is not stored, read it once from the link's base as
`read_channel` does, with no record; then `NoRelease` when there is none.
For `include`, the channel must be readable the same way. All of this
happens before `confirm`. Then lock, `sign_and_append("list" | "include",
body)`.

**`unlist(entry)`**: the own view has a live listing with that entry id
(`NotListed`); lock; `sign_and_append("unlist", body)`.

**Views.** `listed_by`, `IndexView.included`, `mods` and `skipped` come
from a walk of the chosen indexes on each call, as 5.6 derives a view on
each call. `mods` counts the listed mods that would give a hit (5.10 step
2).

### 5.10 Search (step 4)

`search(query)` reads only stored entries and `follows.json`: no request,
so it works offline (F-QUAL-1). It derives each view once.

1. **Candidates.** One per mod (`publisher key`, `slug`):
   - each slug of the own channel (`Via::Own`);
   - each slug of each followed channel (`Via::Followed`);
   - each slug of each chosen index, and of each channel that it includes
     (`Via::Index`, `through` the included channel);
   - each mod of a live `list` in the read set of each chosen index
     (`Via::Index`, with the note, `through` for a list of an included
     channel).
2. **Release.** The candidate's newest release that is not withdrawn
   (DD-7, `latest`) from the stored channel of its publisher. None (no
   record for a publisher that is not the own key, never read, no release
   of that slug, or every version withdrawn): no hit
   (F-FIND-4: a withdrawn version is hidden, and an index alone never
   makes a hit).
3. **Filters**, all that are set must hold:
   - `publisher`: the publisher key equals it.
   - `origin` (step 5): `release.origin` is set, its publisher equals the
     key and its slug equals the slug (any version).
   - `interface`: one of `offers`, `events`, or the interface of a call or
     subscribe grant of the release equals it; with no version in the
     query, the interface name without its `@<version>` equals it.
   - `text`: lower-case the query and the haystack with
     `str::to_lowercase`; every whitespace-separated word of the query is a
     substring of the haystack. The haystack is the slug, the release
     name and summary, the publisher's profile name, the notes of every
     `Via::Index` of the candidate, and the interface names of `offers`,
     `events` and grants, joined with `\n`. An empty text matches all.
4. **Order.** More distinct `Via::Index` indexes first, then `Via::Own` and
   `Via::Followed` before the others, then the release name
   (lower-cased), then the `ReleaseRef` text. This order uses only the
   participant's own choices (V6); it is no global ranking.
5. At most 200 hits (the first 200 of the order).
6. **Marks** (DIST-F-42): `checked_at` is the publisher record's
   `last_success`; `unchecked_since` is `Some(last_success)` when
   `last_attempt > last_success`, i.e. the newest read did not give
   `Current` (5.4 step 7). A hit of a
   compromised channel stays in the result with `status: Compromised`, so
   the participant sees why it cannot be installed (F-QUAL-3); `fetch`
   refuses it.

**Install a hit** (the one definition; app 9.6 cites it). `hit.link` names
the publisher's channel, the front base of its record, and the mod; it is
`None` for the own channel. The caller follows `hit.link` when it is set
(`follow`, so that updates and withdrawals keep coming after the index
drops the mod) and then fetches `hit.info.reference` (5.7). A search never
installs and never follows.

### 5.11 Provider bases (step 5)

A consumer pins each provider's key in `deps.json` (W14), but no file says
where that key's channel is read. A home finds it in the statements it
already holds (W25, DD-28):

1. **Hint channels**: the own channel, then each channel with a record in
   `follows.json` (followed, chosen, listed), in key order, skipping
   compromised ones.
2. **Bases**: in that order, the base of each live `list` or `include`
   (4.1) of a hint channel whose link key is the wanted key, in seq order;
   distinct, at most 4. A base on a local address (5.9) is skipped unless
   `listed_local` is set, also when the own channel gives it (one rule).
3. **Read**: `read_channel` from each base, in order, exactly as a listed
   channel is read (5.9: every base, the best outcome, at most
   `max_listed_entries` entry files per base, the limits of 4.5). dist
   creates **no record**: the entries stay in `<home>/log/`, as for the
   check before a `list` (5.9), and a search gives no hit for them. A fork
   gives `Compromised`.
4. No base: `NotFollowed` with the key; its message says that no followed
   channel, chosen index or listing names a base for it. Every base
   failed: the cause of the first base's failure (for example
   `WrongChannel`). Either way the app then offers a pick (app 9.7).

**Publish side.** So that a consumer's own channel names the base of its
providers, the app asks the publisher to `list` each provider of another
publisher after the release (app 8.1, W25). That is an ordinary `list`
(5.9), with its own confirmation. A home that installs the consumer from
its link follows the consumer's channel first (5.7 `open_link`), so its
listings are hints for the providers.

**After an install** the caller follows `Fetched.link` of each installed
release whose channel is not followed (DD-29; app 9.4), so that updates and
withdrawals of a provider keep coming, as for a hit (5.10).

## 6. Security properties

| Property | Enforcement | Test |
|---|---|---|
| No signature without a confirmation (F-AGENT-4, W13) | `sign_and_append` is the only caller of `append`; it calls `confirm` first | A2, `own_refuse_*` |
| What is shown is what is signed | the request is decoded from the body bytes that are then appended; the own lock is held from confirm to append | A3 |
| A web host cannot forge | `log` decodes and verifies each entry; the key comes from the link; the seq must match the file name; packages by BLAKE3 against the signed release | A11, A12, A13 |
| A web host cannot swap the channel | the link pins the key; `follows.json` is keyed by the key; a new base never changes the key | A13 |
| Forks stop a channel (F-PUB-7) | `log` marks it; `refresh` makes no request and `fetch` refuses after it | A15 |
| Rollback is visible | `Behind`; `log` keeps the stored entries | A16 |
| Bounded reads | body limits, whole-request timeouts, at most `max_entries_per_read` files per read, at most 5 redirects | A17, `http_*` |
| No path from remote data to a local path | local paths are built from a seq (u64) and a hash that dist computed or read from a verified release; never from a URL or a name; `unpack` writes only below a new temp folder (pkg 6.4) | `site_*` review |
| The site folder holds only public data | export writes only own entries and own packages, only under `head`, `entries/`, `packages/` | A6 |
| Private data stays private | `<home>/dist/` is 0700; the secret key never leaves `log` except in an encrypted backup | `open_modes` |
| One writer per home | `File::try_lock` on `<home>/dist/lock` | `open_twice_is_locked` |
| Untrusted text cannot steer a terminal | errors print URLs, paths, names and host causes escaped and unquoted | `error_text_escapes_controls` |
| An index cannot vouch for bytes or versions (F-FIND-4) | a `list` names a mod, never a version or a package id; a hit's release comes only from the publisher's verified channel; a channel read through a listing is checked against the key in the listing | A23 |
| An index cannot hide a withdrawal | the release of a hit is the newest that the publisher's channel does not withdraw | A24 |
| An index cannot make dist read without bound | one level of includes; at most `max_listed` listed channels, `max_listed_per_index` per index, `max_listed_entries` per read; each read has the limits of 4.5 | A26, A31 |
| A stale mirror cannot hide a withdrawal | a listed channel is read from each listed base until one is `Current`; only `Current` counts as checked | A34 |
| A listing cannot reach the local network | listed bases on `localhost` or local IP literals are skipped | A35 |
| The own channel is never read from a listing | the own key is skipped in every read set | `index_own_skipped` |
| A stale result is marked | `unchecked_since` when the newest read of the publisher failed | A25 |
| A listing cannot redirect a follow | the base of a followed or chosen channel wins over any listing | `index_base_*` |
| A provider hint cannot swap a provider (step 5) | the key comes from the consumer's signed `deps.json` grants; every entry read from a hinted base is verified against that key; a hint only says where to read | A36 |
| A provider hint cannot reach the local network or read without bound | the local-address rule and the limits of a listed read (5.9) hold for hint bases; at most 4 bases per fetch | A37 |
| An origin is only a claim (W24) | `Release.origin` is shown as the publisher's claim; dist never treats it as a statement of the origin's publisher | review |

Not protected (see `spec/TECHNICAL.md` section 6): a listed host name that
resolves to a local address (step 4; only IP literals and `localhost` are
skipped); a host that withholds new entries; another program of the same user; the use of one identity on
two devices, which forks the channel (question Q7).

## 7. Test plan

Rules: a test computes its expectation without `watane_dist`. Oracles are
literal values, `std::fs`, the `blake3` crate, and `watane_log` and
`watane_pkg` (dependencies, not under test). Network tests use a local
`tiny_http` server on `127.0.0.1:0`, and no other host.

`tests/support/mod.rs`:
- `TestHost::serve(dir)`: serves the files of `dir`; counts requests per
  path; modes `Stall(Duration)` (accept, then send nothing) and
  `Oversize(u64)` (send that many zero bytes).
- `RecordingConfirm`: a queue of `Decision`s; records each `SignRequest`.
- `fixture(version: &str, source: bool) -> Vec<u8>`: copies
  `tests/fixtures/reading-list/` to a temp folder, sets the version in the
  package line `mods:reading-list@<version>` of `wit/world.wit` (D-A2),
  and returns `watane_pkg::pack(dir, source)?.into_bytes()`.
- `fast_options()`: `entry_timeout` 300 ms, `package_timeout` 1 s,
  `max_package` 4096.

| Test | Checks | Oracle |
|---|---|---|
| `link_table` (unit) | 12 valid and 14 invalid link strings: parts, normalized base, round trip | literal table |
| `body_vectors` (unit) | encode and decode of the two vectors of 4.1; refusal of each changed form (array length, indefinite length, trailing byte, non-shortest head) | literal bytes |
| `text_rules` (unit) | each refused character class, lengths 0, 1, 64, 65, 280, 281 | literal table |
| `derive_rules` (unit) | rules 1 to 8 of 4.1 on hand-built entries signed with a `log` test key | literal expected views |
| `open_*` | A1; a second open is `Locked`; dir modes 0700 | `std::fs::metadata` |
| `own_refuse_*` | A2 for publish, withdraw, set_name: own head and site files unchanged | `head` of a second `Log::open(<home>/log)`; directory listing |
| `own_request_*` | A3; A20; A4 and A5 refuse before `confirm` is called | recorded requests; `Release::decode` of the stored body |
| `site_*` | A6, A7, A8; files equal the stored entries and the package bytes | `Entry::bytes` of `Log::entries`; `blake3::hash`; literal file set |
| `follow_*` | A10, A12, A13; a failed first read adds nothing (DIST-F-23); own link refused (DIST-F-30) | bytes Alice published; request counts; `follows.json` read with `serde_json` |
| `refresh_*` | A14, A15 (no request after the fork), A16 | the test's own list of what Alice published |
| `fetch_*` | A11, A17, A18; no cache file after a failed fetch; `fetch` of a `ReleaseRef` parsed from literal text | `blake3::hash`; `std::fs` |
| `restore_*` | A19; a link of another key leaves the head unconfirmed; a statement before `confirm_own` is `OwnHeadUnconfirmed` and asks nothing | `Log::entries`; recorded requests |
| `j3_dist` | the dist half of J3 end to end: open, set name, publish, set site URL, serve, link, `open_link`, tamper, refuse | values the test created |
| `error_text_escapes_controls` | a URL with `\u{1b}` prints escaped | literal string |
| `body_vectors` (step 4 rows) | the three vectors of 4.1 for `list`, `include`, `unlist`; a link that does not format back is refused | literal bytes |
| `derive_rules` (step 4 rows) | rules 9 to 14 of 4.1 | literal expected views |
| `index_*` | A21, A23, A26, A27, A28, A30, A31, A34, A35; `index_base_*`: a followed base wins over a listing's base; `index_own_skipped`: a listing of the own key makes no request | request counts per path; `follows.json` with `serde_json` |
| `search_*` | A22, A24, A25, A29, A32 | the test's own list of what each home published |
| `j4_dist` | the dist half of J4 (A33) | values the test created |

Step 4 tests use one `TestHost` per channel (Alice, Ivy, Jay, ...), each
serving that home's site folder. The index owner's statements are made by
its own `Dist` with an approving `RecordingConfirm`.

The fork in A15 is made without `dist` code under test: the test opens a
`watane_log::Log` in a temp folder, restores Alice's backup into it,
ingests Alice's `entries/0`, calls `confirm_own_head`, appends a different
entry (seq 1), and writes its `bytes()` as `head` and `entries/1` of the
served folder.
`TestHost` serves `Dist::site_dir()` of Alice, or a copy that the test
changes.

Commands: `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`.

## 8. Build plan

Until a sibling provides an item, use a stub crate in `stubs/<name>/` of
this folder with the same package name (`watane-log`) and exactly the
items of section 10, and point the path dependency at it. Delete `stubs/`
and restore `../log` as soon as the sibling has the items. Never put a
stub in another subsystem's folder.

| Stage | Build | Needs | Stub until then | Done when |
|---|---|---|---|---|
| S1 | `error`, `link`, `body` (text rules, CBOR), `Options` | `log`: `PublicKey`, `EntryId` text forms (log stage L1); `pkg`: `Slug` (stage 1) | `PublicKey` as 32 bytes with z-base-32 text (`data-encoding`, alphabet `ybndrfg8ejkmcpqxot1uwisza345h769`) | `link_table`, `body_vectors`, `text_rules` pass |
| S2 | `open`, `sign_and_append`, `set_name`, `withdraw`, `view::derive`, `own`, `channel` | `log`: `Log`, `Entry` (L2, L3) | a `log` stub: `Log` with an in-memory chain and real ed25519 (`ed25519-dalek = "3.0"`) | `open_*`, `own_refuse_*` (set_name), `derive_rules` pass |
| S3 | `publish`, `unpacked`, package cache, `export`, `site_dir`, `set_site_url`, `own_link` | `pkg`: section 10 (pkg stages 1, 5, 7) | none: `pkg` needs no sibling, so wait for it | `own_request_*`, `site_*` pass |
| S4 | `http`, `read_channel`, `follow`, `unfollow`, `refresh` | `log`: `ingest`, `status` (L3) | none new | `follow_*`, `refresh_*` pass |
| S5 | `fetch`, `open_link`, `backup`, `restore`, `confirm_own` | `log`: backup and own-head confirmation (L4) | none new | `fetch_*`, `restore_*`, `j3_dist` pass with the real `../log` and `../pkg` |
| S7 (step 5) | 5.11 provider bases in `fetch`; `Fetched.link`; `Query.origin` | nothing new | none | A36 to A38 pass; the step 3 and 4 tests still pass unchanged |
| S6 (step 4) | `list`, `include`, `unlist` bodies and rules 9 to 14; `Statement` and `Change` additions; `follows.json` fields; the read set, `choose_index`, `drop_index`, `indexes`, three-pass `refresh`; `search`, `Query`, `Hit` and the marks | nothing new: `log` and `pkg` as for S5 | none | `body_vectors` and `derive_rules` with the step 4 rows, `index_*`, `search_*` and `j4_dist` pass; the step 3 tests still pass unchanged |

J3 itself passes only with `host`, `app` and `dev` (spec/TECHNICAL.md
section 8). `app` integrates `dist` after S5.

## 9. Provides to other subsystems

`app` uses all of section 3, as written there; the re-exports mean it
needs no direct `log` dependency. The uses:

| Items of section 3 | Use |
|---|---|
| `open`, `identity`, `fingerprint`, `backup`, `restore`, `confirm_own`, `own_confirmed` | first start (F-APP-2), backup and restore (F-APP-6) |
| `Confirm`, `SignRequest`, `Statement` | the consent view; a release is shown by its `ReleaseRef` (D-A1) |
| `set_name`, `publish`, `withdraw`, `set_site_url`, `own_link`, `site_dir`, `export` | publish (J3), and the local API request `publish` of `watane-dev` |
| `follow`, `refresh`, the views, `latest`, `RefreshReport` | the channel views, update and withdrawal notices |
| `open_link`, `fetch`, `Fetched { package, entry, .. }` | through `app` to `host.plan`: "a package with its release" |
| the link text of 4.2, the site folder of 4.3 | `watane-dev publish` prints both (through `app`'s local API) |

## 10. Needs from other subsystems

Exact signatures assumed: `log/TECHNICAL.md` section 2 and
`pkg/TECHNICAL.md` sections 3 to 8 (decision D-A3), used as written.

**`log`** (`watane_log`), the items of log 2 that dist uses: `MAX_ENTRY`;
`PublicKey` (`from_bytes(&[u8; 32]) -> Result<_, KeyTextError>`,
`as_bytes`, `fingerprint`, text form); `EntryId` (`as_bytes`, text form);
`Entry` (`decode`, `bytes`, `id`, `author`, `seq`, `kind`, `body`); `Head`,
`Ingest`, `Status`, `Fork`; `Log` (`open`, `identity`, `append`, `ingest`,
`entries`, `head`, `status`, `export_identity`, `restore_identity`,
`own_head_confirmed`, `confirm_own_head`); `DecodeError`, `KeyTextError`,
`Error`. A stub (section 8) copies these signatures from log 2.

`dist` checks compromise and the own-head confirmation before it asks the
app, so `log`'s `Compromised` and `OwnHeadUnconfirmed` errors on `append`
are a second guard, not a path the participant sees.

**`pkg`** (`watane_pkg`):

```rust
// Text forms (pkg 3), each with FromStr, Display, Clone, Debug, Eq, Ord, Hash.
pub struct Slug;  pub struct Version;
pub struct PublisherKey;  // from_bytes([u8; 32]), as_bytes() -> &[u8; 32]
pub struct PackageId;     // of(&[u8]) (BLAKE3), as_bytes(); text: 64 lower hex
pub struct ReleaseRef { pub publisher: PublisherKey, pub slug: Slug, pub version: Version }   // text: <key>/<slug>@<version>
pub const MAX_PACKAGE_BYTES: u64;                    // 67_108_864
pub const RELEASE_KIND: &str;                        // "release"
pub struct Mod;           // only passed from unpack to Release::new and check
pub struct Package;       // Clone, Debug
impl Package {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Package, PackageError>;
    pub fn id(&self) -> PackageId;
    pub fn as_bytes(&self) -> &[u8];
    pub fn into_bytes(self) -> Vec<u8>;
    pub fn unpack(&self, dest: &Path) -> Result<Mod, PackageError>;  // dest must not exist
}
pub struct Release { pub slug: Slug, pub version: Version, pub package: PackageId, pub size: u64, /* pkg 7.1 */ }
impl Release {                                       // Clone, Debug, PartialEq, Eq
    pub fn new(m: &Mod, package: &Package) -> Result<Release, ReleaseError>;
    pub fn check(&self, m: &Mod, package: &Package) -> Result<(), ReleaseError>;
    pub fn encode(&self) -> Vec<u8>;
    pub fn decode(bytes: &[u8]) -> Result<Release, ReleaseError>;
}
pub fn pack(dir: &Path, include_source: bool) -> Result<Package, PackageError>;   // tests only
pub enum PackageError;    // wraps ModError for unpack (PackageError::Mod)
pub enum ReleaseError;    // LocalProvider { slug }, Mismatch { field }, ...
```

**`host`**: the home layout reserves `<home>/dist/` for `dist` and
`<home>/log/` for `log` (opened only by `dist`); `host` never reads or
writes them, and a preview home has neither (W5). `host.plan` accepts a
source made of a `watane_pkg::Package` and its release
`watane_log::Entry`, as `Fetched` gives them.

**`app`** (matches `app/FUNCTIONAL.md` APP-F-28 to APP-F-37): implements
`Confirm`, and shows every field of `SignRequest` with the fingerprint.
Calls `dist` only from blocking threads (`tokio::task::spawn_blocking`).
Calls `refresh()` at start, every 10 minutes and on request (APP-F-36).
Routes the local API request `publish` (package bytes) to `publish`, and
`fetch` to `fetch` (a `ReleaseRef`) or `open_link` (a link). After
`restore`, asks the participant for the own link, or for the statement
that the identity never published, and calls `confirm_own`. Opens `dist`
only for the home of the participant. For a provider of a fetched mod
(APP-F-34), calls `fetch` with the `ReleaseRef` of host's
`MissingProvider`; this works when the provider's channel is followed (Q2).
A second fetch of a release uses the verified local copy and makes no
request, so app scenario S15 ("a byte of the package changes and Bob
installs again") sees the cause only for a release not fetched before.

**`dev`**: nothing directly.

## 11. Step 5

Designed in this file (S32): provider bases in 5.7 and 5.11,
`Fetched.link`, and the `origin` filter of 5.10 (F-BUILD-4). No format
changes: the origin is already in the release body (pkg 7), and the hints
are `list` and `include` statements of 4.1.

## 12. Questions for spec/

| # | Where | Finding | Proposal (used here) |
|---|---|---|---|
| Q1 | TECHNICAL §9, step 3 | The published folder, the link and the bodies are listed as not designed. | Designed in this file (4.1–4.3). Move them out of §9 and point to `dist/TECHNICAL.md`. |
| Q2 | TECHNICAL §3, W14; F-INST-4, J5 | Where to read the channel of a provider's publisher. A provider from a channel that is not followed cannot be fetched. | **Decided (S32, W25, DD-28).** D-A1 names the release (`ReleaseRef`); D-A2 fixes `deps.json` as `mods:<slug>` to a key or null. The base comes from live `list` and `include` statements in the channels the home already reads (5.11); a publish lists its foreign providers in the own channel. |
| Q3 | F-APP-6, F-PUB-7, J3 | A restore of an identity with no copy of its channel starts again at seq 0, and the next publish forks the own channel. | As `log` proposes: no statement until the own head is confirmed. `confirm_own(link)` reads the own channel from its link; `confirm_own(None)` only after the participant states it never published. F-APP-6 should name this step. |
| Q4 | F-PUB-6 | "keeps a followed channel up to date" has no trigger: static files cannot push. | `dist` has no timer. The app calls `refresh` at start, every 10 minutes, and on request (APP-F-36). |
| Q5 | F-PUB-2, F-PUB-5 | Whether the bytes of a withdrawn version stay published. | The statement stays forever, the built-in `site/` removes withdrawn package files (DD-12), and external exports retain them for the maker to remove from the host if needed. |
| Q6 | W15 | The body of `profile` is not specified. | Name only (4.1). The key is the identity; the name is a claim the app shows beside the fingerprint. |
| Q7 | F-PUB-1, F-PUB-7 | One identity on two devices forks its channel. | The first version supports one device per identity. Document it in spec §6 "Not protected". |
| Q8 | TECHNICAL §2 | `app` depends on `host` and `dist` only, but the backup (F-APP-6) is `log`'s. | `dist` passes `backup`, `restore` and `confirm_own` through; only `dist` opens `<home>/log/`. Say so in §2. |
| Q9 | spec/ (none) | spec/ sets no limit on the size of a package; `pkg` drafts 64 MiB. | 64 MiB everywhere: `pkg` refuses at pack and decode, `dist` at publish and as the body limit of a fetch. Name it in spec §3. |
| Q11 | spec TECHNICAL §9, W15 | The four open points of step 4 (S26). | Decided in 4.1, 5.9, 5.10 and DD-18 to DD-24; spec W20 to W23 name them. |
| Q12 | log | A listed channel that no index lists any more leaves its entries in `<home>/log/`; storage grows with every channel ever listed. | Step 4 keeps them (they are small, and a later listing reuses them). A `Log::forget(key)` is a later log stage. |
| Q10 | dist §5.5 and §5.3 | A restored home can read its release entries but has no packages for the required site export. | DD-13: fetch and verify live packages from the supplied own link before confirming the head and exporting. |

## 13. Decisions of this subsystem

| # | Decision | Reason |
|---|---|---|
| DD-1 | Blocking API with `ureq`; no async runtime. | Sequential reads; simplest for a library; `app` wraps calls. |
| DD-2 | One file per entry plus `head`. | Incremental refresh; immutable files cache well; matches a fork check by seq. |
| DD-3 | `head` holds the newest entry itself, not a pointer. | It is verifiable, and a fork at the head is seen in one request. |
| DD-4 | Link `watane:<key>@<base>[#<slug>]`. | The key pins the channel; the base is only a location; one form for a channel and a mod. |
| DD-5 | `profile` and `withdraw` bodies are deterministic CBOR arrays. | Same encoding family as `log`'s envelope; strict decoding has one form. |
| DD-6 | `http` and `https` are both allowed. | Integrity does not depend on transport, and V4 allows any static host, also on a local network. |
| DD-7 | "Newest" means the highest seq, not the highest version. | The publisher's order is signed; no version order is needed from `pkg`. |
| DD-8 | A fetch asks `pkg` whether release and package agree (PKG-F-23); dist compares no field itself. | One rule in one place; `pkg` owns the body. |
| DD-9 | `dist` keeps `<home>/dist/site/` current after every statement. | J3 is then: publish, confirm, copy one folder. |
| DD-10 | A follow is added only after a first successful read. | A mistyped link leaves nothing behind. |
| DD-11 | In S1/S2 the empty `site/` directory stays 0700; from S3, open creates it with 0755 and export writes 0644 files. | S21a-2 AC10a supersedes the S1/S2 private empty site rule. |
| DD-12 | S3 removes withdrawn package files only from the built-in `site/`; external export never deletes files. Section 4.3 states this exception for that managed directory. | S21a-2 AC6 requires the built-in site to contain exactly the live packages; §4.3 and Q5 still apply to external exports. |
| DD-13 | After reading a restored identity's channel, `confirm_own(Some(link))` fetches missing live packages from that base with the same hash, package and release checks as fetch, before confirming the head. | Export requires these packages; an unsuccessful recovery must leave a restored head unconfirmed. No follow record is created for the own channel. |
| DD-14 | The HTTP reader probes at most one byte beyond the acceptance limit and rejects any excess; the accepted body limit remains exactly that of §4.5. This refines the `limit(limit)` pseudocode there. | ureq 3.5's limit reader refuses at the boundary before probing EOF; an exact `release.size` package must succeed. |
| DD-15 | A reader that reads up to a new head also reads `entries/<head seq>` and requires its bytes to equal `head` (`HeadMismatch`). | S21a user-sim: a damaged `entries/<head seq>` went unnoticed, because readers took the head entry from `head`. |
| DD-16 | `confirm_own` with a link of another key is `NotOwnLink` ("this link is not your channel"), not `WrongChannel`, which stays for a host that serves another channel. | S21a user-sim: the participant gave a wrong link, not a wrong host. |
| DD-17 | Kept: a first `follow` returns every failure as `Err`; `refresh` reports `Failed` per channel. The app maps the first to a command error and the second to the row's status. | S21a user-sim saw two shapes; they answer two questions (follow or not; how is a followed channel). |
| DD-18 | A `list` names a mod (key, slug) and where to read it, not a version or a package id. | The publisher's channel is the one authority for versions, withdrawals and bytes (F-FIND-4); an index need not sign again for each version. Undo: a new kind `list-release` with a package id; readers that ignore it keep working (rule 1). |
| DD-19 | One removal kind, `unlist`, for both `list` and `include`, naming the entry id. | Same shape as `withdraw`; one rule. Undo: cheap before any index exists, a new kind after. |
| DD-20 | The body carries the link text of 4.2, not a key and a URL apart. | One parser and one form for people and statements. Undo: a new kind. |
| DD-21 | A chosen index's own releases count as listed; a `list` of the own channel is refused. | F-FIND-1 says an index lists the releases of its owner; listing them again would only add noise. |
| DD-22 | The read set is three passes; listed channels get a record in `follows.json` with `followed: false`. | Freshness marks need `last_attempt` and `last_success` for each channel a hit comes from; one file keeps one rule. Undo: cheap (local file, serde defaults). |
| DD-23 | Search is a local filter over stored channels: words as lower-case substrings, filters by key and interface, an order made only from the participant's choices, at most 200 hits. | Offline (F-QUAL-1); no global ranking (V6, non-goals); good enough for a few thousand mods. Undo: cheap, reader-only; a later full-text index changes no format. |
| DD-24 | Installing a hit follows the publisher. | Updates and withdrawals keep coming after an index drops the mod (F-INST-7, F-INST-8). Undo: cheap (app). |
| DD-25 | A listed channel reads every listed base in order (at most 4 per refresh), ingesting each against the head stored so far. The best outcome wins (`Current` > `Behind` > `Failed`); any `Current` sets `last_success`. The base serving the highest head moves first. Channel caps and per-base entry limits still apply. | A mirror serving exactly the stored head can hide a later withdrawal, including after an outage promoted it; stopping at `Current` is insufficient. Undo: cheap, reader-only. |
| DD-26 | A listed base on `localhost` or a local IP literal is skipped; followed and chosen links may use any base. | A third party's listing must not make the participant's device probe its own network; the participant's own links keep DD-6. Undo: cheap (one check); DNS names that resolve locally stay open. |
| DD-27 | Caps: 512 listed channels, 128 per chosen index, 1000 entry files per read of a listed channel. | Bounded refresh even with hostile indexes; one index cannot starve the others. Undo: cheap (Options). |
| DD-28 | A provider's base comes from live `list` and `include` statements in the channels already read, not from a link in `deps.json` or the release body. | No new format in published bytes; the consumer's publisher states it with a statement it can end (`unlist`) or repeat with a new base. Undo: cheap, reader-only; a field in a later body version (a new kind) can add a base hint later. |
| DD-29 | A channel read through a provider base gets no record; the caller follows `Fetched.link` after an install. | A failed or declined install leaves no follow behind; an installed provider gets updates like a hit (DD-24). Undo: cheap (app). |
| DD-30 | The derived mods of F-BUILD-4 are a search filter on `Release.origin` (publisher and slug, any version) over the channels already read. | Offline and bounded like search (DD-23); "as far as the chosen indexes list them". Undo: cheap, reader-only. |
