//! A light machine asks a full one (TECHNICAL 5.9, 5.11): the choice of the
//! machine, `need`, `need_blob`, `query` and `own_heads`.

use crate::{
    BlobId, Core, DeviceList, HeadInfo, Hit, Ingested, Net, NetError, Query, Role, blobs, client,
    key::NodeKey, lock, sync,
};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use watane_log::PublicKey;

/// Channels one `Net` pulls for its queries in one window (node Q6).
pub(crate) const QUERY_PULLS: usize = 32;
/// The window of `QUERY_PULLS`.
pub(crate) const QUERY_WINDOW: Duration = Duration::from_secs(5);

/// The channels pulled for queries in the window that began at `start`.
#[derive(Default)]
pub(crate) struct Window {
    start: Option<Instant>,
    count: usize,
}

impl Window {
    /// How many of `want` channels may be pulled at `now`; they are counted.
    pub(crate) fn take(&mut self, now: Instant, want: usize) -> usize {
        if self
            .start
            .is_none_or(|s| now.saturating_duration_since(s) >= QUERY_WINDOW)
        {
            self.start = Some(now);
            self.count = 0;
        }
        let granted = want.min(QUERY_PULLS - self.count);
        self.count += granted;
        granted
    }
}

/// The full machines of the list other than `me`, in the order to ask them:
/// full replicas with a kept connection, the other full replicas, then the
/// anchor of the list.
pub(crate) fn order(list: &DeviceList, me: &NodeKey, connected: &HashSet<NodeKey>) -> Vec<NodeKey> {
    let full = list
        .devices
        .iter()
        .filter(|(k, role)| *role == Role::Full && k != me)
        .map(|(k, _)| *k);
    let (anchor, replicas): (Vec<_>, Vec<_>) = full.partition(|k| *k == list.anchor);
    let (kept, dial): (Vec<_>, Vec<_>) = replicas.into_iter().partition(|k| connected.contains(k));
    kept.into_iter().chain(dial).chain(anchor).collect()
}

impl Core {
    /// The machines to ask, in order; none that is dropped for the session
    /// or that cannot be reached for lack of a connection and an address.
    fn full_machines(&self) -> Vec<NodeKey> {
        let Some(list) = self.devices() else {
            return Vec::new();
        };
        let connected: HashSet<NodeKey> = list
            .devices
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| self.link_of(k).is_some())
            .collect();
        let mut nodes = order(&list, &self.key, &connected);
        nodes.retain(|k| {
            !self.is_dropped(k) && (connected.contains(k) || !self.addrs_of(k).is_empty())
        });
        nodes
    }

    fn is_on(&self) -> bool {
        lock(&self.endpoint).is_some()
    }
}

/// Pull `channel` from `node` as an asked channel (5.9): `heads`, then
/// `entries` from the holder's seq + 1. `Some`: the holder did not take a
/// page, and why.
async fn need_from(
    core: &std::sync::Arc<Core>,
    node: &NodeKey,
    channel: &PublicKey,
) -> Result<Option<Ingested>, NetError> {
    let remote = client::heads_async(core, node, &[*channel]).await?;
    let Some((_, Some(head))) = remote.into_iter().next() else {
        return Ok(None);
    };
    let key = *channel;
    let own: Option<HeadInfo> = sync::holder(core, move |h| h.heads(Some(&[key])))
        .await?
        .into_iter()
        .next()
        .and_then(|(_, h)| h);
    sync::pull(core, node, Role::Light, channel, head, own).await
}

/// A bad object drops the machine (5.6); anything else is not an answer.
fn judge(core: &Core, node: &NodeKey, e: NetError) -> Option<NetError> {
    match e {
        NetError::BadObject { cause, .. } => {
            core.refuse(node, cause.clone());
            Some(NetError::BadObject { node: *node, cause })
        }
        _ => None,
    }
}

