//! The handler of `watane/1` (TECHNICAL 5.4): what a callee does with one
//! stream. Order: byte limit before decoding, decode, allowance of the op,
//! then the op, answered from the holder.

use crate::{
    Class, Core, HeadInfo,
    error::Code,
    key::NodeKey,
    wire::{self, Request},
};
use iroh::{
    endpoint::{Connection, RecvStream, SendStream},
    protocol::{AcceptError, ProtocolHandler},
};
use std::sync::Arc;
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
        while let Ok((send, recv)) = connection.accept_bi().await {
            let core = self.core.clone();
            tokio::spawn(async move { stream(core, caller, send, recv).await });
        }
        Ok(())
    }
}

async fn stream(core: Arc<Core>, caller: NodeKey, mut send: SendStream, recv: RecvStream) {
    let limits = core.limits();
    let answer = match tokio::time::timeout(limits.deadline, answer(&core, caller, recv)).await {
        Ok(Some(answer)) => answer,
        Ok(None) => return,
        Err(_) => wire::err_answer(Code::Busy, "the request took longer than its deadline"),
    };
    if send.write_all(&answer).await.is_ok() && send.finish().is_ok() {
        // keep the connection until the caller has taken the answer
        let _ = send.stopped().await;
    }
}

async fn answer(core: &Arc<Core>, caller: NodeKey, mut recv: RecvStream) -> Option<Vec<u8>> {
    let limits = core.limits();
    // the byte limit comes first, before any decoding
    let bytes = match recv.read_to_end(limits.request).await {
        Ok(bytes) => bytes,
        Err(iroh::endpoint::ReadToEndError::TooLong) => {
            return Some(wire::err_answer(
                Code::TooLarge,
                &format!("a request is at most {} bytes", limits.request),
            ));
        }
        Err(_) => return None,
    };
    let request = match Request::decode(&bytes) {
        Ok(request) => request,
        Err(code) => return Some(wire::err_answer(code, "the request does not decode")),
    };
    if !matches!(request, Request::Hello { .. }) && core.class_of(&caller) != Class::Trusted {
        return Some(wire::err_answer(
            Code::NotAllowed,
            &format!("node {} may not call {}", caller.short(), request.op()),
        ));
    }
    let core = core.clone();
    let work = tokio::task::spawn_blocking(move || run(&core, request));
    Some(
        work.await
            .unwrap_or_else(|_| wire::err_answer(Code::Busy, "the holder failed")),
    )
}

fn run(core: &Core, request: Request) -> Vec<u8> {
    let limits = core.limits();
    match request {
        Request::Hello { .. } => wire::hello_answer(&limits),
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
    }
}
