//! Own machines (TECHNICAL 5.5): the stored device list and the anchor's
//! contact time, pairing codes, the signing of new lists, and the moves of a
//! list between machines. A list moves only forward, and only when the
//! stored anchor signed it.

use crate::error::{Code, NetError};
use crate::key::{NodeKey, zbase32};
use crate::ticket::{Hint, PairCode};
use crate::wire::{self, Request};
use crate::{Class, Core, DeviceList, Event, Role, client, lock};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const DAY_MS: u64 = 86_400_000;
/// `anchor_seen` is written to disk at most this often.
const SEEN_EVERY_MS: u64 = 60_000;
const MAX_HINTS: usize = 8;

/// The newest verified list, as its signed bytes too, and the last contact
/// with the anchor (ms since the epoch).
pub(crate) struct DevState {
    pub(crate) list: Option<DeviceList>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) anchor_seen: u64,
}

/// A code the anchor gave out and nobody has used yet.
pub(crate) struct PendingCode {
    secret: [u8; 16],
    role: Role,
    deadline: Instant,
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn io_error(path: &Path, e: io::Error) -> NetError {
    NetError::Io {
        path: path.to_owned(),
        cause: e.to_string(),
    }
}

fn corrupt(path: &Path, cause: impl Into<String>) -> NetError {
    NetError::Corrupt {
        path: path.to_owned(),
        cause: cause.into(),
    }
}

/// Temp file, fsync, rename, mode 0600 (TECHNICAL 4.5).
pub(crate) fn write_atomic(dir: &Path, name: &str, data: &[u8]) -> Result<(), NetError> {
    let path = dir.join(name);
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).map_err(|e| io_error(&path, io::Error::other(e)))?;
    let tmp = dir.join(format!(".{name}.{}.tmp", zbase32(&random)));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| io_error(&tmp, e))?;
        file.write_all(data).map_err(|e| io_error(&tmp, e))?;
        file.sync_all().map_err(|e| io_error(&tmp, e))?;
        fs::rename(&tmp, &path).map_err(|e| io_error(&path, e))?;
        File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|e| io_error(dir, e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, NetError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(path, e)),
    }
}

/// `book.json` as found, or the defaults of 4.5 for a new one.
pub(crate) fn read_book(dir: &Path) -> Result<serde_json::Value, NetError> {
    let path = dir.join("book.json");
    match read_optional(&path)? {
        None => Ok(serde_json::json!({
            "version": 1,
            "on": false,
            "dht": false,
            "relays": [],
            "addrs": {},
            "anchor_seen": 0,
        })),
        Some(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) if value.is_object() => Ok(value),
            Ok(_) => Err(corrupt(&path, "not a JSON object")),
            Err(e) => Err(corrupt(&path, e.to_string())),
        },
    }
}

fn write_anchor_seen(dir: &Path, ms: u64) -> Result<(), NetError> {
    crate::reach::update_book(dir, |book| book["anchor_seen"] = ms.into())
}

/// Read `devices`, `anchor` and `book.json`; a bad file is `Corrupt`.
pub(crate) fn load(dir: &Path) -> Result<DevState, NetError> {
    let devices = dir.join("devices");
    let (list, bytes) = match read_optional(&devices)? {
        None => (None, Vec::new()),
        Some(bytes) => {
            let list = DeviceList::verify(&bytes).map_err(|e| corrupt(&devices, e.to_string()))?;
            (Some(list), bytes)
        }
    };
    let anchor_path = dir.join("anchor");
    if let Some(text) = read_optional(&anchor_path)? {
        let anchor = std::str::from_utf8(&text)
            .ok()
            .and_then(|t| t.trim().parse::<NodeKey>().ok())
            .ok_or_else(|| corrupt(&anchor_path, "not a node key"))?;
        if let Some(list) = &list
            && list.anchor != anchor
        {
            return Err(corrupt(&anchor_path, "not the anchor of the device list"));
        }
    }
    let anchor_seen = read_book(dir)?["anchor_seen"].as_u64().unwrap_or(0);
    Ok(DevState {
        list,
        bytes,
        anchor_seen,
    })
}

