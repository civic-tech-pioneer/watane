//! Kept connections and sync between own machines (TECHNICAL 5.2, 5.6).
//!
//! A keeper task dials each own machine whose address is known, keeps one
//! connection to it, calls `hello` on it, and syncs: on connect, on a hint
//! and every `sync_interval`. Everything another machine sends is checked
//! before the holder sees it; a machine that sends a bad object is dropped
//! for the session.

use crate::{
    Class, Core, Event, HeadInfo, Holdings, Ingested, NetError, Role, Why,
    client::{self},
    gossip,
    key::NodeKey,
    lock,
    serve::Serve,
};
use iroh::endpoint::Connection;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use watane_log::PublicKey;

const TICK: Duration = Duration::from_millis(250);
/// Two connections made within this long of each other may be a race of two
/// dials: the one of the smaller key wins.
const RACE: Duration = Duration::from_secs(2);
/// The machine of the larger key waits this long before it dials.
const DIAL_GRACE: Duration = Duration::from_secs(1);
const REDIAL_MIN: Duration = Duration::from_millis(500);
const REDIAL_MAX: Duration = Duration::from_secs(5);
/// Channels asked for in one `heads` request.
const HEADS_CHUNK: usize = 1000;

/// The one connection kept to an own machine.
pub(crate) struct Link {
    conn: Connection,
    initiator: NodeKey,
    born: Instant,
}

/// What is to be synced with one machine, and whether a sync runs now.
#[derive(Default)]
pub(crate) struct Slot {
    running: bool,
    full: bool,
    hints: BTreeMap<PublicKey, (u64, [u8; 32])>,
    last_full: Option<Instant>,
}

/// The state of the redial of one machine.
pub(crate) struct Dial {
    since: Instant,
    busy: bool,
    next: Instant,
    backoff: Duration,
}

/// What a sync of one machine is asked to do.
pub(crate) enum Want {
    /// 5.6, steps 1 to 4.
    Everything,
    /// Step 2 for one channel (a hint: seq and id as told).
    Hint(PublicKey, u64, [u8; 32]),
}

impl Core {
    /// The live kept connection to `node`.
    pub(crate) fn link_of(&self, node: &NodeKey) -> Option<Connection> {
        lock(&self.links)
            .get(node)
            .filter(|l| l.conn.close_reason().is_none())
            .map(|l| l.conn.clone())
    }

    pub(crate) fn is_dropped(&self, node: &NodeKey) -> bool {
        lock(&self.dropped).contains(node)
    }

    /// A bad object: report the node and drop it for the session (NET-F-24).
    pub(crate) fn refuse(&self, node: &NodeKey, cause: String) {
        let first = lock(&self.dropped).insert(*node);
        if first {
            // every cause names the node by its short key (NET-F-32)
            let short = node.short();
            let cause = if cause.contains(&short) {
                cause
            } else {
                format!("node {short}: {cause}")
            };
            self.emit(Event::Refused { node: *node, cause });
        }
        if let Some(link) = lock(&self.links).get(node) {
            link.conn.close(2u32.into(), b"bad object");
        }
    }

    /// This machine's role in the newest list (a light machine without one).
    pub(crate) fn role(&self) -> Role {
        self.devices()
            .and_then(|l| {
                l.devices
                    .iter()
                    .find(|(k, _)| *k == self.key)
                    .map(|(_, r)| *r)
            })
            .unwrap_or(Role::Light)
    }

    /// Remember an address where `node` was reached, first in line.
    pub(crate) fn learn_addr(&self, node: &NodeKey, addr: std::net::SocketAddr) {
        self.remember(node, &[addr], true);
    }
}

/// The role this machine syncs as: a full replica keeps everything; the
/// anchor of the list keeps only what its holder asks for, as a light
/// machine does (NET-F-21, node Q5).
pub(crate) async fn role_of(core: &Core) -> Role {
    if core.role() == Role::Light || core.devices().is_some_and(|l| l.anchor == core.key) {
        return Role::Light;
    }
    match holder(core, |h| h.is_anchor()).await {
        Ok(true) => Role::Light,
        _ => Role::Full,
    }
}

/// Run the keeper until it is aborted.
pub(crate) async fn keeper(core: Arc<Core>) {
    loop {
        tokio::time::sleep(TICK).await;
        tick(&core);
        gossip::maintain(&core).await;
    }
}