pub(crate) fn need(net: &Net, channel: &PublicKey) -> Result<(), NetError> {
    let core = net.core_arc();
    if !core.is_on() {
        return Err(NetError::Off);
    }
    let channel = *channel;
    net.runtime()?.block_on(async move {
        // the last machine that refused this machine
        let mut refused = None;
        for node in core.full_machines() {
            match need_from(&core, &node, &channel).await {
                Ok(None) => return Ok(()),
                // the holder did not take what the machine gave: asking
                // another machine would not change that. Not a bad object:
                // a compromised channel is the author's fork (dist: "now or
                // earlier"), not the machine's lie, so it is not dropped.
                Ok(Some(why)) => {
                    return Err(NetError::Unreachable {
                        node,
                        cause: format!(
                            "the holder did not take channel {channel}: {}",
                            match why {
                                Ingested::Failed(cause) => cause,
                                Ingested::Compromised => "compromised".to_owned(),
                                _ => "not wanted".to_owned(),
                            }
                        ),
                    });
                }
                Err(e) => {
                    if let NetError::Refused { .. } = e {
                        refused = Some(e);
                    } else if let Some(e) = judge(&core, &node, e) {
                        return Err(e);
                    }
                }
            }
        }
        Err(refused.unwrap_or(NetError::NoFullMachine))
    })
}

pub(crate) fn need_blob(net: &Net, id: &BlobId, size: u64) -> Result<(), NetError> {
    let core = net.core_arc();
    if !core.is_on() {
        return Err(NetError::Off);
    }
    let machines = core.full_machines();
    if size > core.limits().blob {
        return Err(NetError::TooLarge {
            node: machines.first().copied().unwrap_or(core.key),
        });
    }
    let id = *id;
    net.runtime()?.block_on(async move {
        // the last machine that answered without the package, and why
        let mut last = None;
        // the last machine that refused this machine
        let mut refused = None;
        for node in machines {
            match blobs::fetch_via(&core, &node, id, size).await {
                Ok(()) => return Ok(()),
                Err(blobs::Failed::NoConnection(_)) => {}
                Err(blobs::Failed::NotFetched(e)) => {
                    if let NetError::BadObject { cause, .. } = &e {
                        core.refuse(&node, cause.clone());
                        return Err(e);
                    }
                    if let NetError::Refused { .. } = e {
                        refused = Some(e);
                    } else {
                        last = Some((node, e));
                    }
                }
            }
        }
        if let Some(e) = refused {
            return Err(e);
        }
        match last {
            Some((node, e)) => Err(NetError::Unreachable {
                node,
                cause: format!(
                    "package {} was not fetched: {}",
                    blobs::hex8(&id),
                    e.cause_text()
                ),
            }),
            None => Err(NetError::NoFullMachine),
        }
    })
}

pub(crate) fn query(net: &Net, query: &Query) -> Vec<(NodeKey, Vec<Hit>)> {
    let core = net.core_arc();
    let mut query = query.clone();
    query.limit = query.limit.min(crate::wire::MAX_HITS);
    let mut out = vec![(core.key, core.holdings.search(&query))];
    if !core.is_on() {
        return out;
    }
    let Ok(runtime) = net.runtime() else {
        return out;
    };
    let remote = runtime.block_on(async {
        for node in core.full_machines() {
            match client::query_async(&core, &node, &query).await {
                Ok(hits) => {
                    pull_hits(&core, &node, &hits).await;
                    return Some((node, hits));
                }
                Err(e) => {
                    judge(&core, &node, e);
                }
            }
        }
        None
    });
    out.extend(remote);
    out
}

