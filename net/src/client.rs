//! The caller's side: send one request, read the answer within its byte
//! limit and deadline, and check the answer before it is returned
//! (NET-F-23, NET-F-24).

use crate::{
    Core, DeviceList, HeadInfo, Hit, Net, NetError, Query,
    error::Code,
    key::NodeKey,
    lock,
    ticket::{Hint, PairCode},
    wire::{self, AnswerError, Hello, Request},
};
use iroh::{EndpointAddr, TransportAddr, endpoint::Connection};
use std::{collections::HashSet, time::Duration};
use watane_log::{Entry, MAX_SEQ, PublicKey};

fn bad_object(node: &NodeKey, cause: impl Into<String>) -> NetError {
    NetError::BadObject {
        node: *node,
        cause: cause.into(),
    }
}

fn answer_error(node: &NodeKey, e: AnswerError) -> NetError {
    match e {
        AnswerError::Refused(code) => NetError::Refused { node: *node, code },
        AnswerError::Bad(cause) => bad_object(node, cause),
    }
}

/// One request on one stream of `connection`. Returns the answer's bytes.
pub(crate) async fn request_on(
    connection: &Connection,
    node: &NodeKey,
    bytes: &[u8],
    cap: usize,
) -> Result<Vec<u8>, NetError> {
    let unreachable = |cause: String| NetError::Unreachable { node: *node, cause };
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|e| unreachable(e.to_string()))?;
    send.write_all(bytes)
        .await
        .map_err(|e| unreachable(e.to_string()))?;
    send.finish().map_err(|e| unreachable(e.to_string()))?;
    match recv.read_to_end(cap).await {
        Ok(answer) => Ok(answer),
        Err(iroh::endpoint::ReadToEndError::TooLong) => Err(NetError::TooLarge { node: *node }),
        Err(e) => Err(unreachable(e.to_string())),
    }
}

/// The IP address `connection` runs over: the selected path when it is one,
/// else any. A path through a relay has none.
pub(crate) fn ip_of(connection: &Connection) -> Option<std::net::SocketAddr> {
    let paths = connection.paths();
    let mut any = None;
    for path in paths.iter() {
        if let TransportAddr::Ip(addr) = path.remote_addr() {
            if path.is_selected() {
                return Some(*addr);
            }
            any.get_or_insert(*addr);
        }
    }
    any
}

/// A new connection to `node` on `alpn`, at the addresses `net` knows and
/// through the relays the participant chose. A trusted machine's address that
/// worked is kept (4.5).
pub(crate) async fn connect(
    core: &Core,
    node: &NodeKey,
    alpn: &[u8],
) -> Result<Connection, NetError> {
    connect_within(core, node, alpn, core.limits().connect_timeout).await
}

pub(crate) async fn connect_within(
    core: &Core,
    node: &NodeKey,
    alpn: &[u8],
    within: Duration,
) -> Result<Connection, NetError> {
    let endpoint = lock(&core.endpoint).clone().ok_or(NetError::Off)?;
    let unreachable = |cause: String| NetError::Unreachable { node: *node, cause };
    let id = iroh::EndpointId::from_bytes(node.as_bytes())
        .map_err(|e| unreachable(format!("not a node key: {e}")))?;
    let addr = EndpointAddr::from_parts(
        id,
        core.addrs_of(node)
            .into_iter()
            .map(TransportAddr::Ip)
            .chain(core.relay_urls().into_iter().map(TransportAddr::Relay)),
    );
    let connection = tokio::time::timeout(within, endpoint.connect(addr, alpn))
        .await
        .map_err(|_| unreachable(core.no_path_cause()))?
        .map_err(|_| unreachable(core.no_path_cause()))?;
    if let Some(addr) = ip_of(&connection) {
        core.learn_addr(node, addr);
    }
    Ok(connection)
}

/// One request on one stream: of the kept connection to `node` when there
/// is one, else of a new connection that is closed after. Any answer from
/// the anchor renews the expiry (ND-17).
pub(crate) async fn call_async(
    core: &Core,
    node: &NodeKey,
    request: &Request,
    cap: usize,
    deadline: Duration,
) -> Result<Vec<u8>, NetError> {
    let limits = core.limits();
    let bytes = request.encode();
    if bytes.len() > limits.request {
        return Err(NetError::TooLarge { node: *node });
    }
    if lock(&core.endpoint).is_none() {
        return Err(NetError::Off);
    }
    // set while a new connection is being made: a deadline that ends it is a
    // machine that cannot be reached
    let connecting = std::sync::atomic::AtomicBool::new(false);
    let work = async {
        if let Some(link) = core.link_of(node) {
            match request_on(&link, node, &bytes, cap).await {
                Err(NetError::Unreachable { .. }) => {}
                other => return other,
            }
        }
        connecting.store(true, std::sync::atomic::Ordering::Relaxed);
        let connection =
            connect_within(core, node, wire::ALPN, limits.connect_timeout.min(deadline)).await?;
        connecting.store(false, std::sync::atomic::Ordering::Relaxed);
        let result = request_on(&connection, node, &bytes, cap).await;
        connection.close(0u32.into(), b"done");
        result
    };
    let answer = tokio::time::timeout(deadline, work).await.map_err(|_| {
        if connecting.load(std::sync::atomic::Ordering::Relaxed) {
            NetError::Unreachable {
                node: *node,
                cause: core.no_path_cause(),
            }
        } else {
            NetError::Timeout { node: *node }
        }
    })??;
    core.seen(node);
    Ok(answer)
}