/// Constant-time equality of two secrets.
fn same_secret(a: &[u8; 16], b: &[u8; 16]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl Core {
    pub(crate) fn devices(&self) -> Option<DeviceList> {
        lock(&self.state).list.clone()
    }

    /// The class of a node: in the newest verified list and not expired
    /// (5.3); the own node always.
    pub(crate) fn class_of(&self, node: &NodeKey) -> Class {
        if *node == self.key {
            return Class::Trusted;
        }
        let state = lock(&self.state);
        let Some(list) = &state.list else {
            return Class::Unknown;
        };
        if !list.devices.iter().any(|(k, _)| k == node) {
            return Class::Unknown;
        }
        if list.anchor != self.key && list.expires_days > 0 {
            let since = list.time.max(state.anchor_seen);
            let until = since.saturating_add(u64::from(list.expires_days) * DAY_MS);
            if now_ms() > until {
                return Class::Unknown;
            }
        }
        Class::Trusted
    }

    /// Contact with `node`, authenticated by the handshake: when it is the
    /// anchor, the expiry starts again (ND-17).
    pub(crate) fn seen(&self, node: &NodeKey) {
        let mut state = lock(&self.state);
        let Some(list) = &state.list else { return };
        if list.anchor != *node || *node == self.key {
            return;
        }
        let now = now_ms();
        if now > state.anchor_seen.saturating_add(SEEN_EVERY_MS) {
            state.anchor_seen = now;
            let _ = write_anchor_seen(&self.dir, now);
        }
    }

    /// Store a verified list: the file first, then the memory.
    fn store(&self, state: &mut DevState, bytes: &[u8], list: DeviceList) -> Result<(), NetError> {
        write_atomic(&self.dir, "devices", bytes)?;
        state.list = Some(list.clone());
        state.bytes = bytes.to_vec();
        self.emit(Event::DevicesChanged(list));
        Ok(())
    }

    /// A list from another node: stored only when the stored anchor signed
    /// it, it verifies and its seq is higher (NET-F-11). `Ok(false)`: not
    /// newer, so nothing changed.
    pub(crate) fn accept_list(&self, bytes: &[u8]) -> Result<bool, NetError> {
        let list = DeviceList::verify(bytes)?;
        let mut state = lock(&self.state);
        let Some(current) = &state.list else {
            return Err(NetError::BadList {
                cause: "this machine has no anchor".into(),
            });
        };
        if current.anchor == self.key {
            // this machine signs the lists; nobody else can
            return Ok(false);
        }
        if list.anchor != current.anchor {
            return Err(NetError::BadList {
                cause: "not a list of the anchor of this machine".into(),
            });
        }
        if list.seq <= current.seq {
            return Ok(false);
        }
        self.store(&mut state, bytes, list)?;
        Ok(true)
    }

    /// The list a joining machine got: stored with its anchor, whatever the
    /// seq when `replace`.
    pub(crate) fn install(
        &self,
        bytes: &[u8],
        list: DeviceList,
        replace: bool,
    ) -> Result<(), NetError> {
        let mut state = lock(&self.state);
        if state.list.is_some() && !replace {
            return Err(NetError::HasAnchor);
        }
        let now = now_ms();
        write_atomic(&self.dir, "anchor", list.anchor.to_string().as_bytes())?;
        write_anchor_seen(&self.dir, now)?;
        state.anchor_seen = now;
        self.store(&mut state, bytes, list)
    }

    /// Sign the next list of this home: `change` edits a copy of the newest
    /// one (a list of this node alone, as full, when there is none).
    pub(crate) fn sign_next(
        &self,
        change: impl FnOnce(&mut DeviceList) -> Result<(), NetError>,
    ) -> Result<(DeviceList, Vec<u8>), NetError> {
        if !self.holdings.is_anchor() {
            return Err(NetError::NotAnchor);
        }
        let mut state = lock(&self.state);
        let mut next = match &state.list {
            Some(list) if list.anchor == self.key => list.clone(),
            other => DeviceList {
                anchor: self.key,
                seq: other.as_ref().map_or(0, |l| l.seq),
                time: 0,
                expires_days: 0,
                devices: vec![(self.key, Role::Full)],
            },
        };
        change(&mut next)?;
        next.seq += 1;
        next.time = now_ms();
        next.devices.sort_by_key(|(k, _)| *k);
        let bytes = next.sign(&self.seed)?;
        self.store(&mut state, &bytes, next.clone())?;
        Ok((next, bytes))
    }

    /// A code for `role`, valid for one use until the pair deadline.
    pub(crate) fn new_code(&self, role: Role) -> Result<PairCode, NetError> {
        if !self.holdings.is_anchor() {
            return Err(NetError::NotAnchor);
        }
        let own_list = self.devices().is_some_and(|list| list.anchor == self.key);
        if !own_list {
            self.sign_next(|_| Ok(()))?;
        }
        let mut secret = [0u8; 16];
        getrandom::fill(&mut secret).map_err(|e| NetError::Io {
            path: self.dir.clone(),
            cause: e.to_string(),
        })?;
        let now = Instant::now();
        let mut codes = lock(&self.codes);
        codes.retain(|c| c.deadline > now);
        codes.push(PendingCode {
            secret,
            role,
            deadline: now + self.limits().pair_deadline,
        });
        let hints = lock(&self.endpoint)
            .as_ref()
            .map(|e| e.bound_sockets())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| !a.ip().is_unspecified())
            .take(MAX_HINTS)
            .map(Hint::Addr)
            .collect();
        Ok(PairCode {
            anchor: self.key,
            secret,
            hints,
        })
    }

    /// A code that is right, unspent and not expired is spent at once and
    /// gives its role. The secret is compared in constant time against every
    /// pending code.
    fn take_code(&self, secret: &[u8; 16]) -> Option<Role> {
        let now = Instant::now();
        let mut codes = lock(&self.codes);
        codes.retain(|c| c.deadline > now);
        let mut found = None;
        for (i, code) in codes.iter().enumerate() {
            if same_secret(&code.secret, secret) {
                found = Some(i);
            }
        }
        found.map(|i| codes.remove(i).role)
    }

    pub(crate) fn answer_join(&self, node: &NodeKey, yes: bool) {
        if let Some(tx) = lock(&self.joins).remove(node) {
            let _ = tx.send(yes);
        }
    }
}

