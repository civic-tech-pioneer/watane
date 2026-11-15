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
    hints: BTreeMap<PublicKey, (u64, [u8; 32], u32)>,
    last_full: Option<Instant>,
}

/// The state of the redial of one machine.
pub(crate) struct Dial {
    since: Instant,
    busy: bool,
    next: Instant,
    backoff: Duration,
    /// `Event::Unreachable` was told, and the machine has not connected since.
    reported: bool,
}

/// What a sync of one machine is asked to do.
pub(crate) enum Want {
    /// 5.6, steps 1 to 4.
    Everything,
    /// Step 2 for one channel (a hint: seq and id as told), and how many
    /// times it was asked again already (5.7).
    Hint(PublicKey, u64, [u8; 32], u32),
}

impl Core {
    /// The live kept connection to `node`.
    pub(crate) fn link_of(&self, node: &NodeKey) -> Option<Connection> {
        lock(&self.links)
            .get(node)
            .filter(|l| l.conn.close_reason().is_none())
            .map(|l| l.conn.clone())
    }

    /// Turning net off: end every kept connection with an application close
    /// code, so the other side sees it end (`PeerDown`) at once.
    pub(crate) fn close_links(&self) {
        for link in lock(&self.links).values() {
            link.conn.close(0u32.into(), b"net turned off");
        }
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
    /// Only for a machine of the device list; the book keeps at most four,
    /// newest first (4.5).
    pub(crate) fn learn_addr(&self, node: &NodeKey, addr: std::net::SocketAddr) {
        let listed = lock(&self.state)
            .list
            .as_ref()
            .is_some_and(|l| l.devices.iter().any(|(k, _)| k == node));
        if *node == self.key {
            return;
        }
        if !listed {
            // a join in progress: keep where its anchor answered, until the
            // list is installed (5.5)
            if let Some((anchor, at)) = lock(&self.joining).as_mut()
                && anchor == node
            {
                *at = Some(addr);
            }
            return;
        }
        self.remember(node, &[addr], true);
        let changed = {
            let mut stored = lock(&self.stored);
            let list = stored.entry(*node).or_default();
            if list.first() == Some(&addr) {
                None
            } else {
                list.retain(|a| *a != addr);
                list.insert(0, addr);
                list.truncate(crate::reach::MAX_ADDRS);
                Some(list.clone())
            }
        };
        if let Some(list) = changed {
            let _ = crate::reach::write_addrs(&self.dir, node, &list);
        }
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
            if let Some(d) = lock(&core.dials).get_mut(node) {
                d.reported = false;
            }
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
            reported: false,
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
        let outcome = dial(&core, node).await;
        let up = matches!(outcome, Ok(true));
        let report = {
            let mut dials = lock(&core.dials);
            let mut report = false;
            if let Some(d) = dials.get_mut(&node) {
                d.busy = false;
                if up {
                    d.backoff = REDIAL_MIN;
                    d.reported = false;
                } else {
                    d.backoff = (d.backoff * 2).min(REDIAL_MAX);
                    report = outcome.is_err() && !d.reported;
                    d.reported |= report;
                }
                d.next = Instant::now() + d.backoff;
            }
            report
        };
        // once for a machine, until it connects again
        if let (true, Err(cause)) = (report, outcome) {
            core.emit(Event::Unreachable {
                node,
                cause: format!("node {}: {cause}", node.short()),
            });
        }
    });
}

