# log: technical specification

How `log` does what `FUNCTIONAL.md` says. This file is the one place for
every byte that `log` writes or reads. `log` depends on no watane subsystem.

## 1. Crate and layout

One crate at the root of this folder: package `watane-log`, lib
`watane_log`. No binary.

```text
log/
  Cargo.toml
  src/lib.rs        Log, Error, the limits; re-exports
  src/key.rs        PublicKey, z-base-32, the strict check of a key and a signature
  src/entry.rs      Entry, EntryId, DecodeError; encode, decode, sign
  src/identity.rs   the key file: create, read, replace; the backup format
  src/store.rs      the SQLite channel store: append, ingest, reads, forks
  src/schema.sql    the schema, version 1
  tests/oracle/mod.rs   independent helpers (section 8.1); never imports watane_log
  tests/*.rs        one file for each group of section 8.2
```

The inner modules are private. Only the items of section 2 are public.

## 2. Public API

```rust
pub const MAX_KIND: usize = 32;            // bytes
pub const MAX_BODY: usize = 65_536;        // bytes
pub const MAX_ENTRY: usize = 65_728;       // bytes of the largest valid entry encoding
pub const MAX_SEQ: u64 = (1 << 53) - 1;    // also the largest time
pub const BACKUP_LEN: usize = 108;

/// An Ed25519 public key. Every value is canonical and not of small order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PublicKey([u8; 32]);
impl PublicKey {
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<PublicKey, KeyTextError>;
    pub fn as_bytes(&self) -> &[u8; 32];
    pub fn fingerprint(&self) -> String;   // the first 10 characters of the text form
}
impl Display for PublicKey;                // 52 characters of z-base-32 (section 4.1)
impl FromStr for PublicKey { type Err = KeyTextError; }
impl Debug for PublicKey;                  // "PublicKey(<text>)"

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeyTextError {
    #[error("a key text has 52 characters, not {0}")]  Length(usize),
    #[error("a key text has only z-base-32 characters")] Alphabet,
    #[error("the unused bits of the key text are not zero")] UnusedBits,
    #[error("the bytes are not a canonical Ed25519 point")] NotCanonical,
    #[error("the key is a point of small order")] SmallOrder,
}

/// The BLAKE3 hash of the encoding of an entry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId([u8; 32]);
impl EntryId {
    pub fn from_bytes(bytes: [u8; 32]) -> EntryId;
    pub fn as_bytes(&self) -> &[u8; 32];
}
impl Display for EntryId;                  // 64 lower-case hex characters
impl FromStr for EntryId { type Err = IdTextError; }  // exactly 64 of [0-9a-f]
impl Debug for EntryId;                    // "EntryId(<hex>)"
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an entry id is 64 lower-case hex characters")]
pub struct IdTextError;

/// A verified entry. The only constructors are `Entry::decode` and
/// `Log::append`, so every value passed the strict rule of section 4.5.
#[derive(Clone, PartialEq, Eq)]
pub struct Entry { /* raw bytes, id and the parsed fields */ }
impl Entry {
    pub fn decode(bytes: &[u8]) -> Result<Entry, DecodeError>;
    pub fn bytes(&self) -> &[u8];          // the exact encoding (section 4.3)
    pub fn id(&self) -> EntryId;
    pub fn author(&self) -> PublicKey;
    pub fn seq(&self) -> u64;
    pub fn prev(&self) -> Option<EntryId>; // None only at seq 0
    pub fn time(&self) -> u64;             // ms since the Unix epoch, as the author's clock said
    pub fn kind(&self) -> &str;
    pub fn body(&self) -> &[u8];
}
impl Debug for Entry;                      // id, author, seq, kind; not the body

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Head { pub seq: u64, pub id: EntryId }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ingest {
    pub stored: Vec<EntryId>,   // newly stored, in seq order
    pub gap: Option<u64>,       // the lowest seq that was kept out because it does not follow the head
    pub status: Status,         // after this call
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status { Ok, Compromised(Box<Fork>) }

/// Two validly signed entries of one author that cannot both be in one chain.
/// Either `other.seq == chain.seq` and the ids differ, or
/// `other.seq == chain.seq + 1` and `other.prev() != Some(chain.id())`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fork { pub chain: Entry, pub other: Entry }

/// The identity and the channel store of one directory. Send + Sync.
/// Every method blocks: async callers use `spawn_blocking`.
pub struct Log { /* Mutex<Inner { dir, signing key, rusqlite::Connection }> */ }
impl Log {
    pub fn open(dir: &Path) -> Result<Log, Error>;                 // creates the identity at the first open
    pub fn identity(&self) -> PublicKey;
    pub fn append(&self, kind: &str, body: &[u8]) -> Result<Entry, Error>;
    pub fn ingest(&self, author: &PublicKey, entries: &[Entry]) -> Result<Ingest, Error>;
    pub fn entries(&self, author: &PublicKey, from_seq: u64, limit: usize) -> Result<Vec<Entry>, Error>;
    pub fn head(&self, author: &PublicKey) -> Result<Option<Head>, Error>;
    pub fn status(&self, author: &PublicKey) -> Result<Status, Error>;
    pub fn export_identity(&self, passphrase: &str) -> Result<Vec<u8>, Error>;   // BACKUP_LEN bytes
    pub fn restore_identity(&self, backup: &[u8], passphrase: &str) -> Result<PublicKey, Error>;
    pub fn own_head_confirmed(&self) -> Result<bool, Error>;
    pub fn confirm_own_head(&self) -> Result<(), Error>;
}
impl Debug for Log;                        // "Log { identity: <text> }"; never the secret
```