/// The `pair` op on the anchor (5.5): spend the code, ask the participant,
/// and on yes sign the list with the joiner. Returns the whole answer.
pub(crate) async fn handle_pair(core: &Arc<Core>, caller: NodeKey, secret: [u8; 16]) -> Vec<u8> {
    let Some(role) = core.take_code(&secret) else {
        return wire::err_answer(Code::BadCode, "not a pairing code that can be used");
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    lock(&core.joins).insert(caller, tx);
    core.emit(Event::JoinAsked { node: caller, role });
    let yes = matches!(
        tokio::time::timeout(core.limits().pair_deadline, rx).await,
        Ok(Ok(true))
    );
    {
        let mut joins = lock(&core.joins);
        if joins.get(&caller).is_some_and(|tx| tx.is_closed()) {
            joins.remove(&caller);
        }
    }
    if !yes {
        return wire::err_answer(Code::Declined, "the participant did not confirm");
    }
    let signed = core.sign_next(|list| {
        match list.devices.iter_mut().find(|(k, _)| *k == caller) {
            Some(device) => device.1 = role,
            None => list.devices.push((caller, role)),
        }
        Ok(())
    });
    match signed {
        Ok((list, bytes)) => {
            tokio::spawn(push(Arc::clone(core), list, Some(caller)));
            wire::pair_answer(&bytes)
        }
        Err(_) => wire::err_answer(Code::Busy, "the list could not be signed"),
    }
}

/// Tell the other own machines that a list with this seq exists; each pulls
/// it back (4.2 `devices`). Best effort: a machine with no known address is
/// left to ask later.
pub(crate) async fn push(core: Arc<Core>, list: DeviceList, skip: Option<NodeKey>) {
    for (node, _) in &list.devices {
        if *node == core.key || Some(*node) == skip || core.addrs_of(node).is_empty() {
            continue;
        }
        let (core, node, seq) = (Arc::clone(&core), *node, list.seq);
        tokio::spawn(async move {
            let limits = core.limits();
            if let Ok(answer) = client::call_async(
                &core,
                &node,
                &Request::Devices { seq },
                wire::LIST_ANSWER,
                limits.deadline,
            )
            .await
            {
                take_answer(&core, &node, &answer);
            }
        });
    }
}

/// Ask `from` for a list newer than ours and take it when it is the
/// anchor's.
pub(crate) async fn pull(core: Arc<Core>, from: NodeKey) {
    let Some(own) = core.devices() else { return };
    let limits = core.limits();
    let Ok(answer) = client::call_async(
        &core,
        &from,
        &Request::Devices { seq: own.seq },
        wire::LIST_ANSWER,
        limits.deadline,
    )
    .await
    else {
        return;
    };
    take_answer(&core, &from, &answer);
}

/// What a `devices` answer of `from` gives: a newer list when it is the
/// anchor's, and addresses of other machines of the list, after the ones
/// known (5.10). A bad answer drops the machine for the session.
fn take_answer(core: &Core, from: &NodeKey, answer: &[u8]) {
    let parsed = match wire::parse_devices(answer) {
        Ok(parsed) => parsed,
        Err(wire::AnswerError::Bad(cause)) => return core.refuse(from, cause),
        Err(_) => return,
    };
    if let Some(bytes) = parsed.list {
        let _ = core.accept_list(bytes);
    }
    let listed = |node: &NodeKey| {
        core.devices()
            .is_some_and(|l| l.devices.iter().any(|(k, _)| k == node))
    };
    if let Some((node, _)) = parsed.addrs.iter().find(|(node, _)| !listed(node)) {
        return core.refuse(
            from,
            format!(
                "an address of {}, which is not an own machine",
                node.short()
            ),
        );
    }
    for (node, addrs) in &parsed.addrs {
        if *node != core.key {
            core.remember(node, addrs, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlobId, HeadInfo, Hit, Holdings, Ingested, Net, Options, Query, Why};
    use std::path::PathBuf;
    use watane_log::{Entry, PublicKey};

    struct Holder(bool);
    impl Holdings for Holder {
        fn is_anchor(&self) -> bool {
            self.0
        }
        fn heads(&self, _: Option<&[PublicKey]>) -> Vec<(PublicKey, Option<HeadInfo>)> {
            Vec::new()
        }
        fn entries(&self, _: &PublicKey, _: u64, _: usize, _: usize) -> Vec<Entry> {
            Vec::new()
        }
        fn fork(&self, _: &PublicKey) -> Option<(Entry, Entry)> {
            None
        }
        fn blob(&self, _: &BlobId) -> Option<PathBuf> {
            None
        }
        fn wanted_blobs(&self, _: Role) -> Vec<(BlobId, u64)> {
            Vec::new()
        }
        fn ingest(&self, _: &PublicKey, _: Vec<Entry>, _: Why) -> Ingested {
            Ingested::NotWanted
        }
        fn store_blob(&self, _: &BlobId, _: &Path) -> Result<(), String> {
            Ok(())
        }
        fn search(&self, _: &Query) -> Vec<Hit> {
            Vec::new()
        }
    }

    fn open(dir: &Path, anchor: bool) -> Net {
        Net::open(
            &dir.join("net"),
            Arc::new(Holder(anchor)),
            Options::default(),
        )
        .unwrap()
    }

    fn key(n: u8) -> (NodeKey, [u8; 32]) {
        let seed = [n; 32];
        (crate::key::public_of(&seed), seed)
    }

    fn list(seed: &[u8; 32], seq: u64, others: &[NodeKey]) -> (DeviceList, Vec<u8>) {
        let anchor = crate::key::public_of(seed);
        let mut devices: Vec<_> = others.iter().map(|k| (*k, Role::Light)).collect();
        devices.push((anchor, Role::Full));
        devices.sort_by_key(|(k, _)| *k);
        let list = DeviceList {
            anchor,
            seq,
            time: now_ms(),
            expires_days: 0,
            devices,
        };
        let bytes = list.sign(seed).unwrap();
        (list, bytes)
    }

    /// A replica whose list is `seq` 5 of the anchor of seed 1.
    fn replica(dir: &Path) -> (Net, [u8; 32], NodeKey) {
        let net = open(dir, false);
        let (anchor, seed) = key(1);
        let (_, bytes) = list(&seed, 5, &[net.node_key()]);
        let c = net.core();
        c.install(&bytes, DeviceList::verify(&bytes).unwrap(), false)
            .unwrap();
        (net, seed, anchor)
    }

    /// AC7: a list moves only forward, and only the stored anchor's.
    #[test]
    fn ac7_a_list_moves_only_forward_and_only_the_anchors() {
        let dir = tempfile::tempdir().unwrap();
        let (net, seed, anchor) = replica(dir.path());
        let me = net.node_key();
        let before = net.devices().unwrap();
        assert_eq!((before.seq, before.anchor), (5, anchor));
        let core = net.core();

        // a lower and an equal seq
        for seq in [4, 5] {
            let (_, bytes) = list(&seed, seq, &[me]);
            assert!(!core.accept_list(&bytes).unwrap());
            assert_eq!(net.devices().unwrap(), before);
        }
        // a list naming the anchor whose signature another key made: it does
        // not verify
        let (_, other_seed) = key(2);
        let (_, mut forged) = list(&seed, 9, &[me]);
        let (_, other) = list(&other_seed, 9, &[me]);
        let n = forged.len();
        forged[n - 64..].copy_from_slice(&other[other.len() - 64..]);
        core.accept_list(&forged).unwrap_err();
        assert_eq!(net.devices().unwrap(), before);
        // a list that does not name the stored anchor
        let (_, bytes) = list(&other_seed, 9, &[me]);
        assert!(matches!(
            core.accept_list(&bytes),
            Err(NetError::BadList { .. })
        ));
        assert_eq!(net.devices().unwrap(), before);
        // a higher one of the anchor is taken, and stored as bytes
        let (newer, bytes) = list(&seed, 6, &[me]);
        assert!(core.accept_list(&bytes).unwrap());
        assert_eq!(net.devices().unwrap(), newer);
        assert_eq!(fs::read(dir.path().join("net/devices")).unwrap(), bytes);
    }

    /// AC2: a reopen gives the same list, the files are 0600, and bad files
    /// are `Corrupt`.
    #[test]
    fn ac2_storage_reopens_and_bad_files_are_corrupt() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (net, _, anchor) = replica(dir.path());
        let kept = net.devices().unwrap();
        let class = net.class_of(&anchor);
        drop(net);
        let net_dir = dir.path().join("net");
        for name in ["devices", "anchor", "book.json"] {
            let mode = fs::metadata(net_dir.join(name))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o7777, 0o600, "{name}");
        }
        let book: serde_json::Value =
            serde_json::from_slice(&fs::read(net_dir.join("book.json")).unwrap()).unwrap();
        assert_eq!(book["version"], 1);
        assert!(book["anchor_seen"].as_u64().unwrap() > 0);
        let again = open(dir.path(), false);
        assert_eq!(again.devices().unwrap(), kept);
        assert_eq!(again.class_of(&anchor), class);
        assert_eq!(class, Class::Trusted);
        drop(again);

        let good = fs::read(net_dir.join("devices")).unwrap();
        let mut bad = good.clone();
        let n = bad.len();
        bad[n - 1] ^= 1;
        fs::write(net_dir.join("devices"), &bad).unwrap();
        assert!(matches!(
            Net::open(&net_dir, Arc::new(Holder(false)), Options::default()),
            Err(NetError::Corrupt { .. })
        ));
        fs::write(net_dir.join("devices"), &good).unwrap();
        fs::write(net_dir.join("anchor"), "not a key").unwrap();
        assert!(matches!(
            Net::open(&net_dir, Arc::new(Holder(false)), Options::default()),
            Err(NetError::Corrupt { .. })
        ));
    }

    /// 5.5: the anchor's list never expires for the anchor, whatever
    /// `anchor_seen` says; the same list expires on a replica.
    #[test]
    fn the_anchor_never_expires_for_itself() {
        let (other, _) = key(3);
        let stale = |seed: &[u8; 32]| {
            let anchor = crate::key::public_of(seed);
            let mut devices = vec![(anchor, Role::Full), (other, Role::Light)];
            devices.sort_by_key(|(k, _)| *k);
            DeviceList {
                anchor,
                seq: 2,
                time: now_ms() - 2 * DAY_MS,
                expires_days: 1,
                devices,
            }
            .sign(seed)
            .unwrap()
        };
        let dir = tempfile::tempdir().unwrap();
        let seed = *open(dir.path(), true).core().seed;
        fs::write(dir.path().join("net/devices"), stale(&seed)).unwrap();
        assert_eq!(open(dir.path(), true).class_of(&other), Class::Trusted);

        let dir = tempfile::tempdir().unwrap();
        let (_, anchor_seed) = key(1);
        drop(open(dir.path(), false));
        fs::write(dir.path().join("net/devices"), stale(&anchor_seed)).unwrap();
        assert_eq!(open(dir.path(), false).class_of(&other), Class::Unknown);
    }

    #[test]
    fn a_book_json_keeps_the_fields_it_had() {
        let dir = tempfile::tempdir().unwrap();
        let net_dir = dir.path();
        fs::write(
            net_dir.join("book.json"),
            r#"{"version":1,"on":true,"relays":["https://r.example"],"extra":7,"anchor_seen":3}"#,
        )
        .unwrap();
        write_anchor_seen(net_dir, 99).unwrap();
        let book: serde_json::Value =
            serde_json::from_slice(&fs::read(net_dir.join("book.json")).unwrap()).unwrap();
        assert_eq!(book["anchor_seen"], 99);
        assert_eq!(book["extra"], 7);
        assert_eq!(book["on"], true);
        assert_eq!(book["relays"][0], "https://r.example");
    }

    #[test]
    fn a_code_is_spent_by_its_first_use_and_compared_whole() {
        let dir = tempfile::tempdir().unwrap();
        let net = open(dir.path(), true);
        let code = net.pair_code(Role::Light).unwrap();
        let core = net.core();
        let mut wrong = code.secret;
        wrong[15] ^= 1;
        assert_eq!(core.take_code(&wrong), None);
        assert_eq!(core.take_code(&code.secret), Some(Role::Light));
        assert_eq!(core.take_code(&code.secret), None);
        // a replica cannot give a code, an anchor signs a list of itself first
        assert_eq!(net.devices().unwrap().seq, 1);
        let other = tempfile::tempdir().unwrap();
        assert!(matches!(
            open(other.path(), false).pair_code(Role::Full),
            Err(NetError::NotAnchor)
        ));
    }

    /// AC5: the addresses of a `devices` answer are checked against the list
    /// as it stands after a newer list of the same answer is taken; a machine
    /// of no list drops the callee.
    #[test]
    fn ac5_a_devices_answer_names_only_machines_of_the_list() {
        let dir = tempfile::tempdir().unwrap();
        let (net, seed, anchor) = replica(dir.path());
        let (me, core) = (net.node_key(), net.core());
        let at: std::net::SocketAddr = "10.0.0.7:4433".parse().unwrap();
        // a machine just paired, in the newer list the answer carries
        let (fresh, _) = key(3);
        let (_, bytes) = list(&seed, 6, &[me, fresh]);
        let answer = wire::devices_answer(Some(&bytes), &[(fresh, vec![at])]);
        take_answer(core, &anchor, &answer);
        assert!(!core.is_dropped(&anchor));
        assert_eq!(net.devices().unwrap().seq, 6);
        assert_eq!(core.addrs_of(&fresh), vec![at]);
        // a machine of no list
        let (stranger, _) = key(4);
        let answer = wire::devices_answer(None, &[(stranger, vec![at])]);
        take_answer(core, &anchor, &answer);
        assert!(core.is_dropped(&anchor));
        assert!(core.addrs_of(&stranger).is_empty());
    }
}
