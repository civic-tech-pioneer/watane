//! The caller's side: send one request, read the answer within its byte
//! limit and deadline, and check the answer before it is returned
//! (NET-F-23, NET-F-24).

use crate::{
    HeadInfo, Net, NetError,
    key::NodeKey,
    wire::{self, AnswerError, Hello, Request},
};
use iroh::{EndpointAddr, TransportAddr};
use std::collections::HashSet;
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

/// One request on one stream of one new connection. Returns the answer's bytes.
fn call(net: &Net, node: &NodeKey, request: &Request, cap: usize) -> Result<Vec<u8>, NetError> {
    let limits = net.core().limits();
    let bytes = request.encode();
    if bytes.len() > limits.request {
        return Err(NetError::TooLarge { node: *node });
    }
    let endpoint = net.endpoint().ok_or(NetError::Off)?;
    let unreachable = |cause: String| NetError::Unreachable { node: *node, cause };
    let id = iroh::EndpointId::from_bytes(node.as_bytes())
        .map_err(|e| unreachable(format!("not a node key: {e}")))?;
    let addr = EndpointAddr::from_parts(
        id,
        net.core().addrs_of(node).into_iter().map(TransportAddr::Ip),
    );
    let work = async {
        let connection =
            tokio::time::timeout(limits.connect_timeout, endpoint.connect(addr, wire::ALPN))
                .await
                .map_err(|_| unreachable("the connection took too long".into()))?
                .map_err(|e| unreachable(e.to_string()))?;
        let result = async {
            let (mut send, mut recv) = connection
                .open_bi()
                .await
                .map_err(|e| unreachable(e.to_string()))?;
            send.write_all(&bytes)
                .await
                .map_err(|e| unreachable(e.to_string()))?;
            send.finish().map_err(|e| unreachable(e.to_string()))?;
            match recv.read_to_end(cap).await {
                Ok(answer) => Ok(answer),
                Err(iroh::endpoint::ReadToEndError::TooLong) => {
                    Err(NetError::TooLarge { node: *node })
                }
                Err(e) => Err(unreachable(e.to_string())),
            }
        }
        .await;
        connection.close(0u32.into(), b"done");
        result
    };
    let runtime = net.runtime()?;
    runtime
        .block_on(async { tokio::time::timeout(limits.deadline, work).await })
        .map_err(|_| NetError::Timeout { node: *node })?
}

pub(crate) fn hello(net: &Net, node: &NodeKey) -> Result<Hello, NetError> {
    let answer = call(
        net,
        node,
        &Request::Hello {
            version: wire::VERSION,
        },
        wire::HELLO_ANSWER,
    )?;
    wire::parse_hello(&answer).map_err(|e| answer_error(node, e))
}

pub(crate) fn heads(
    net: &Net,
    node: &NodeKey,
    asked: &[PublicKey],
) -> Result<Vec<(PublicKey, Option<HeadInfo>)>, NetError> {
    let answer = call(
        net,
        node,
        &Request::Heads {
            channels: asked.to_vec(),
        },
        wire::HEADS_ANSWER,
    )?;
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
    let limits = net.core().limits();
    let answer = call(
        net,
        node,
        &Request::Entries {
            channel: *channel,
            from,
            limit,
        },
        limits.answer_entries,
    )?;
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
    let answer = call(
        net,
        node,
        &Request::Fork { channel: *channel },
        wire::FORK_ANSWER,
    )?;
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