fn tick(core: &Arc<Core>) {
    let Some(list) = core.devices() else { return };
    let interval = core.limits().sync_interval;
    for (node, _) in &list.devices {
        if *node == core.key {
            continue;
        }
        if core.class_of(node) != Class::Trusted {
            continue;
        }
        if core.is_dropped(node) {
            continue;
        }
        if core.link_of(node).is_some() {
            let due = {
                let syncs = lock(&core.syncs);
                syncs
                    .get(node)
                    .and_then(|s| s.last_full)
                    .is_some_and(|t| t.elapsed() >= interval)
            };
            if due {
                request(core, *node, Want::Everything);
            }
        } else {
            maybe_dial(core, *node);
        }
    }
    // a machine that is no longer trusted keeps no connection
    let stale: Vec<Connection> = lock(&core.links)
        .iter()
        .filter(|(node, _)| core.class_of(node) != Class::Trusted)
        .map(|(_, l)| l.conn.clone())
        .collect();
    for conn in stale {
        conn.close(1u32.into(), b"not an own machine");
    }
}

fn maybe_dial(core: &Arc<Core>, node: NodeKey) {
    if core.addrs_of(&node).is_empty() {
        return;
    }
    {
        let mut dials = lock(&core.dials);
        let dial = dials.entry(node).or_insert(Dial {
            since: Instant::now(),
            busy: false,
            next: Instant::now(),
            backoff: REDIAL_MIN,
        });
        if dial.busy || Instant::now() < dial.next {
            return;
        }
        // of two machines that know each other, the smaller key dials first;
        // the larger waits a moment for the call, so that two dials do not cross
        if core.key > node && dial.since.elapsed() < DIAL_GRACE {
            return;
        }
        dial.busy = true;
    }
    let core = core.clone();
    tokio::spawn(async move {
        let up = dial(&core, node).await;
        let mut dials = lock(&core.dials);
        if let Some(d) = dials.get_mut(&node) {
            d.busy = false;
            if up {
                d.backoff = REDIAL_MIN;
            } else {
                d.backoff = (d.backoff * 2).min(REDIAL_MAX);
            }
            d.next = Instant::now() + d.backoff;
        }
    });
}

/// Connect to `node` and keep the connection. `true`: a connection is kept.
async fn dial(core: &Arc<Core>, node: NodeKey) -> bool {
    let Ok(conn) = client::connect(core, &node, crate::wire::ALPN).await else {
        return false;
    };
    // the machine may call us on the connection we dialed
    let serve = Serve { core: core.clone() };
    let for_serve = conn.clone();
    tokio::spawn(async move {
        use iroh::protocol::ProtocolHandler;
        let _ = serve.accept(for_serve).await;
    });
    adopt(core.clone(), node, conn, core.key).await
}

/// Keep `conn` as the connection to `node` when it is the one to keep: call
/// `hello` on it, tell `PeerUp`, sync.
pub(crate) async fn adopt(
    core: Arc<Core>,
    node: NodeKey,
    conn: Connection,
    initiator: NodeKey,
) -> bool {
    if core.is_dropped(&node) || core.class_of(&node) != Class::Trusted {
        return false;
    }
    if !wins(&core, &node, initiator) {
        return false;
    }
    match client::hello_async(&core, &node, Some(&conn)).await {
        Ok(_) => {}
        Err(NetError::BadObject { cause, .. }) => {
            core.refuse(&node, cause);
            conn.close(2u32.into(), b"bad object");
            return false;
        }
        Err(_) => {
            conn.close(1u32.into(), b"no hello");
            return false;
        }
    }
    let id = conn.stable_id();
    let announce = {
        let mut links = lock(&core.links);
        let keep = match links.get(&node) {
            Some(old) if old.conn.close_reason().is_none() => {
                if old.initiator == initiator {
                    // the same side dialed again: the newer connection
                    // replaces the older
                    Some(false)
                } else if old.born.elapsed() < RACE && initiator < old.initiator {
                    Some(false)
                } else {
                    None
                }
            }
            _ => Some(true),
        };
        match keep {
            None => return false,
            Some(announce) => {
                if let Some(old) = links.insert(
                    node,
                    Link {
                        conn: conn.clone(),
                        initiator,
                        born: Instant::now(),
                    },
                ) {
                    old.conn.close(0u32.into(), b"replaced");
                }
                announce
            }
        }
    };
    if initiator != core.key {
        learn_from(&core, &node, &conn);
    }
    if announce {
        core.emit(Event::PeerUp(node));
    }
    {
        let core = core.clone();
        let conn = conn.clone();
        tokio::spawn(async move {
            conn.closed().await;
            let gone = {
                let mut links = lock(&core.links);
                if links.get(&node).is_some_and(|l| l.conn.stable_id() == id) {
                    links.remove(&node);
                    true
                } else {
                    false
                }
            };
            if gone {
                core.emit(Event::PeerDown(node));
            }
        });
    }
    if announce {
        request(&core, node, Want::Everything);
    }
    true
}