/// Pull the channels of `hits` that the holder lacks or holds lower, at most
/// the window allows. A failed pull changes nothing: the holder verifies.
async fn pull_hits(core: &std::sync::Arc<Core>, node: &NodeKey, hits: &[Hit]) {
    let mut wanted: Vec<(PublicKey, u64)> = Vec::new();
    for hit in hits {
        match wanted.iter_mut().find(|(c, _)| *c == hit.channel) {
            Some((_, seq)) => *seq = (*seq).max(hit.seq),
            None => wanted.push((hit.channel, hit.seq)),
        }
    }
    let channels: Vec<PublicKey> = wanted.iter().map(|(c, _)| *c).collect();
    let Ok(own) = sync::holder(core, move |h| h.heads(Some(&channels))).await else {
        return;
    };
    let lacking: Vec<PublicKey> = wanted
        .iter()
        .filter(|(c, seq)| {
            let head = own.iter().find(|(k, _)| k == c).and_then(|(_, h)| *h);
            head.is_none_or(|h| h.seq < *seq)
        })
        .map(|(c, _)| *c)
        .collect();
    let granted = lock(&core.pulls).take(Instant::now(), lacking.len());
    for channel in lacking.into_iter().take(granted) {
        if let Err(e) = need_from(core, node, &channel).await
            && judge(core, node, e).is_some()
        {
            return;
        }
    }
}

pub(crate) fn own_heads(net: &Net, channel: &PublicKey) -> Vec<(NodeKey, Option<HeadInfo>)> {
    let core = net.core_arc();
    if !core.is_on() {
        return Vec::new();
    }
    let Ok(runtime) = net.runtime() else {
        return Vec::new();
    };
    let channel = *channel;
    runtime.block_on(async move {
        let mut tasks = tokio::task::JoinSet::new();
        for (at, node) in core.full_machines().into_iter().enumerate() {
            let core = core.clone();
            tasks.spawn(async move {
                let answer = client::heads_async(&core, &node, &[channel]).await;
                (at, node, answer)
            });
        }
        let mut found = Vec::new();
        while let Some(done) = tasks.join_next().await {
            let Ok((at, node, answer)) = done else {
                continue;
            };
            match answer {
                Ok(heads) => found.push((at, node, heads.into_iter().next().and_then(|(_, h)| h))),
                Err(e) => {
                    judge(&core, &node, e);
                }
            }
        }
        found.sort_by_key(|(at, _, _)| *at);
        found.into_iter().map(|(_, n, h)| (n, h)).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> NodeKey {
        NodeKey::from_bytes([n; 32])
    }

    #[test]
    fn ac3_the_order_is_kept_replicas_then_dialed_replicas_then_the_anchor() {
        let (anchor, me, r1, r2, r3, light) = (key(1), key(2), key(3), key(4), key(5), key(6));
        let list = DeviceList {
            anchor,
            seq: 1,
            time: 1,
            expires_days: 0,
            devices: vec![
                (anchor, Role::Full),
                (me, Role::Full),
                (r1, Role::Full),
                (r2, Role::Full),
                (r3, Role::Full),
                (light, Role::Light),
            ],
        };
        let connected: HashSet<NodeKey> = [anchor, r3].into_iter().collect();
        assert_eq!(order(&list, &me, &connected), vec![r3, r1, r2, anchor]);
        // none connected: replicas in list order, the anchor last
        assert_eq!(order(&list, &me, &HashSet::new()), vec![r1, r2, r3, anchor]);
        // this node is the anchor: it is not its own candidate
        let own = order(&list, &anchor, &HashSet::new());
        assert_eq!(own, vec![me, r1, r2, r3]);
        // a light machine is never a candidate
        assert!(!order(&list, &me, &connected).contains(&light));
    }

    #[test]
    fn ac7_the_query_window_holds_32_channels_in_5_seconds() {
        let t0 = Instant::now();
        let mut w = Window::default();
        assert_eq!(w.take(t0, 20), 20);
        // three calls for one query cannot pull 96
        assert_eq!(w.take(t0 + Duration::from_secs(1), 20), 12);
        assert_eq!(w.take(t0 + Duration::from_secs(4), 5), 0);
        // the window began at the first pull and ends 5 s after it
        assert_eq!(w.take(t0 + Duration::from_secs(5), 40), 32);
        assert_eq!(w.take(t0 + Duration::from_secs(6), 1), 0);
    }
}