`Log` holds one `Mutex`. A poisoned mutex is recovered with
`PoisonError::into_inner`: each write is one SQLite transaction or one
atomic rename, so a panic leaves no half state.

## 3. Errors

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {                      // in the order of the checks of section 4.6
    #[error("the entry has {0} bytes; the limit is 65728")]           TooLarge(usize),
    #[error("not an entry: {0}")]                                       Malformed(&'static str),
    #[error("not the deterministic encoding of an entry")]              NotDeterministic,
    #[error("the author is not a valid key: {0}")]                      Author(KeyTextError),
    #[error("the kind is not 1 to 32 bytes of [a-z0-9-] starting with a letter")] Kind,
    #[error("the body has {0} bytes; the limit is 65536")]              BodyTooLarge(usize),
    #[error("seq or time is above 2^53 - 1")]                           OutOfRange,
    #[error("seq 0 has a previous id, or a later seq has none")]        Prev,
    #[error("the signature does not verify under the strict rule")]     Signature,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("exposed: {path} has mode {mode:04o}; group and others must have no access (chmod 700 for the directory, 600 for the key)")]
    Exposed { path: PathBuf, mode: u32 },
    #[error("{path} is not a {expected} (a symbolic link is refused)")]
    NotPlain { path: PathBuf, expected: &'static str }, // expected: "directory" or "regular file"
    #[error("the log directory path is empty")]
    EmptyPath,                                        // Log::open("")
    #[error("corrupt key file {path}: it must hold exactly 32 bytes")]
    CorruptKey { path: PathBuf },
    #[error("channel store: {0}")]
    Storage(String),                                  // SQLite failures, unknown schema version
    #[error("corrupt store: entry {seq} of {author}: {cause}")]
    CorruptStore { author: PublicKey, seq: u64, cause: String },
    #[error("invalid kind {0:?}: 1 to 32 bytes of [a-z0-9-], starting with a letter")]
    Kind(String),
    #[error("the body has {0} bytes; the limit is 65536")]
    BodyTooLarge(usize),
    #[error("the own channel is full")]
    ChannelFull,                                      // next seq above MAX_SEQ
    #[error("the channel of {0} is compromised; no entry can be added")]
    Compromised(PublicKey),
    #[error("the own channel was restored; read its newest entries and confirm the head before you append")]
    OwnHeadUnconfirmed,
    #[error("entry {index} has author {found}, not {expected}")]
    AuthorMismatch { index: usize, expected: PublicKey, found: PublicKey },
    #[error("the passphrase is empty")]
    EmptyPassphrase,
    #[error("bad backup: {field}: expected {expected}, found {found}")]
    BadBackup { field: &'static str, expected: String, found: String }, // field: length, magic, memory, passes, lanes
    #[error("wrong passphrase or damaged backup")]
    WrongPassphrase,
    #[error("the current identity {0} has signed entries; it cannot be replaced")]
    IdentityInUse(PublicKey),
}
```

Each message names the cause (F-QUAL-3). `dist` shows it unchanged.

## 4. Formats

### 4.1 Text form of a public key

- 52 characters of z-base-32, alphabet `ybndrfg8ejkmcpqxot1uwisza345h769`,
  most significant bit first, no padding. The last character carries 1 bit
  of key and 4 unused bits, which must be zero.
- A parser refuses another length, an upper-case letter, a character outside
  the alphabet and non-zero unused bits, in that order of checks. Then
  `PublicKey::from_bytes` applies.
- The fingerprint is the first 10 characters. It is for display only; no code
  compares fingerprints.
- The text is a valid DNS label (lower-case letters and digits, 52 < 63).
- Example (RFC 8032 test 1): bytes `d75a9801…f707511a` give
  `47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy`.

`PublicKey::from_bytes` accepts 32 bytes only if they decompress to a point
(ed25519-dalek `VerifyingKey::from_bytes`), the point compresses back to the
same 32 bytes, and `is_weak()` is false.

### 4.2 Text form of an entry id

64 lower-case hex characters. A parser refuses upper case.

### 4.3 Entry encoding

An entry is the CBOR array of 7 items, in the core deterministic encoding of
RFC 8949 section 4.2.1: definite lengths, and each head in its shortest form.

| # | Field | CBOR | Bytes |
|---|---|---|---|
| | header | array(7) | `87` |
| 1 | author | byte string of 32 | `58 20` + 32 bytes |
| 2 | seq | unsigned integer, 0 ≤ seq ≤ 2^53−1 | shortest form |
| 3 | prev | null at seq 0; else byte string of 32: the id of the entry at seq − 1 | `f6`, or `58 20` + 32 bytes |
| 4 | time | unsigned integer, 0 ≤ time ≤ 2^53−1, ms since the Unix epoch | shortest form |
| 5 | kind | text string, 1 to 32 bytes, grammar `[a-z][a-z0-9-]*` | `61`..`77`, or `78 18`..`78 20`, + bytes |
| 6 | body | byte string, 0 to 65 536 bytes; opaque to `log` | shortest head + bytes |
| 7 | sig | byte string of 64: Ed25519 R ‖ S | `58 40` + 64 bytes |

Shortest form of an unsigned integer n (major type 0; for a length, the same
with major type 2 or 3): n ≤ 23 → one byte `n`; n ≤ 0xff → `18 n`;
n ≤ 0xffff → `19` + 2 bytes big-endian; n ≤ 0xffffffff → `1a` + 4 bytes;
else `1b` + 8 bytes.

MAX_ENTRY = 1 + 34 + 9 + 34 + 9 + 34 + (5 + 65 536) + 66 = 65 728.

The encoding is self-delimiting, so a sequence of entries can be stored back
to back. The container of a published folder is `dist`'s (section 11).

### 4.4 Signed bytes and entry id

- **Signed bytes** = the 15 bytes `77 61 74 61 6e 65 2f 65 6e 74 72 79 2f 31 00`
  (ASCII `watane/entry/1` and a zero byte) ‖ `86` ‖ items 1 to 6 encoded as in
  section 4.3. The prefix separates the domain: no other watane signature can
  equal an entry signature.
- **Signature**: Ed25519 of RFC 8032 section 5.1.6 (pure; no context, no
  prehash) with the identity key over the signed bytes. One message gives one
  signature.
- **Entry id** = BLAKE3-256 of the whole entry encoding (all 7 items).

Golden vectors. The secret key is the seed of RFC 8032 test 1
(`9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60`).
The signatures were made with Node.js 25 `crypto.sign`, an implementation
independent of ed25519-dalek, and the ids with the `blake3` crate.

| | Entry 0 | Entry 1 |
|---|---|---|
| seq, prev | 0, null | 1, id of entry 0 |
| time | 1767225600000 | 1767225601000 |
| kind, body | `profile`, ASCII `hello` | `release`, `01 02 03` |
| length | 126 | 157 |
| id | `26e9f56befea2e5c43ba15e0fbfb28ac527d50d703ea3db5b0a6d89732a2fa20` | `4f193a17cf9a3f16a29b3a58da0ac358cf6ff3eef28669a2ce8e4e4367abd176` |

```text
entry 0:
875820d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a00
f61b0000019b76daa8006770726f66696c654568656c6c6f58401783608165a9ea936ffa
3b538b1e0918f9a47521ed025a22a37e5082710b4c1ddae418c709d8e90d7fa273e1be68
aeeb772bfb6c44bc1c2c28dd570c7b8c4c02
entry 1:
875820d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a01
582026e9f56befea2e5c43ba15e0fbfb28ac527d50d703ea3db5b0a6d89732a2fa201b00
00019b76daabe86772656c656173654301020358402b5caac5bf68e6075addc9d84f83f7
37ed5c9c42bbbe904d9cb55f8c9696246ebb3b3bd03c2d3a1d2cd28fd986d507d3acbc66
b89cd61cf886b16b9db53c9107
```

Join the lines of each entry before you decode the hex.

### 4.5 The strict rule

An entry signature (R ‖ S) under author key A is valid if and only if:
A and R are canonical encodings, neither A nor R has small order, S < L,
and [S]B = R + [k]A without the cofactor, where k = SHA-512(R ‖ A ‖ signed
bytes) mod L. Code: `PublicKey::from_bytes` (section 4.1) for A, then
ed25519-dalek `VerifyingKey::verify_strict`, which refuses a small-order R,
a non-canonical R (it compares the compressed R) and S ≥ L (the
`legacy_compatibility` feature must stay off). This is the only signature
check in watane. Plain RFC 8032 and ZIP 215 accept more; a looser rule would
accept entries that a watane reader refuses.

### 4.6 Decode

`Entry::decode(bytes)` applies these checks in this order and returns the
first failure:

1. `bytes.len() ≤ MAX_ENTRY`, else `TooLarge`.
2. Parse: an array header of 7; a byte string of 32; an unsigned integer
   (u64); null or a byte string of 32; an unsigned integer; a text string
   (UTF-8); a byte string; a byte string of 64; then the end of the input.
   Else `Malformed` with the item name.
3. Encode the parsed fields by section 4.3. If the result differs from
   `bytes`, then `NotDeterministic`. This refuses long heads, indefinite
   lengths and tags.
4. `PublicKey::from_bytes(author)`, else `Author`.
5. Kind grammar and length, else `Kind`. Body length ≤ 65 536, else
   `BodyTooLarge`.
6. seq ≤ MAX_SEQ and time ≤ MAX_SEQ, else `OutOfRange`.
7. `(seq == 0) == prev.is_none()`, else `Prev`.
8. The strict rule of section 4.5, else `Signature`.

`decode` does not check the chain. Only the store checks `prev` against a
stored entry (section 5.2).

### 4.7 The directory of a `Log`

The caller chooses the directory. `host` reserves `<home>/log/` for it
(section 11).

```text
<dir>/                 mode 0700
  identity.key         mode 0600; exactly 32 bytes: the Ed25519 secret key (seed) of RFC 8032
  channels.db          mode 0600; SQLite, with channels.db-wal and channels.db-shm
```

No macOS keychain. The key is a file, like an SSH key. The reasons: the
threat model of `spec/TECHNICAL.md` section 6 does not protect against
another program of the same user, and the keychain would not change that for
an unsigned program. A file works the same on macOS and Linux and in
headless tests. A backup must give out the seed anyway. A keychain is a
later hardening.

`Log::open(dir)`:
1. An empty path is `EmptyPath` ("the log directory path is empty").
2. If `dir` is missing, create it and its missing parents with mode 0700
   (`DirBuilder::recursive(true).mode(0o700)`).
3. `symlink_metadata(dir)`: not a directory is `NotPlain { expected: "directory" }`; `mode & 0o077 != 0`
   is `Exposed`.
4. `symlink_metadata(dir/identity.key)`:
   - missing: create the key (below);
   - not a regular file (a link included): `NotPlain { expected: "regular file" }`;
   - `mode & 0o077 != 0`: `Exposed`, and the file is not changed;
   - else read it. A length other than 32 is `CorruptKey`.
5. Open the store (section 4.8).

Create the key: 32 bytes from `getrandom::fill`. Write them to
`.identity.key.<32 hex random>.tmp` with `create_new` and mode 0600, then
`sync_all`, then `fs::hard_link` to `identity.key`, which fails if the name
exists. On `AlreadyExists` another opener won: remove the temporary file and
read the winner's file. Then remove the temporary file and `sync_all` the
directory. Hold the seed in `zeroize::Zeroizing`.

Replace the key (restore only): write the temporary file in the same way,
then `fs::rename` it over `identity.key`, then `sync_all` the directory.

### 4.8 The channel store

`channels.db`: create the empty file with `create_new` and mode 0600 before
SQLite opens it, so that SQLite copies that mode to the `-wal` and `-shm`
files. Pragmas at each open: `journal_mode = WAL`, `synchronous = FULL`,
`busy_timeout = 5000`. Retry `journal_mode = WAL` on `SQLITE_BUSY` for up to
5 s (two first opens race on it). At `user_version` 0 with no table, run
`schema.sql` in one `BEGIN IMMEDIATE` transaction. Any other version than 1
is `Storage("unsupported schema version <n>")`, and the file is not changed.

```sql
CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) STRICT;
CREATE TABLE entries (
  author BLOB NOT NULL, seq INTEGER NOT NULL, id BLOB NOT NULL UNIQUE,
  raw BLOB NOT NULL, PRIMARY KEY (author, seq)) STRICT;
CREATE TABLE forks (
  author BLOB PRIMARY KEY, chain BLOB NOT NULL, other BLOB NOT NULL) STRICT;
CREATE TRIGGER entries_no_update BEFORE UPDATE ON entries BEGIN SELECT RAISE(ABORT, 'immutable'); END;
CREATE TRIGGER entries_no_delete BEFORE DELETE ON entries BEGIN SELECT RAISE(ABORT, 'immutable'); END;
CREATE TRIGGER forks_no_update BEFORE UPDATE ON forks BEGIN SELECT RAISE(ABORT, 'immutable'); END;
CREATE TRIGGER forks_no_delete BEFORE DELETE ON forks BEGIN SELECT RAISE(ABORT, 'immutable'); END;
PRAGMA user_version = 1;
```

- `entries.raw` is the entry encoding; `author`, `seq` and `id` are copies
  for lookup. `forks.chain` and `forks.other` are the encodings of the two
  entries of the proof.
- `meta` has at most one row: `key = 'unconfirmed'`, `value` = the 32 bytes
  of an identity restored in this directory whose own head is not confirmed.
- **Read check.** Each read of a row decodes `raw` with `Entry::decode` and
  checks that the columns equal the decoded author, seq and id. A fork row
  must decode to two entries of its `author` in the relation of `Fork`. A
  failure is `CorruptStore` and changes nothing.
- The code uses plain `INSERT`, never `REPLACE`.

### 4.9 Identity backup

108 bytes. Integers are u32 little-endian.

| Offset | Length | Field |
|---|---|---|
| 0 | 8 | magic: ASCII `watanek1` |
| 8 | 4 | Argon2id memory in KiB; an export writes 65536 |
| 12 | 4 | Argon2id passes; an export writes 3 |
| 16 | 4 | Argon2id lanes; an export writes 4 |
| 20 | 16 | salt, random for each export |
| 36 | 24 | nonce, random for each export |
| 60 | 48 | XChaCha20-Poly1305 ciphertext of the 32-byte seed, the 16-byte tag at the end |

- Password: the UTF-8 bytes of the NFC form of the passphrase. An empty
  passphrase (after NFC) is `EmptyPassphrase`, at export and at restore.
- Key: Argon2id version 0x13, the header parameters, the salt, 32 bytes of
  output, no secret, no associated data. The parameters are the second
  recommendation of RFC 9106.
- Cipher: XChaCha20-Poly1305 with the 32-byte key and the nonce; the
  associated data is bytes 0 to 59.
- Restore refuses, before Argon2id runs, with `BadBackup { field, expected, found }` ("bad backup: <field>: expected <e>, found <f>", e.g. `expected 108 bytes, found 107 bytes`): a length
  other than 108 (`"length"`), another magic (`"magic"`), memory outside
  8192..=1048576 or below 8 × lanes (`"memory"`), passes outside 1..=10
  (`"passes"`), lanes outside 1..=16 (`"lanes"`). A failed tag is
  `WrongPassphrase`.
- `log` gives bytes. The app chooses the file name and how to show it.

Golden vector: the seed of RFC 8032 test 1, passphrase `correct horse`, salt
`00 01 … 0f`, nonce `10 11 … 27`. Node.js 25 (`crypto.argon2Sync`,
`chacha20-poly1305` with an HChaCha20 subkey) decrypts it to the seed, so the
vector does not depend on the Rust crates.

```text
776174616e656b31000001000300000004000000000102030405060708090a0b0c0d0e0f
101112131415161718191a1b1c1d1e1f2021222324252627e2d8d174adbc08bfce3774a8
a87915ecea1faa48eab9e5bc6b51f1727ca433d267325d6d08ee1b378c413ad95e167c5d
```

## 5. Behaviour

### 5.1 Append

`append(kind, body)`:
1. Check the kind grammar (`Kind`) and the body length (`BodyTooLarge`).
2. Lock; `BEGIN IMMEDIATE`.
3. A fork row for the identity: `Compromised`. A `meta` row `unconfirmed`
   equal to the identity: `OwnHeadUnconfirmed`.
4. Read the head of the identity (read check). seq = head.seq + 1, or 0;
   above MAX_SEQ is `ChannelFull`. prev = head.id.
5. time = the system clock in ms since the epoch, clamped to 0..=MAX_SEQ.
6. Sign the signed bytes (section 4.4), build the encoding, insert, commit.
7. Return the entry.

`append` never asks. Its caller asks the participant first (W13, F-AGENT-4).

### 5.2 Ingest

`ingest(author, entries)`:
1. If an entry has another author: `AuthorMismatch` for the first one;
   nothing is written.
2. Lock; `BEGIN IMMEDIATE`. If a fork row exists for `author`: commit nothing
   and return `stored = []`, `gap = None` and the compromised status.
3. Sort the entries by (seq, id bytes) and drop repeated ids.
4. For each entry e, with h the current head (read check):
   - an entry s is stored at e.seq: if s.id = e.id, skip e. Else insert the
     fork row (chain = s, other = e) and stop the loop;
   - e.seq = h.seq + 1 (or 0 with no head): if e.prev = h.id (both absent at
     seq 0), insert e and make it the head. Else insert the fork row
     (chain = the head entry, other = e) and stop the loop;
   - e.seq > h.seq + 1: keep e out; set gap to e.seq if gap is unset.
5. Commit. The entries stored before a fork in the same call stay stored.
6. Return the stored ids, the gap and the status.

A channel starts only at seq 0, so each stored channel is a whole chain
from its first entry. `ingest` accepts entries of the own identity: that is
how the own head is learned after a restore. A fork in the own channel marks
it compromised like any other.

### 5.3 Reads

- `entries(author, from_seq, limit)`: the stored entries with
  seq ≥ from_seq, in seq order, at most `limit`. A `from_seq` above i64::MAX
  gives an empty list.
- `head(author)`: the entry with the highest seq, or None.
- `status(author)`: validate all stored entries of the author, then return
  `Compromised` with the checked fork row, or `Ok`. A corrupt stored entry
  must not appear as a healthy channel (S26).
- Each read applies the read check (section 4.8).

`log` keeps every stored entry, also of a channel that `dist` no longer
follows. Nothing deletes an entry or a proof.

### 5.4 Backup and restore

- `export_identity(passphrase)`: section 4.9 with fresh salt and nonce from
  `getrandom`.
- `restore_identity(backup, passphrase)`:
  1. Decode and decrypt (section 4.9) before taking the lock, because
     Argon2id takes about a second. Derive the public key P.
  2. Lock. If P is the current identity: return P, change nothing.
  3. If the own channel of the current identity has an entry:
     `IdentityInUse`.
  4. In one transaction, write the `meta` row `unconfirmed` = P.
  5. Replace the key file (section 4.7). Replace the key in memory.
  6. Return P.
  A crash between steps 4 and 5 leaves the old key; the participant runs the
  restore again.
- `own_head_confirmed()`: false only if the `unconfirmed` row equals the
  identity.
- `confirm_own_head()`: delete the `unconfirmed` row. The caller calls it
  after it ingested the own channel from its published copy, or after the
  participant states that the identity never published.

A `Log` reads its key at open and after its own restore. Each directory has
one `Log` in one process (the app's `dist`); a second `Log` of the same
directory keeps its old key after a restore.

## 6. Libraries

The colos prototype `../colos/crates/colos-log` compiled and passed its tests
with these versions (its `Cargo.lock`). Edition 2024, rust-version 1.91.

```toml
[package]
name = "watane-log"
version = "0.1.0"
edition = "2024"
rust-version = "1.91"
license = "MIT OR Apache-2.0"

[lib]
name = "watane_log"

[dependencies]
ed25519-dalek = "3.0"            # default features (fast, zeroize); never legacy_compatibility
getrandom = "0.4"                # getrandom::fill
blake3 = "1.8"
minicbor = { version = "2.3", default-features = false, features = ["alloc"] }
argon2 = { version = "0.6", default-features = false, features = ["alloc"] }
chacha20poly1305 = { version = "0.11", default-features = false, features = ["alloc"] }
rusqlite = { version = "0.41", features = ["bundled"] }
data-encoding = "2.11"           # z-base-32 (custom Specification) and HEXLOWER
unicode-normalization = "0.1.26"
zeroize = "1.9"
thiserror = "2.0"

[dev-dependencies]
tempfile = "3.27"
ed25519-compact = "2.6"          # oracle signer: KeyPair::from_seed(Seed::new(seed)), kp.sk.sign(msg, None)
proptest = "1.12"

[lints.rust]
unsafe_code = "forbid"

[profile.dev.package.argon2]
opt-level = 3
[profile.dev.package.blake2]
opt-level = 3
```

z-base-32 with `data-encoding`: `Specification::new()`, push the alphabet to
`symbols`, keep the default `check_trailing_bits = true`, no padding. The
colos file `src/zbase32.rs` does exactly this.

## 7. Security properties

| Property | Enforced by |
|---|---|
| The secret key stays on the device | It is read into `Zeroizing` and a `SigningKey` (zeroized on drop). No API returns it. `Debug` shows only the public key. The only output is the encrypted backup. |
| Other users cannot read or plant the key | The directory must be 0700-like and the key 0600-like (no group or other bit); a link or a non-regular file is refused. The key is created with `create_new` and published by `hard_link`, so it never overwrites. |
| No signing oracle | No public `sign`. A signature only exists inside an entry of the own channel, with a domain prefix (section 4.4). |
| One rule for everyone | `Entry` has no public constructor except `decode` and `append`, and `decode` applies section 4.6 in full. |
| One byte form, one id | The decoder re-encodes and compares (section 4.6 step 3). |
| Forks are found and kept | Section 5.2; the fork row is immutable; a compromised channel takes no further entry, and the own channel takes no append. |
| No accidental self-fork after a restore | `OwnHeadUnconfirmed` (section 5.4). |
| Stored data cannot silently change | The read check on every row; SQLite triggers refuse UPDATE and DELETE. |
| Bounded work on hostile input | `MAX_ENTRY` before parsing; backup parameter bounds before Argon2id. |
| Durability | WAL with `synchronous = FULL`; `sync_all` of the key and its directory. |

Not protected, as in `spec/TECHNICAL.md` section 6: another program of the
same user (it can read the key file), a compromised operating system, and a
web host that withholds new entries.

## 8. Test plan

Commands: `cargo test` (all), `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`. Property tests use proptest with
`PROPTEST_RNG_SEED` fixed in CI (default seed 0) and 256 cases.

### 8.1 Oracles (`tests/oracle/mod.rs`)

The oracle never calls `watane_log` to compute an expected value.

- `cbor::entry(author, seq, prev, time, kind, body, sig)` and
  `cbor::signed(...)`: a byte builder that writes section 4.3 by hand. It
  can also write a long head, an indefinite length or a map, for refusal
  cases.
- `zbase32(bytes)`: a bit-by-bit encoder, independent of `data-encoding`.
- `sign(seed, msg)`: `ed25519-compact`, an Ed25519 implementation
  independent of ed25519-dalek.
- `blake3`: the `blake3` crate is the hash primitive, not code under test.
- The golden vectors of sections 4.4 and 4.9, made with Node.js.
- `model::Chain`: an in-memory model of section 5.2 written in the test
  (a `BTreeMap<seq, id>`, a head, an optional fork pair), for S28.
- The RFC 8032 test 1 key pair.

### 8.2 Tests

| File | Scenarios | Checks | Oracle |
|---|---|---|---|
| `key_text.rs` | S5, S6 | text form, parse refusals, small-order and non-canonical keys (`0100…00`; `eeff…ff7f`, which is y = p + 1) | RFC 8032 key; `zbase32()`; hand-made bytes |
| `identity_file.rs` | S1–S4 | modes 0700/0600, length 32, reopen, refusals leave the file unchanged | `std::fs::metadata`; RFC 8032 seed written by the test |
| `entry_codec.rs` | S16, S17 | golden vectors decode with the right fields and id; each refusal of section 4.6 gives its error: 65 729 bytes; 6 and 8 items; a map; trailing byte; seq as `18 05`; indefinite body; uppercase kind; kind of 33 bytes; body of 65 537 bytes (built under the size limit with a short kind); seq 2^53; seq 0 with prev; seq 1 without prev; a flipped signature bit | golden vectors; `cbor::`; `sign()` |
| `strict_verify.rs` | S18 | S + L refused; A = `0100…00`, R = `0100…00`, S = 0 refused (a cofactorless check alone accepts it); non-canonical A refused; a valid `sign()` entry accepted | hand-made bytes; little-endian addition of L in the test |
| `append.rs` | S15, S19 | seq 0..2, prev = BLAKE3 of the previous bytes, signature valid under `ed25519_compact::PublicKey::verify` over `cbor::signed`; refusals leave the head unchanged | `cbor::`, `ed25519-compact`, `blake3` |
| `ingest.rs` | S20–S25 | shuffled order, gap, duplicates, both fork forms, no store after a fork, reopen keeps the proof, append refused | entries signed with `sign()` from a known seed; expected heads written in the test |
| `corrupt.rs` | S26 | the test drops the update trigger with its own `rusqlite::Connection`, flips one byte of `raw`, and expects `CorruptStore` on `head`, `entries` and `status` | the test's own SQL |
| `backup.rs` | S7–S13 | golden vector restores to the RFC 8032 key; export then restore gives the same key; the export decrypts with `argon2` + `chacha20poly1305` called directly in the test; NFC; bounds refused fast (< 50 ms); `IdentityInUse` | golden vector; direct crate calls |
| `restore_head.rs` | S14 | `OwnHeadUnconfirmed`, ingest of the own entries, confirm, next append at seq 3 with prev = id of entry 2 | entries made by a first `Log` in another directory |
| `concurrency.rs` | S27 | two `Log`s on one directory, two threads, 50 appends each; then seq 0..99 and every prev link | `cbor::` re-check of each prev with `blake3` |
| `model.rs` | S28 | proptest: random batches of valid, duplicate, gapped and forked entries of 2 authors | `model::Chain` |

## 9. Build plan

`log` needs no sibling. It can be built and finished before `dist`. Each
stage ends with `cargo test` and clippy green.

| Stage | Build | Done when | Needs | Fake until then |
|---|---|---|---|---|
| L1 Keys | `key.rs`, `PublicKey`, `EntryId`, text forms; `identity.rs` key file | `key_text.rs`, `identity_file.rs` pass | none | none |
| L2 Entries | `entry.rs`: encode, decode, strict rule | `entry_codec.rs`, `strict_verify.rs` pass | none | none |
| L3 Store | `store.rs`, `schema.sql`, `Log::open/append/ingest/entries/head/status` | `append.rs`, `ingest.rs`, `corrupt.rs`, `concurrency.rs`, `model.rs` pass | none | none |
| L4 Backup | backup format, `export_identity`, `restore_identity`, own-head confirmation | `backup.rs`, `restore_head.rs` pass | none | none |
| L5 Use | `dist` publishes and follows with `Log`; `host` verifies a release with `Entry::decode` | the tests of `dist` and `host` that use `log` pass, then J3 | `dist`, `host` | none in `log`; `dist` and `host` need no fake of `log` |

Step 4 (find) needs no new stage. If `dist` later needs entries of many
channels by kind, a schema version 2 adds an index on a `kind` column; the
`user_version` check is the place for that migration. Not designed now.

## 10. Provides to other subsystems

| Item | For | Use |
|---|---|---|
| `PublicKey` and its text form (section 4.1) | `pkg`, `host`, `dist`, `app`, `dev` | the publisher of a mod, `deps.json`, links, display. `pkg` stores the text only (it has no dependency on `log`). |
| `EntryId` and its text form (section 4.2) | `dist` | an entry of a channel names another entry of the same channel (the `withdraw` body). Outside its channel, a release is named by `pkg`'s `ReleaseRef` (spec W16). |
| `Entry::decode`, `Entry` getters, `DecodeError` | `host`, `dist` | `host` checks the release entry that comes with a package (F-INST-6): decode, then `kind() == "release"`, then the body with `pkg`. |
| `Entry::bytes` | `dist` | the exact bytes that `dist` writes to the published folder and reads back. |
| `Log` (all of section 2) | `dist` only | the identity, the own channel, followed channels, backup and restore. |
| `Status`, `Fork`, `Ingest`, `Head` | `dist`, through `dist` to `app` | gap and compromise reports (F-PUB-6, F-PUB-7). |
| `MAX_KIND`, `MAX_BODY` | `dist`, `pkg` | `pkg` keeps an encoded `Release` body within `MAX_BODY`. |

## 11. Needs from other subsystems

`log` needs nothing to build. For use, it relies on these points:

- **host**: the home layout reserves `<home>/log/` for `log`. Only `dist`
  opens it. A preview home has no `log/` (W5).
- **dist**: opens `Log::open(&home.join("log"))`. It calls `append` only
  after the confirmation of the app (W13). It calls `ingest` with the
  entries of one channel at a time, from seq 0 for a new channel. It stops
  updates of a channel whose status is `Compromised` (F-PUB-7). After
  `restore_identity`, it reads the own channel from its link, ingests it and
  then calls `confirm_own_head`, or asks the participant to confirm that
  the identity never published. Its kinds (`profile`, `release`,
  `withdraw`, W15) fit the kind grammar.
- **pkg**: refers to a publisher by the text of section 4.1 and checks the
  syntax only (52 characters of the alphabet). `host` and `dist` convert
  with `PublicKey::from_str` for the full check.
- **app**: gives the passphrase, saves the 108 backup bytes to a file of the
  participant's choice, and offers restore (F-APP-6) through `dist`.

`Log` is one type for the key and the store, because the own-head
confirmation after a restore ties the key to the store.

## 12. Questions for spec/

1. **Text form of a public key.** Decided: 52-character z-base-32 (section
   4.1); spec/TECHNICAL.md section 3 names it.
2. **Name of a release.** Decided (spec W16): `<publisher key>/<slug>@<version>`
   outside its channel; the entry id only inside it.
3. **Backup format** (`spec/TECHNICAL.md` section 9, step 3). Proposal used:
   section 4.9, format `watanek1`.
4. **Restore after the first start.** F-APP-2 creates an identity at the
   first start, and F-APP-6 restores one. Proposal used: a restore replaces
   the current identity only if it signed no entry (LOG-F-6). F-APP-6 should
   say so.
5. **Restore and the own head.** After a restore, `log` does not know the
   newest own entry. An append would fork the own channel and mark it
   compromised for every follower (F-PUB-7). Proposal used: no append until
   the caller confirms the head (LOG-F-8). J3 or F-APP-6 should name this
   step; `dist` and `app` need a flow for it.
6. **"The same position" in F-PUB-7.** Proposal used: two entries cannot
   both be in one chain (same seq and other id, or the next seq with another
   prev). Both need the key, so both prove a fork. F-PUB-7 should use this
   wording.
7. **Keychain.** spec/ is silent. Proposal used: a 0600 file, by the threat
   model of section 6; the keychain is a later hardening.
8. **Signature domain.** Proposal used: each signature covers the prefix
   `watane/entry/1` and a zero byte (section 4.4), so a later signature kind
   cannot be confused with an entry. No conflict; record it as a decision.
9. **Unfollow keeps entries.** F-PUB-6 is silent. Proposal used: `log` keeps
   every stored entry and proof (LOG-F-18); `dist` only stops reading.
10. **Range of seq and time.** Proposal used: at most 2^53 − 1, so a JSON
    reader (`watane.js`, the app) never loses precision.

11. **Exposed diagnostic (log).** Include "exposed" in `Error::Exposed` so the message names the refusal required by S2.

12. **Status read check (log).** Clarified section 5.3: status validates the stored channel entries as well as its fork proof, so S26 cannot report a corrupt channel as healthy.