/// Connect to `node` and keep the connection. `Ok(true)`: a connection is
/// kept; `Err`: no path to the machine, and why.
async fn dial(core: &Arc<Core>, node: NodeKey) -> Result<bool, String> {
    let within = {
        let limits = core.limits();
        limits.connect_timeout.min(limits.deadline)
    };
    let conn = match client::connect_within(core, &node, crate::wire::ALPN, within).await {
        Ok(conn) => conn,
        Err(NetError::Unreachable { cause, .. }) => return Err(cause),
        Err(_) => return Ok(false),
    };
    // the machine may call us on the connection we dialed
    let serve = Serve { core: core.clone() };
    let for_serve = conn.clone();
    tokio::spawn(async move {
        use iroh::protocol::ProtocolHandler;
        let _ = serve.accept(for_serve).await;
    });
    Ok(adopt(core.clone(), node, conn, core.key).await)
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
    if let Some(addr) = client::ip_of(conn) {
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
            Want::Hint(c, seq, id, tries) => {
                let entry = slot.hints.entry(c).or_insert((seq, id, tries));
                if seq > entry.0 {
                    *entry = (seq, id, tries);
                } else if seq == entry.0 {
                    entry.2 = entry.2.min(tries);
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
                for (channel, (seq, id, tries)) in hints {
                    match sync_channel(&core, &node, &channel, seq, id, tries).await {
                        Ok(hinted) if again(&hinted, tries) => {
                            let core = core.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(HINT_RETRY).await;
                                let next = next_machine(&core, node);
                                request(&core, next, Want::Hint(channel, seq, id, tries + 1));
                            });
                        }
                        Ok(_) => {}
                        Err(e) => result = result.and(Err(e)),
                    }
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

/// How long after a hint found the machine asked behind the next one is asked.
const HINT_RETRY: Duration = Duration::from_millis(250);
/// How many times: 20 times 250 ms, 5 s in all (5.7).
const HINT_RETRIES: u32 = 20;

/// How acting on a hint ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Hinted {
    /// Done: pulled, forged, or nothing to pull.
    Done,
    /// The machine asked has a head below the hint's seq and ours is
    /// too: ask the next machine soon.
    Behind,
}

/// The machine a waiting hint is asked of next: the one after `node` in the
/// ring of the trusted machines this machine has a live link to (by key),
/// `node` itself when it is the only one. The machine that delivered a hint
/// may be one that never holds the channel (a light machine), so each retry
/// asks the next machine and the same 20 retries reach every machine (5.7).
fn next_machine(core: &Core, node: NodeKey) -> NodeKey {
    let mut ring: Vec<NodeKey> = lock(&core.links)
        .iter()
        .filter(|(k, l)| l.conn.close_reason().is_none() && **k != node)
        .map(|(k, _)| *k)
        .filter(|k| core.class_of(k) == crate::Class::Trusted && !core.is_dropped(k))
        .collect();
    ring.push(node);
    ring.sort();
    let at = ring.iter().position(|k| *k == node).unwrap_or(0);
    ring[(at + 1) % ring.len()]
}

/// Is a hint asked again after its try number `tries` (0 is the first)?
/// Only while the machine asked is behind, and at most `HINT_RETRIES`
/// times.
fn again(hinted: &Hinted, tries: u32) -> bool {
    *hinted == Hinted::Behind && tries < HINT_RETRIES
}

/// Is a hint still waiting for the machine asked? It is when
/// that machine's head is below the hint's seq and this machine's is too.
pub(crate) fn waits_for(remote: Option<HeadInfo>, own: Option<HeadInfo>, seq: u64) -> bool {
    remote.is_none_or(|h| h.seq < seq) && own.is_none_or(|h| h.seq < seq)
}

/// A hint: step 2 for one channel, with the machine asked (the one that
/// delivered it on the first try, 5.7).
async fn sync_channel(
    core: &Arc<Core>,
    node: &NodeKey,
    channel: &PublicKey,
    seq: u64,
    id: [u8; 32],
    tries: u32,
) -> Result<Hinted, NetError> {
    let role = role_of(core).await;
    let remote = client::heads_async(core, node, &[*channel]).await?;
    let head = remote.into_iter().next().and_then(|(_, h)| h);
    // the hint is a hint: at the seq it names, the head must be the id it names
    if let Some(head) = head
        && head.seq == seq
        && head.id.as_bytes() != &id
    {
        return Ok(Hinted::Done);
    }
    let key = *channel;
    let own_head = holder(core, move |h| h.heads(Some(&[key])))
        .await?
        .into_iter()
        .next()
        .and_then(|(_, h)| h);
    // the deliverer has not got the hint's seq while this machine lacks it
    // too: ask it again soon; what it has is pulled on the first and the
    // last try only (a forged seq does not keep a pull open for 5 s)
    let behind = waits_for(head, own_head, seq);
    let Some(head) = head else {
        return Ok(if behind { Hinted::Behind } else { Hinted::Done });
    };
    if behind && tries > 0 && tries < HINT_RETRIES {
        return Ok(Hinted::Behind);
    }
    pull(core, node, role, channel, head, own_head).await?;
    // a new head may name a package
    fetch_wanted(core, node, role).await?;
    Ok(if behind { Hinted::Behind } else { Hinted::Done })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntryId;

    fn head(seq: u64) -> Option<HeadInfo> {
        Some(HeadInfo {
            seq,
            id: EntryId::from_bytes([0; 32]),
            forked: false,
        })
    }

    #[test]
    fn a_hint_waits_only_while_both_heads_are_below_its_seq() {
        assert!(waits_for(None, None, 3));
        assert!(waits_for(head(2), head(1), 3));
        assert!(waits_for(head(2), None, 3));
        assert!(!waits_for(head(3), head(1), 3));
        assert!(!waits_for(head(2), head(3), 3));
        assert!(!waits_for(None, head(3), 3));
    }

    #[test]
    fn a_hint_whose_deliverer_stays_behind_is_asked_21_times_over_5_s() {
        // the first try, then one every 250 ms while behind
        let tries = 1 + (0..100).take_while(|&t| again(&Hinted::Behind, t)).count();
        assert_eq!(tries, 21);
        assert_eq!(HINT_RETRY * (tries as u32 - 1), Duration::from_millis(5000));
        assert!(!again(&Hinted::Done, 0));
    }
}