/// Is a new connection of `initiator` to `node` to be kept, given what is
/// kept now? Cheap check before `hello`.
fn wins(core: &Core, node: &NodeKey, initiator: NodeKey) -> bool {
    let links = lock(&core.links);
    match links.get(node) {
        Some(old) if old.conn.close_reason().is_none() => {
            old.initiator == initiator || (old.born.elapsed() < RACE && initiator < old.initiator)
        }
        _ => true,
    }
}

/// The address a trusted machine called from is where to reach it.
fn learn_from(core: &Core, node: &NodeKey, conn: &Connection) {
    let addr = conn.paths().iter().find_map(|p| match p.remote_addr() {
        iroh::TransportAddr::Ip(addr) => Some(*addr),
        _ => None,
    });
    if let Some(addr) = addr {
        core.learn_addr(node, addr);
    }
}

/// Ask for a sync with `node`. Only one runs at a time for a machine; what
/// is asked while one runs is done after it.
pub(crate) fn request(core: &Arc<Core>, node: NodeKey, want: Want) {
    {
        let mut syncs = lock(&core.syncs);
        let slot = syncs.entry(node).or_default();
        match want {
            Want::Everything => slot.full = true,
            Want::Hint(c, seq, id) => {
                let entry = slot.hints.entry(c).or_insert((seq, id));
                if seq > entry.0 {
                    *entry = (seq, id);
                }
            }
        }
        if slot.running {
            return;
        }
        slot.running = true;
    }
    let core = core.clone();
    tokio::spawn(async move {
        loop {
            let (full, hints) = {
                let mut syncs = lock(&core.syncs);
                let slot = syncs.entry(node).or_default();
                if !slot.full && slot.hints.is_empty() {
                    slot.running = false;
                    return;
                }
                if slot.full {
                    slot.last_full = Some(Instant::now());
                }
                (
                    std::mem::take(&mut slot.full),
                    std::mem::take(&mut slot.hints),
                )
            };
            if core.is_dropped(&node) || lock(&core.endpoint).is_none() {
                continue;
            }
            let result = if full {
                sync_all(&core, &node).await
            } else {
                let mut result = Ok(());
                for (channel, (seq, id)) in hints {
                    result = result.and(sync_channel(&core, &node, &channel, seq, id).await);
                }
                result
            };
            if let Err(NetError::BadObject { cause, .. }) = result {
                core.refuse(&node, cause);
            }
        }
    });
}

/// Run a holder call on a blocking thread.
pub(crate) async fn holder<T: Send + 'static>(
    core: &Core,
    f: impl FnOnce(&dyn Holdings) -> T + Send + 'static,
) -> Result<T, NetError> {
    let holdings = core.holdings.clone();
    tokio::task::spawn_blocking(move || f(holdings.as_ref()))
        .await
        .map_err(|_| NetError::Io {
            path: core.dir.clone(),
            cause: "the holder failed".into(),
        })
}

/// 5.6, steps 1 to 4.
async fn sync_all(core: &Arc<Core>, node: &NodeKey) -> Result<(), NetError> {
    let role = role_of(core).await;
    let asked: Vec<PublicKey> = match role {
        Role::Full => Vec::new(),
        Role::Light => holder(core, |h| h.heads(None))
            .await?
            .into_iter()
            .map(|(c, _)| c)
            .collect(),
    };
    // a light machine with no channel asks for none: an empty list would
    // ask for all
    if role == Role::Full || !asked.is_empty() {
        let mut remote = Vec::new();
        if role == Role::Full {
            remote = client::heads_async(core, node, &[]).await?;
        } else {
            for chunk in asked.chunks(HEADS_CHUNK) {
                remote.extend(client::heads_async(core, node, chunk).await?);
            }
        }
        let channels: Vec<PublicKey> = remote.iter().map(|(c, _)| *c).collect();
        let own: BTreeMap<PublicKey, Option<HeadInfo>> =
            holder(core, move |h| h.heads(Some(&channels)))
                .await?
                .into_iter()
                .collect();
        for (channel, head) in remote {
            let Some(head) = head else { continue };
            let own_head = own.get(&channel).copied().flatten();
            pull(core, node, role, &channel, head, own_head).await?;
        }
    }
    fetch_wanted(core, node, role).await
}

