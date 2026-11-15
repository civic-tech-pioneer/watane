//! The handler of `watane/1` (TECHNICAL 5.4): what a callee does with one
//! stream. Order: byte limit before decoding, decode, allowance of the op,
//! then the op, answered from the holder.

use crate::{
    Class, Core, HeadInfo,
    error::Code,
    key::NodeKey,
    lock, pairing,
    wire::{self, Request},
};
use iroh::{
    endpoint::{Connection, RecvStream, SendStream},
    protocol::{AcceptError, ProtocolHandler},
};
use std::{sync::Arc, time::Duration};
use watane_log::{Entry, PublicKey};

pub(crate) struct Serve {
    pub(crate) core: Arc<Core>,
}

impl std::fmt::Debug for Serve {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Serve")
    }
}

impl ProtocolHandler for Serve {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        // The caller's key is the transport's (NET-F-4), never a field.
        let caller = NodeKey::from_bytes(*connection.remote_id().as_bytes());
        // a trusted machine that called us: keep the connection (5.2)
        if connection.side().is_server() && self.core.class_of(&caller) == Class::Trusted {
            tokio::spawn(crate::sync::adopt(
                self.core.clone(),
                caller,
                connection.clone(),
                caller,
            ));
        }
        while let Ok((send, recv)) = connection.accept_bi().await {
            let core = self.core.clone();
            let class = core.class_of(&caller);
            // an unknown caller's streams and bytes are budgeted (4.2)
            let budget = if class == Class::Trusted {
                Some(None)
            } else {
                let limits = core.limits();
                core.pool.poll(&connection, &limits);
                core.pool.stream(caller, &limits).map(Some)
            };
            tokio::spawn(async move {
                match budget {
                    Some(guard) => {
                        stream(core, caller, class, send, recv).await;
                        drop(guard);
                    }
                    None => {
                        let answer = wire::err_answer(
                            Code::OverBudget,
                            &format!("node {} is over its budget", caller.short()),
                        );
                        reply(core.limits().deadline, send, recv, answer).await;
                    }
                }
            });
        }
        Ok(())
    }
}

/// Write the answer and keep the connection until the caller has taken it.
async fn reply(deadline: Duration, mut send: SendStream, _recv: RecvStream, answer: Vec<u8>) {
    if send.write_all(&answer).await.is_ok() && send.finish().is_ok() {
        let _ = tokio::time::timeout(deadline, send.stopped()).await;
    }
}

async fn stream(
    core: Arc<Core>,
    caller: NodeKey,
    class: Class,
    send: SendStream,
    mut recv: RecvStream,
) {
    let limits = core.limits();
    let read = tokio::time::timeout(
        limits.deadline,
        read_request(&core, caller, class, &mut recv),
    );
    let request = match read.await {
        Ok(Ok(request)) => request,
        Ok(Err(Some(answer))) => return reply(limits.deadline, send, recv, answer).await,
        Ok(Err(None)) => return,
        Err(_) => {
            let answer = wire::err_answer(Code::Busy, "the request took longer than its deadline");
            return reply(limits.deadline, send, recv, answer).await;
        }
    };
    // `pair` waits for the participant: its own deadline, and a second for
    // its own timeout to answer first
    let deadline = if matches!(request, Request::Pair { .. }) {
        limits.pair_deadline + Duration::from_secs(1)
    } else {
        limits.deadline
    };
    let answer = match tokio::time::timeout(deadline, respond(&core, caller, class, request)).await
    {
        Ok(answer) => answer,
        Err(_) => wire::err_answer(Code::Busy, "the request took longer than its deadline"),
    };
    reply(limits.deadline, send, recv, answer).await;
}

/// The request, or the answer that refuses it (`None`: nothing to answer).
async fn read_request(
    core: &Arc<Core>,
    caller: NodeKey,
    class: Class,
    recv: &mut RecvStream,
) -> Result<Request, Option<Vec<u8>>> {
    let limits = core.limits();
    // the byte limit comes first, before any decoding
    let bytes = match recv.read_to_end(limits.request).await {
        Ok(bytes) => bytes,
        Err(iroh::endpoint::ReadToEndError::TooLong) => {
            return Err(Some(wire::err_answer(
                Code::TooLarge,
                &format!("a request is at most {} bytes", limits.request),
            )));
        }
        Err(_) => return Err(None),
    };
    let request = Request::decode(&bytes)
        .map_err(|code| Some(wire::err_answer(code, "the request does not decode")))?;
    // an unknown caller has `hello` and `pair` only (4.2)
    let open = matches!(request, Request::Hello { .. } | Request::Pair { .. });
    if !open && class != Class::Trusted {
        return Err(Some(wire::err_answer(
            Code::NotAllowed,
            &format!("node {} may not call {}", caller.short(), request.op()),
        )));
    }
    Ok(request)
}