fn call_for(
    net: &Net,
    node: &NodeKey,
    request: &Request,
    cap: usize,
    deadline: Duration,
) -> Result<Vec<u8>, NetError> {
    let runtime = net.runtime()?;
    runtime.block_on(call_async(net.core(), node, request, cap, deadline))
}

/// Join the anchor of `code` (5.5).
pub(crate) fn join(net: &Net, code: &PairCode, replace: bool) -> Result<DeviceList, NetError> {
    let core = net.core();
    if !replace && core.devices().is_some() {
        return Err(NetError::HasAnchor);
    }
    let hints: Vec<_> = code
        .hints
        .iter()
        .filter_map(|h| match h {
            Hint::Addr(a) => Some(*a),
            Hint::Relay(_) => None,
        })
        .collect();
    net.add_addrs(&code.anchor, &hints);
    *lock(&core.joining) = Some((code.anchor, None));
    let result = join_with(net, code, replace);
    let reached = lock(&core.joining).take().and_then(|(_, at)| at);
    let list = result?;
    // the anchor is listed now: keep where it answered (4.5)
    if let Some(addr) = reached {
        core.learn_addr(&code.anchor, addr);
    }
    // Pair carries the list only. Pull addresses even when its seq is current;
    // a failed pull cannot undo the join that was already installed.
    net.runtime()?
        .block_on(crate::pairing::pull(net.core_arc(), code.anchor));
    Ok(list)
}

fn join_with(net: &Net, code: &PairCode, replace: bool) -> Result<DeviceList, NetError> {
    let core = net.core();
    let answer = call_for(
        net,
        &code.anchor,
        &Request::Pair {
            secret: code.secret,
        },
        wire::LIST_ANSWER,
        core.limits().pair_deadline,
    )?;
    let bytes = wire::parse_pair(&answer).map_err(|e| match e {
        AnswerError::Refused(Code::BadCode) => NetError::BadCode,
        AnswerError::Refused(Code::Declined) => NetError::Declined,
        other => answer_error(&code.anchor, other),
    })?;
    let list = DeviceList::verify(bytes)?;
    if list.anchor != code.anchor {
        return Err(NetError::BadList {
            cause: "the list is not signed by the key of the code".into(),
        });
    }
    if !list.devices.iter().any(|(k, _)| *k == core.key) {
        return Err(NetError::BadList {
            cause: "the list does not name this machine".into(),
        });
    }
    core.install(bytes, list.clone(), replace)?;
    Ok(list)
}

pub(crate) fn hello(net: &Net, node: &NodeKey) -> Result<Hello, NetError> {
    net.runtime()?
        .block_on(hello_async(&net.core.clone(), node, None))
}

/// `hello` on `connection` (a new kept connection) or, with none, as any
/// call. A trusted node with a higher list: ask for it (5.2); a failed pull
/// changes nothing.
pub(crate) async fn hello_async(
    core: &std::sync::Arc<Core>,
    node: &NodeKey,
    connection: Option<&Connection>,
) -> Result<Hello, NetError> {
    let request = Request::Hello {
        version: wire::VERSION,
    };
    let answer = match connection {
        Some(connection) => {
            let limits = core.limits();
            let bytes = request.encode();
            let work = request_on(connection, node, &bytes, wire::HELLO_ANSWER);
            let answer = tokio::time::timeout(limits.deadline, work)
                .await
                .map_err(|_| NetError::Timeout { node: *node })??;
            core.seen(node);
            answer
        }
        None => {
            call_async(
                core,
                node,
                &request,
                wire::HELLO_ANSWER,
                core.limits().deadline,
            )
            .await?
        }
    };
    let hello = wire::parse_hello(&answer).map_err(|e| answer_error(node, e))?;
    let own = core.devices().map(|l| l.seq);
    if let (Some(own), Some(theirs)) = (own, hello.devices_seq)
        && theirs > own
        && core.class_of(node) == crate::Class::Trusted
    {
        crate::pairing::pull(core.clone(), *node).await;
    }
    Ok(hello)
}

pub(crate) fn heads(
    net: &Net,
    node: &NodeKey,
    asked: &[PublicKey],
) -> Result<Vec<(PublicKey, Option<HeadInfo>)>, NetError> {
    net.runtime()?
        .block_on(heads_async(net.core(), node, asked))
}