/// A hint: step 2 for one channel, with the machine that delivered it.
async fn sync_channel(
    core: &Arc<Core>,
    node: &NodeKey,
    channel: &PublicKey,
    seq: u64,
    id: [u8; 32],
) -> Result<(), NetError> {
    let role = role_of(core).await;
    let remote = client::heads_async(core, node, &[*channel]).await?;
    let Some((_, Some(head))) = remote.into_iter().next() else {
        return Ok(());
    };
    // the hint is a hint: at the seq it names, the head must be the id it names
    if head.seq == seq && head.id.as_bytes() != &id {
        return Ok(());
    }
    let key = *channel;
    let own_head = holder(core, move |h| h.heads(Some(&[key])))
        .await?
        .into_iter()
        .next()
        .and_then(|(_, h)| h);
    pull(core, node, role, channel, head, own_head).await?;
    // a new head may name a package
    fetch_wanted(core, node, role).await
}

/// Step 2 and 3 for one channel: read from `own seq + 1` to the other's seq
/// in pages, check, and hand to the holder.
pub(crate) async fn pull(
    core: &Arc<Core>,
    node: &NodeKey,
    role: Role,
    channel: &PublicKey,
    remote: HeadInfo,
    own: Option<HeadInfo>,
) -> Result<Option<Ingested>, NetError> {
    let why = match role {
        Role::Full => Why::Kept,
        Role::Light => Why::Asked,
    };
    let limits = core.limits();
    let mut stored = None;
    // what stopped the channel, when the holder did not take a page
    let mut stopped = None;
    let mut from = own.map_or(0, |h| h.seq + 1);
    if own.is_none_or(|h| remote.seq > h.seq) {
        while from <= remote.seq {
            let limit = (remote.seq - from + 1).min(limits.max_entries as u64);
            let page = client::entries_async(core, node, channel, from, limit).await?;
            let Some(last) = page.last().map(|e| e.seq()) else {
                break;
            };
            let key = *channel;
            let result = holder(core, move |h| h.ingest(&key, page, why)).await?;
            match result {
                Ingested::Stored(head) if head >= last => {
                    stored = Some(head);
                    from = head + 1;
                }
                // not wanted, compromised, failed, or a head that did not
                // move: this channel stops
                Ingested::Stored(_) => {
                    stopped = Some(Ingested::Failed("the head did not move".into()));
                    break;
                }
                other => {
                    stopped = Some(other);
                    break;
                }
            }
        }
    }
    if let Some(seq) = stored {
        core.emit(Event::Pulled {
            channel: *channel,
            seq,
            from: *node,
        });
    }
    if remote.forked
        && !own.is_some_and(|h| h.forked)
        && let Some((a, b)) = client::fork_async(core, node, channel).await?
    {
        let key = *channel;
        holder(core, move |h| {
            for entry in [a, b] {
                h.ingest(&key, vec![entry], why);
            }
        })
        .await?;
    }
    Ok(stopped)
}

/// 5.6 step 4.
async fn fetch_wanted(core: &Arc<Core>, node: &NodeKey, role: Role) -> Result<(), NetError> {
    let wanted = holder(core, move |h| h.wanted_blobs(role)).await?;
    let limit = core.limits().blob;
    for (id, size) in wanted {
        if size > limit {
            core.emit(Event::Unreachable {
                node: *node,
                cause: format!(
                    "package {} is {size} bytes, over this machine's limit of {limit}",
                    crate::blobs::hex8(&id)
                ),
            });
            continue;
        }
        match crate::blobs::fetch(core, node, id, size).await {
            Ok(()) => {}
            Err(e @ NetError::BadObject { .. }) => return Err(e),
            // a package this machine does not have, or a path that broke:
            // the next one
            Err(_) => {}
        }
    }
    Ok(())
}