async fn respond(core: &Arc<Core>, caller: NodeKey, class: Class, request: Request) -> Vec<u8> {
    match request {
        Request::Pair { secret } => return pairing::handle_pair(core, caller, secret).await,
        Request::Devices { seq } => {
            // a higher seq than ours: answer, then pull the list ourselves
            let (own, bytes, others) = {
                let state = lock(&core.state);
                // a list older than the caller's may name machines the
                // caller removed: then pass on none, or an honest machine
                // would be dropped for it (5.10)
                let current = state.list.as_ref().filter(|l| l.seq >= seq);
                let others: Vec<_> = current.map_or_else(Vec::new, |l| {
                    l.devices
                        .iter()
                        .map(|(k, _)| *k)
                        .filter(|k| *k != caller && *k != core.key)
                        .collect()
                });
                (
                    state.list.as_ref().map(|l| l.seq),
                    state.bytes.clone(),
                    others,
                )
            };
            // where this machine knows the others of the list (5.10)
            let addrs: Vec<_> = others.into_iter().map(|k| (k, core.addrs_of(&k))).collect();
            let newer = own.filter(|own| *own > seq).map(|_| bytes.as_slice());
            let answer = wire::devices_answer(newer, &addrs);
            if own.is_some_and(|own| seq > own) {
                tokio::spawn(pairing::pull(core.clone(), caller));
            }
            return answer;
        }
        _ => {}
    }
    let core = core.clone();
    let work = tokio::task::spawn_blocking(move || run(&core, class, request));
    work.await
        .unwrap_or_else(|_| wire::err_answer(Code::Busy, "the holder failed"))
}

fn run(core: &Core, class: Class, request: Request) -> Vec<u8> {
    let limits = core.limits();
    match request {
        Request::Hello { .. } => {
            let seq = (class == Class::Trusted)
                .then(|| core.devices().map(|l| l.seq))
                .flatten();
            wire::hello_answer(&limits, seq)
        }
        Request::Heads { channels } => {
            let heads: Vec<(PublicKey, Option<HeadInfo>)> = if channels.is_empty() {
                core.holdings.heads(None)
            } else {
                core.holdings.heads(Some(&channels))
            };
            let answer = wire::heads_answer(&heads);
            if answer.len() > wire::HEADS_ANSWER {
                return wire::err_answer(Code::TooLarge, "the answer is over its byte limit");
            }
            answer
        }
        Request::Entries {
            channel,
            from,
            limit,
        } => {
            let count = limit.min(limits.max_entries as u64) as usize;
            // what the holder may return: the stream holds the `ok` item
            // and a head of at most 5 bytes for each entry
            let max_bytes = limits
                .answer_entries
                .saturating_sub(wire::OK_ITEM_LEN + 5 * count);
            let entries: Vec<Entry> = if count == 0 {
                Vec::new()
            } else {
                core.holdings.entries(&channel, from, count, max_bytes)
            };
            // never trust the holder to honour the limits
            let mut total = wire::OK_ITEM_LEN;
            let mut kept = Vec::new();
            for entry in entries.iter().take(count) {
                total += wire::entry_item_len(entry.bytes().len());
                if total > limits.answer_entries {
                    break;
                }
                kept.push(entry.bytes());
            }
            wire::entries_answer(&kept)
        }
        Request::Fork { channel } => {
            let pair = core.holdings.fork(&channel);
            let answer = wire::fork_answer(pair.as_ref().map(|(a, b)| (a, b)));
            if answer.len() > wire::FORK_ANSWER {
                return wire::err_answer(Code::TooLarge, "the answer is over its byte limit");
            }
            answer
        }
        Request::Query(query) => {
            // from this holder only, never forwarded (ND-6)
            let most = query.limit.min(limits.hits as u64) as usize;
            let mut hits = core.holdings.search(&query);
            hits.truncate(most);
            wire::query_answer(&hits)
        }
        Request::Devices { .. } | Request::Pair { .. } => unreachable!("answered in respond"),
    }
}