pub(crate) async fn heads_async(
    core: &Core,
    node: &NodeKey,
    asked: &[PublicKey],
) -> Result<Vec<(PublicKey, Option<HeadInfo>)>, NetError> {
    let answer = call_async(
        core,
        node,
        &Request::Heads {
            channels: asked.to_vec(),
        },
        wire::HEADS_ANSWER,
        core.limits().deadline,
    )
    .await?;
    let items = wire::parse_heads(&answer).map_err(|e| answer_error(node, e))?;
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for (channel, head) in items {
        if !asked.is_empty() && !asked.contains(&channel) {
            return Err(bad_object(
                node,
                format!("a head for {channel}, which was not asked"),
            ));
        }
        if !seen.insert(channel) {
            return Err(bad_object(node, format!("{channel} is answered twice")));
        }
        let head = match head {
            Some((seq, id, forked)) => {
                if seq > MAX_SEQ {
                    return Err(bad_object(node, "a head's seq is out of range"));
                }
                Some(HeadInfo {
                    seq,
                    id: wire::entry_id(id),
                    forked,
                })
            }
            None => None,
        };
        out.push((channel, head));
    }
    if asked.iter().any(|c| !seen.contains(c)) {
        return Err(bad_object(node, "a channel that was asked is not answered"));
    }
    Ok(out)
}

pub(crate) fn entries(
    net: &Net,
    node: &NodeKey,
    channel: &PublicKey,
    from: u64,
    limit: u64,
) -> Result<Vec<Entry>, NetError> {
    net.runtime()?
        .block_on(entries_async(net.core(), node, channel, from, limit))
}

pub(crate) async fn entries_async(
    core: &Core,
    node: &NodeKey,
    channel: &PublicKey,
    from: u64,
    limit: u64,
) -> Result<Vec<Entry>, NetError> {
    let limits = core.limits();
    let answer = call_async(
        core,
        node,
        &Request::Entries {
            channel: *channel,
            from,
            limit,
        },
        limits.answer_entries,
        limits.deadline,
    )
    .await?;
    let raw = wire::parse_entries(&answer).map_err(|e| answer_error(node, e))?;
    let most = limit.min(limits.max_entries as u64);
    if raw.len() as u64 > most {
        return Err(bad_object(
            node,
            format!("{} entries, and at most {most} were asked for", raw.len()),
        ));
    }
    let mut out: Vec<Entry> = Vec::with_capacity(raw.len());
    for bytes in raw {
        if bytes.len() > limits.entry {
            return Err(bad_object(node, "an entry is over the entry size"));
        }
        let entry = Entry::decode(bytes).map_err(|e| bad_object(node, format!("an entry: {e}")))?;
        if entry.author() != *channel {
            return Err(bad_object(
                node,
                "an entry of another author than the channel",
            ));
        }
        match out.last() {
            None if entry.seq() != from => {
                return Err(bad_object(
                    node,
                    format!("the first entry is seq {}, not {from}", entry.seq()),
                ));
            }
            Some(last) if entry.seq() != last.seq() + 1 || entry.prev() != Some(last.id()) => {
                return Err(bad_object(
                    node,
                    "entries are out of seq order or do not chain",
                ));
            }
            _ => {}
        }
        out.push(entry);
    }
    Ok(out)
}

pub(crate) fn fork(
    net: &Net,
    node: &NodeKey,
    channel: &PublicKey,
) -> Result<Option<(Entry, Entry)>, NetError> {
    net.runtime()?
        .block_on(fork_async(net.core(), node, channel))
}

pub(crate) async fn fork_async(
    core: &Core,
    node: &NodeKey,
    channel: &PublicKey,
) -> Result<Option<(Entry, Entry)>, NetError> {
    let answer = call_async(
        core,
        node,
        &Request::Fork { channel: *channel },
        wire::FORK_ANSWER,
        core.limits().deadline,
    )
    .await?;
    let Some((a, b)) = wire::parse_fork(&answer).map_err(|e| answer_error(node, e))? else {
        return Ok(None);
    };
    let decode = |bytes: &[u8]| {
        Entry::decode(bytes).map_err(|e| bad_object(node, format!("a fork entry: {e}")))
    };
    let (a, b) = (decode(a)?, decode(b)?);
    if a.author() != *channel || b.author() != *channel {
        return Err(bad_object(
            node,
            "a fork entry of another author than the channel",
        ));
    }
    if a.seq() != b.seq() {
        return Err(bad_object(node, "the fork entries are at different seqs"));
    }
    if a.id() == b.id() {
        return Err(bad_object(node, "the fork entries are the same entry"));
    }
    Ok(Some((a, b)))
}

/// `query` of one machine; the answer is checked before it is returned.
pub(crate) async fn query_async(
    core: &Core,
    node: &NodeKey,
    query: &Query,
) -> Result<Vec<Hit>, NetError> {
    let answer = call_async(
        core,
        node,
        &Request::Query(query.clone()),
        wire::QUERY_ANSWER,
        core.limits().deadline,
    )
    .await?;
    let hits = wire::parse_query(&answer).map_err(|e| answer_error(node, e))?;
    if hits.len() as u64 > query.limit {
        return Err(bad_object(
            node,
            format!(
                "{} hits, and at most {} were asked for",
                hits.len(),
                query.limit
            ),
        ));
    }
    Ok(hits)
}
