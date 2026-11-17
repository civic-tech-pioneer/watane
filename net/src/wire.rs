//! Requests and answers of `watane/1` (TECHNICAL 4.2), as deterministic CBOR.

use crate::cbor::{self, Item, Reader};
use crate::error::Code;
use crate::key::NodeKey;
use crate::{HeadInfo, Hit, Query};
use std::time::Duration;
use watane_log::{Entry, EntryId, MAX_ENTRY, PublicKey};

pub(crate) const ALPN: &[u8] = b"watane/1";
pub(crate) const VERSION: u64 = 1;
/// The ops this node answers, in the order `hello` lists them.
pub(crate) const OPS: [&str; 7] = [
    "hello", "heads", "entries", "fork", "devices", "pair", "query",
];

/// Answer byte limits of each op: the whole stream, CBOR framing included.
pub(crate) const HELLO_ANSWER: usize = 1024;
pub(crate) const HEADS_ANSWER: usize = 1 << 20;
pub(crate) const FORK_ANSWER: usize = 2 * MAX_ENTRY + 64;
/// The answers of `devices` and `pair`: a device list and little else.
pub(crate) const LIST_ANSWER: usize = 1 << 20;
/// The answer of `query`: at most 200 hits of about 80 bytes.
pub(crate) const QUERY_ANSWER: usize = 32 * 1024;
/// The most hits a query may ask for (4.4).
pub(crate) const MAX_HITS: u64 = 200;
/// The longest `err` item the caller reads (inside the cap of its op).
pub(crate) const ERR_MESSAGE: usize = 200;

/// Byte limits and deadlines (TECHNICAL 4.2). The `hello` answer carries
/// them as `limits: [uint]` in the order of the fields below, durations in
/// milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Request size: 256 KiB.
    pub request: usize,
    /// The whole answer stream of `entries`: 4 MiB.
    pub answer_entries: usize,
    /// Entries in one answer: 1000.
    pub max_entries: usize,
    /// One entry: `watane_log::MAX_ENTRY`.
    pub entry: usize,
    /// One blob: 64 MiB.
    pub blob: u64,
    /// Hits in one answer: 200.
    pub hits: usize,
    pub connect_timeout: Duration,
    /// The deadline of a request: 5 s.
    pub deadline: Duration,
    /// The deadline of `pair`: 10 min.
    pub pair_deadline: Duration,
    /// The deadline of a blob: 120 s.
    pub blob_deadline: Duration,
    /// Budgets of unknown peers (4.2 "Budgets"); `hello` does not send them.
    /// Connections one unknown key may hold open: 2.
    pub unknown_conns_per_peer: usize,
    /// Streams one unknown key may have open at once: 2.
    pub unknown_streams_per_peer: usize,
    /// Connections all unknown keys together may hold open: 16.
    pub unknown_conns: usize,
    /// Bytes per second, in and out, for all unknown keys together: 65536.
    pub unknown_bytes_per_sec: u64,
    /// How often an own machine is synced again whatever happened: 10 minutes
    /// (NET-F-18). `hello` does not send it.
    pub sync_interval: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            request: 256 * 1024,
            answer_entries: 4 << 20,
            max_entries: 1000,
            entry: MAX_ENTRY,
            blob: 64 << 20,
            hits: 200,
            connect_timeout: Duration::from_secs(10),
            deadline: Duration::from_secs(5),
            pair_deadline: Duration::from_secs(600),
            blob_deadline: Duration::from_secs(120),
            unknown_conns_per_peer: 2,
            unknown_streams_per_peer: 2,
            unknown_conns: 16,
            unknown_bytes_per_sec: 65536,
            sync_interval: Duration::from_secs(600),
        }
    }
}

impl Limits {
    fn to_wire(&self) -> [u64; 10] {
        [
            self.request as u64,
            self.answer_entries as u64,
            self.max_entries as u64,
            self.entry as u64,
            self.blob,
            self.hits as u64,
            self.connect_timeout.as_millis() as u64,
            self.deadline.as_millis() as u64,
            self.pair_deadline.as_millis() as u64,
            self.blob_deadline.as_millis() as u64,
        ]
    }
    fn from_wire(v: &[u64]) -> Option<Limits> {
        let [
            request,
            answer_entries,
            max_entries,
            entry,
            blob,
            hits,
            connect,
            deadline,
            pair,
            blob_deadline,
        ]: [u64; 10] = v.try_into().ok()?;
        Some(Limits {
            request: usize::try_from(request).ok()?,
            answer_entries: usize::try_from(answer_entries).ok()?,
            max_entries: usize::try_from(max_entries).ok()?,
            entry: usize::try_from(entry).ok()?,
            blob,
            hits: usize::try_from(hits).ok()?,
            connect_timeout: Duration::from_millis(connect),
            deadline: Duration::from_millis(deadline),
            pair_deadline: Duration::from_millis(pair),
            blob_deadline: Duration::from_millis(blob_deadline),
            ..Limits::default()
        })
    }
}

/// The answer of `hello`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub version: u64,
    pub ops: Vec<String>,
    pub limits: Limits,
    pub devices_seq: Option<u64>,
}

// ---- requests ----

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    Hello {
        version: u64,
    },
    Heads {
        channels: Vec<PublicKey>,
    },
    Entries {
        channel: PublicKey,
        from: u64,
        limit: u64,
    },
    Fork {
        channel: PublicKey,
    },
    Devices {
        seq: u64,
    },
    Pair {
        secret: [u8; 16],
    },
    Query(Query),
}

impl Request {
    pub(crate) fn op(&self) -> &'static str {
        match self {
            Request::Hello { .. } => "hello",
            Request::Heads { .. } => "heads",
            Request::Entries { .. } => "entries",
            Request::Fork { .. } => "fork",
            Request::Devices { .. } => "devices",
            Request::Pair { .. } => "pair",
            Request::Query(_) => "query",
        }
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Request::Hello { version } => {
                cbor::array(2, &mut out);
                cbor::text("hello", &mut out);
                cbor::uint(*version, &mut out);
            }
            Request::Heads { channels } => {
                cbor::array(2, &mut out);
                cbor::text("heads", &mut out);
                cbor::array(channels.len(), &mut out);
                for c in channels {
                    cbor::bytes(c.as_bytes(), &mut out);
                }
            }
            Request::Entries {
                channel,
                from,
                limit,
            } => {
                cbor::array(4, &mut out);
                cbor::text("entries", &mut out);
                cbor::bytes(channel.as_bytes(), &mut out);
                cbor::uint(*from, &mut out);
                cbor::uint(*limit, &mut out);
            }
            Request::Fork { channel } => {
                cbor::array(2, &mut out);
                cbor::text("fork", &mut out);
                cbor::bytes(channel.as_bytes(), &mut out);
            }
            Request::Devices { seq } => {
                cbor::array(2, &mut out);
                cbor::text("devices", &mut out);
                cbor::uint(*seq, &mut out);
            }
            Request::Pair { secret } => {
                cbor::array(2, &mut out);
                cbor::text("pair", &mut out);
                cbor::bytes(secret, &mut out);
            }
            Request::Query(q) => {
                cbor::array(2, &mut out);
                cbor::text("query", &mut out);
                encode_query(q, &mut out);
            }
        }
        out
    }

    /// `Err(Code::BadRequest)` for bytes that are not exactly a request in
    /// the deterministic form; `Err(Code::UnknownOp)` for an op this stage
    /// does not have (the encoding of its arguments is not looked at).
    pub(crate) fn decode(bytes: &[u8]) -> Result<Request, Code> {
        let mut reader = Reader::new(bytes);
        let item = reader.item().map_err(|_| Code::BadRequest)?;
        if !reader.at_end() {
            return Err(Code::BadRequest);
        }
        let Item::Array(items) = item else {
            return Err(Code::BadRequest);
        };
        let Some(Item::Text(op)) = items.first() else {
            return Err(Code::BadRequest);
        };
        let args = &items[1..];
        let channel = |item: &Item<'_>| match item {
            Item::Bytes(b) => <&[u8; 32]>::try_from(*b)
                .ok()
                .and_then(|b| PublicKey::from_bytes(b).ok()),
            _ => None,
        };
        match (*op, args) {
            ("hello", [Item::Uint(version)]) => Ok(Request::Hello { version: *version }),
            ("heads", [Item::Array(list)]) => {
                let channels: Option<Vec<_>> = list.iter().map(channel).collect();
                Ok(Request::Heads {
                    channels: channels.ok_or(Code::BadRequest)?,
                })
            }
            ("entries", [c, Item::Uint(from), Item::Uint(limit)]) => Ok(Request::Entries {
                channel: channel(c).ok_or(Code::BadRequest)?,
                from: *from,
                limit: *limit,
            }),
            ("fork", [c]) => Ok(Request::Fork {
                channel: channel(c).ok_or(Code::BadRequest)?,
            }),
            ("devices", [Item::Uint(seq)]) => Ok(Request::Devices { seq: *seq }),
            ("pair", [Item::Bytes(secret)]) => Ok(Request::Pair {
                secret: <[u8; 16]>::try_from(*secret).map_err(|_| Code::BadRequest)?,
            }),
            ("query", [Item::Array(fields)]) => decode_query(fields)
                .map(Request::Query)
                .ok_or(Code::BadRequest),
            ("hello" | "heads" | "entries" | "fork" | "devices" | "pair" | "query", _) => {
                Err(Code::BadRequest)
            }
            _ => Err(Code::UnknownOp),
        }
    }
}

fn encode_query(q: &Query, out: &mut Vec<u8>) {
    cbor::array(5, out);
    match &q.text {
        Some(t) => cbor::text(t, out),
        None => cbor::null(out),
    }
    match &q.publisher {
        Some(p) => cbor::bytes(p.as_bytes(), out),
        None => cbor::null(out),
    }
    match &q.interface {
        Some((mode, name)) => {
            cbor::array(2, out);
            cbor::text(mode, out);
            cbor::text(name, out);
        }
        None => cbor::null(out),
    }
    match &q.origin {
        Some((publisher, slug)) => {
            cbor::array(2, out);
            cbor::bytes(publisher.as_bytes(), out);
            cbor::text(slug, out);
        }
        None => cbor::null(out),
    }
    cbor::uint(q.limit, out);
}

fn key_item(item: &Item<'_>) -> Option<PublicKey> {
    key32(item).and_then(|k| PublicKey::from_bytes(k).ok())
}

/// The query of 4.4, strictly: `limit` at most 200, a `mode` of `offers`,
/// `uses` or `sends`, text where text is named.
fn decode_query(fields: &[Item<'_>]) -> Option<Query> {
    let [text, publisher, interface, origin, Item::Uint(limit)] = fields else {
        return None;
    };
    if *limit > MAX_HITS {
        return None;
    }
    let text = match text {
        Item::Null => None,
        Item::Text(t) => Some((*t).to_owned()),
        _ => return None,
    };
    let publisher = match publisher {
        Item::Null => None,
        other => Some(key_item(other)?),
    };
    let interface = match interface {
        Item::Null => None,
        Item::Array(pair) => match pair.as_slice() {
            [
                Item::Text(mode @ ("offers" | "uses" | "sends")),
                Item::Text(name),
            ] => Some(((*mode).to_owned(), (*name).to_owned())),
            _ => return None,
        },
        _ => return None,
    };
    let origin = match origin {
        Item::Null => None,
        Item::Array(pair) => match pair.as_slice() {
            [key, Item::Text(slug)] => Some((key_item(key)?, (*slug).to_owned())),
            _ => return None,
        },
        _ => return None,
    };
    Some(Query {
        text,
        publisher,
        interface,
        origin,
        limit: *limit,
    })
}

// ---- answers: building (callee) ----

/// The first item of every good answer.
fn ok() -> Vec<u8> {
    let mut out = Vec::new();
    cbor::array(1, &mut out);
    cbor::text("ok", &mut out);
    out
}

pub(crate) fn err_answer(code: Code, message: &str) -> Vec<u8> {
    let mut message = message.to_owned();
    while message.len() > ERR_MESSAGE {
        message.pop();
    }
    let mut out = Vec::new();
    cbor::array(3, &mut out);
    cbor::text("err", &mut out);
    cbor::text(code.as_str(), &mut out);
    cbor::text(&message, &mut out);
    out
}

pub(crate) fn hello_answer(limits: &Limits, devices_seq: Option<u64>) -> Vec<u8> {
    let mut out = ok();
    cbor::array(4, &mut out);
    cbor::uint(VERSION, &mut out);
    cbor::array(OPS.len(), &mut out);
    for op in OPS {
        cbor::text(op, &mut out);
    }
    let wire = limits.to_wire();
    cbor::array(wire.len(), &mut out);
    for n in wire {
        cbor::uint(n, &mut out);
    }
    match devices_seq {
        Some(seq) => cbor::uint(seq, &mut out),
        None => cbor::null(&mut out),
    }
    out
}

/// Machines whose addresses a `devices` answer carries, at most.
pub(crate) const DEVICES_ADDRS_MACHINES: usize = 32;
/// Addresses of one machine in a `devices` answer, at most.
pub(crate) const DEVICES_ADDRS_PER_MACHINE: usize = 4;

/// `devices`: one item `[list or null, addrs: [[node, [text]]]]`.
pub(crate) fn devices_answer(
    list: Option<&[u8]>,
    addrs: &[(NodeKey, Vec<std::net::SocketAddr>)],
) -> Vec<u8> {
    let mut out = ok();
    cbor::array(2, &mut out);
    match list {
        Some(list) => cbor::bytes(list, &mut out),
        None => cbor::null(&mut out),
    }
    let machines: Vec<_> = addrs
        .iter()
        .filter(|(_, a)| !a.is_empty())
        .take(DEVICES_ADDRS_MACHINES)
        .collect();
    cbor::array(machines.len(), &mut out);
    for (node, addrs) in machines {
        cbor::array(2, &mut out);
        cbor::bytes(node.as_bytes(), &mut out);
        let addrs = &addrs[..addrs.len().min(DEVICES_ADDRS_PER_MACHINE)];
        cbor::array(addrs.len(), &mut out);
        for addr in addrs {
            cbor::text(&addr.to_string(), &mut out);
        }
    }
    out
}

/// `pair`: one item `[list]`.
pub(crate) fn pair_answer(list: &[u8]) -> Vec<u8> {
    let mut out = ok();
    cbor::array(1, &mut out);
    cbor::bytes(list, &mut out);
    out
}

/// `ok` as one array, then one item per channel.
pub(crate) fn heads_answer(heads: &[(PublicKey, Option<HeadInfo>)]) -> Vec<u8> {
    let mut out = ok();
    for (channel, head) in heads {
        match head {
            Some(h) => {
                cbor::array(4, &mut out);
                cbor::bytes(channel.as_bytes(), &mut out);
                cbor::uint(h.seq, &mut out);
                cbor::bytes(h.id.as_bytes(), &mut out);
                cbor::boolean(h.forked, &mut out);
            }
            None => {
                cbor::array(2, &mut out);
                cbor::bytes(channel.as_bytes(), &mut out);
                cbor::null(&mut out);
            }
        }
    }
    out
}

/// `query`: one item `[hits]`, each hit `[channel, seq, entry]`.
pub(crate) fn query_answer(hits: &[Hit]) -> Vec<u8> {
    let mut out = ok();
    cbor::array(1, &mut out);
    cbor::array(hits.len(), &mut out);
    for hit in hits {
        cbor::array(3, &mut out);
        cbor::bytes(hit.channel.as_bytes(), &mut out);
        cbor::uint(hit.seq, &mut out);
        cbor::bytes(hit.entry.as_bytes(), &mut out);
    }
    out
}

/// `["ok"]`, then one byte string per entry.
pub(crate) fn entries_answer(entries: &[&[u8]]) -> Vec<u8> {
    let mut out = ok();
    for e in entries {
        cbor::bytes(e, &mut out);
    }
    out
}

/// Size of an entry as an item of the `entries` answer.
pub(crate) fn entry_item_len(len: usize) -> usize {
    let mut head = Vec::new();
    cbor::head(2, len as u64, &mut head);
    head.len() + len
}
/// Size of the `["ok"]` item that opens an answer sequence.
pub(crate) const OK_ITEM_LEN: usize = 4;

pub(crate) fn fork_answer(pair: Option<(&Entry, &Entry)>) -> Vec<u8> {
    let mut out = ok();
    match pair {
        Some((a, b)) => {
            cbor::array(2, &mut out);
            cbor::bytes(a.bytes(), &mut out);
            cbor::bytes(b.bytes(), &mut out);
        }
        None => cbor::null(&mut out),
    }
    out
}

// ---- answers: parsing (caller) ----

/// What is wrong with an answer.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AnswerError {
    /// The callee answered `["err", code, message]`.
    Refused(Code),
    /// Not a well-formed answer.
    Bad(String),
}

fn key32<'a>(item: &Item<'a>) -> Option<&'a [u8; 32]> {
    match item {
        Item::Bytes(b) => <&[u8; 32]>::try_from(*b).ok(),
        _ => None,
    }
}

fn bad<T>(cause: &str) -> Result<T, AnswerError> {
    Err(AnswerError::Bad(cause.to_owned()))
}

/// The items of an answer, after its `ok` item. An `err` item is `Refused`.
fn items(bytes: &[u8]) -> Result<Vec<Item<'_>>, AnswerError> {
    let mut reader = Reader::new(bytes);
    let mut out = Vec::new();
    while !reader.at_end() {
        out.push(
            reader
                .item()
                .map_err(|e| AnswerError::Bad(format!("not deterministic CBOR: {e}")))?,
        );
    }
    let mut it = out.into_iter();
    match it.next() {
        Some(Item::Array(first)) => match first.as_slice() {
            [Item::Text("ok")] => Ok(it.collect()),
            [Item::Text("err"), Item::Text(code), Item::Text(_)] => match Code::parse(code) {
                Some(code) => Err(AnswerError::Refused(code)),
                None => bad("an unknown error code"),
            },
            _ => bad("the first item is neither ok nor err"),
        },
        _ => bad("the first item is neither ok nor err"),
    }
}

pub(crate) fn parse_hello(bytes: &[u8]) -> Result<Hello, AnswerError> {
    let items = items(bytes)?;
    let [Item::Array(fields)] = items.as_slice() else {
        return bad("not a hello answer");
    };
    let [
        Item::Uint(version),
        Item::Array(ops),
        Item::Array(limits),
        devices_seq,
    ] = fields.as_slice()
    else {
        return bad("not a hello answer");
    };
    let ops = ops
        .iter()
        .map(|o| match o {
            Item::Text(t) => Ok((*t).to_owned()),
            _ => bad("an op is not text"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let limits = limits
        .iter()
        .map(|n| match n {
            Item::Uint(n) => Some(*n),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .and_then(|v| Limits::from_wire(&v));
    let Some(limits) = limits else {
        return bad("the limits are not ten unsigned integers");
    };
    let devices_seq = match devices_seq {
        Item::Null => None,
        Item::Uint(n) => Some(*n),
        _ => return bad("devices_seq is neither null nor an integer"),
    };
    Ok(Hello {
        version: *version,
        ops,
        limits,
        devices_seq,
    })
}

/// A head item: seq, id, forked.
pub(crate) type RawHead = (u64, [u8; 32], bool);
/// Two byte strings.
pub(crate) type RawPair<'a> = (&'a [u8], &'a [u8]);

pub(crate) fn parse_heads(bytes: &[u8]) -> Result<Vec<(PublicKey, Option<RawHead>)>, AnswerError> {
    let items = items(bytes)?;
    let mut out = Vec::with_capacity(items.len());
    for item in &items {
        let Item::Array(fields) = item else {
            return bad("a heads item is not an array");
        };
        let key = key32;
        match fields.as_slice() {
            [c, Item::Null] => {
                let channel = key(c).and_then(|k| PublicKey::from_bytes(k).ok());
                let Some(channel) = channel else {
                    return bad("a heads item names no valid channel");
                };
                out.push((channel, None));
            }
            [c, Item::Uint(seq), id, Item::Bool(forked)] => {
                let channel = key(c).and_then(|k| PublicKey::from_bytes(k).ok());
                let (Some(channel), Some(id)) = (channel, key(id)) else {
                    return bad("a heads item has a bad channel or id");
                };
                out.push((channel, Some((*seq, *id, *forked))));
            }
            _ => return bad("a heads item has the wrong shape"),
        }
    }
    Ok(out)
}

pub(crate) fn parse_query(bytes: &[u8]) -> Result<Vec<Hit>, AnswerError> {
    let items = items(bytes)?;
    let [Item::Array(outer)] = items.as_slice() else {
        return bad("not a query answer");
    };
    let [Item::Array(hits)] = outer.as_slice() else {
        return bad("not a query answer");
    };
    let mut out = Vec::with_capacity(hits.len());
    for hit in hits {
        let Item::Array(fields) = hit else {
            return bad("a hit is not an array");
        };
        let [channel, Item::Uint(seq), entry] = fields.as_slice() else {
            return bad("a hit has the wrong shape");
        };
        let (Some(channel), Some(entry)) = (key_item(channel), key32(entry)) else {
            return bad("a hit has a bad channel or entry id");
        };
        out.push(Hit {
            channel,
            seq: *seq,
            entry: entry_id(*entry),
        });
    }
    Ok(out)
}

pub(crate) fn parse_entries(bytes: &[u8]) -> Result<Vec<&[u8]>, AnswerError> {
    let items = items(bytes)?;
    items
        .into_iter()
        .map(|i| match i {
            Item::Bytes(b) => Ok(b),
            _ => bad("an entry item is not a byte string"),
        })
        .collect()
}

pub(crate) fn parse_fork(bytes: &[u8]) -> Result<Option<RawPair<'_>>, AnswerError> {
    let items = items(bytes)?;
    match items.as_slice() {
        [Item::Null] => Ok(None),
        [Item::Array(pair)] => match pair.as_slice() {
            [Item::Bytes(a), Item::Bytes(b)] => Ok(Some((a, b))),
            _ => bad("a fork answer is not two byte strings"),
        },
        _ => bad("not a fork answer"),
    }
}

/// A `devices` answer: the list when it holds one, and the addresses of
/// other machines.
pub(crate) struct DevicesAnswer<'a> {
    pub(crate) list: Option<&'a [u8]>,
    pub(crate) addrs: Vec<(NodeKey, Vec<std::net::SocketAddr>)>,
}

/// Decode a `devices` answer: at most 32 machines of at most 4 addresses,
/// each a socket address in text. Whether a machine is in the caller's list
/// is for the caller (`BadObject` too).
pub(crate) fn parse_devices(bytes: &[u8]) -> Result<DevicesAnswer<'_>, AnswerError> {
    let items = items(bytes)?;
    let [Item::Array(fields)] = items.as_slice() else {
        return bad("not a devices answer");
    };
    let (list, machines) = match fields.as_slice() {
        [Item::Null, Item::Array(m)] => (None, m),
        [Item::Bytes(list), Item::Array(m)] => (Some(*list), m),
        _ => return bad("not a devices answer"),
    };
    if machines.len() > DEVICES_ADDRS_MACHINES {
        return bad("a devices answer names too many machines");
    }
    let mut addrs = Vec::with_capacity(machines.len());
    for machine in machines {
        let Item::Array(pair) = machine else {
            return bad("an address entry is not an array");
        };
        let [Item::Bytes(node), Item::Array(texts)] = pair.as_slice() else {
            return bad("an address entry is not [node, [text]]");
        };
        let Some(node) = <[u8; 32]>::try_from(*node).ok().map(NodeKey::from_bytes) else {
            return bad("an address entry's node is not 32 bytes");
        };
        if texts.len() > DEVICES_ADDRS_PER_MACHINE {
            return bad("a machine has too many addresses");
        }
        let mut list_of = Vec::with_capacity(texts.len());
        for text in texts {
            let Item::Text(text) = text else {
                return bad("an address is not text");
            };
            let Ok(addr) = text.parse::<std::net::SocketAddr>() else {
                return bad("an address is not a socket address");
            };
            list_of.push(addr);
        }
        addrs.push((node, list_of));
    }
    Ok(DevicesAnswer { list, addrs })
}

pub(crate) fn parse_pair(bytes: &[u8]) -> Result<&[u8], AnswerError> {
    let items = items(bytes)?;
    match items.as_slice() {
        [Item::Array(fields)] => match fields.as_slice() {
            [Item::Bytes(list)] => Ok(list),
            _ => bad("not a pair answer"),
        },
        _ => bad("not a pair answer"),
    }
}

pub(crate) fn entry_id(bytes: [u8; 32]) -> EntryId {
    EntryId::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> PublicKey {
        PublicKey::from_bytes(&ed25519_pk(n)).unwrap_or_else(|_| panic!("key {n}"))
    }

    /// A valid public key: the base point's multiples are easy to find by
    /// asking `watane_log`, but any valid point will do; hash to one.
    fn ed25519_pk(n: u8) -> [u8; 32] {
        // the compressed identity-adjacent points are not all valid; use
        // the well-known point of y = 1 (identity) shifted by trying seeds
        let mut y = [0u8; 32];
        y[0] = 1;
        let mut k = 0u8;
        loop {
            y[1] = n.wrapping_add(k);
            if PublicKey::from_bytes(&y).is_ok() {
                return y;
            }
            k = k.wrapping_add(1);
            y[2] = k;
        }
    }

    fn sock(text: &str) -> std::net::SocketAddr {
        text.parse().unwrap()
    }

    /// AC5: a `devices` answer round trips, with and without a list.
    #[test]
    fn ac5_a_devices_answer_round_trips() {
        let a = NodeKey::from_bytes([1; 32]);
        let b = NodeKey::from_bytes([2; 32]);
        let addrs = vec![
            (a, vec![sock("10.0.0.1:4433"), sock("[2001:db8::1]:9")]),
            (b, vec![sock("127.0.0.1:1")]),
        ];
        for list in [None, Some(&b"list"[..])] {
            let bytes = devices_answer(list, &addrs);
            let got = parse_devices(&bytes).unwrap();
            assert_eq!(got.list, list);
            assert_eq!(got.addrs, addrs);
        }
        // a machine with no address is left out, and at most 4 and 32 go out
        let many: Vec<_> = (0..40u8)
            .map(|n| {
                let addrs = (0..6).map(|p| sock(&format!("10.0.0.{n}:{}", 1000 + p)));
                (NodeKey::from_bytes([n; 32]), addrs.collect::<Vec<_>>())
            })
            .chain([(NodeKey::from_bytes([99; 32]), vec![])])
            .collect();
        let bytes = devices_answer(None, &many);
        let got = parse_devices(&bytes).unwrap();
        assert_eq!(got.addrs.len(), 32);
        assert!(got.addrs.iter().all(|(_, a)| a.len() == 4));
        let bytes = devices_answer(None, &[]);
        assert!(parse_devices(&bytes).unwrap().addrs.is_empty());
    }

    /// AC5: each shape the caller refuses.
    #[test]
    fn ac5_a_devices_answer_is_checked() {
        fn answer(addrs: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
            let mut out = ok();
            cbor::array(2, &mut out);
            cbor::null(&mut out);
            addrs(&mut out);
            out
        }
        fn machine(node: &[u8], texts: &[&str], out: &mut Vec<u8>) {
            cbor::array(2, out);
            cbor::bytes(node, out);
            cbor::array(texts.len(), out);
            for t in texts {
                cbor::text(t, out);
            }
        }
        let refused = |bytes: Vec<u8>| matches!(parse_devices(&bytes), Err(AnswerError::Bad(_)));
        // the node is not 32 bytes
        assert!(refused(answer(|o| {
            cbor::array(1, o);
            machine(&[1; 31], &["10.0.0.1:1"], o);
        })));
        // an address that is not a socket address
        for text in ["not an address", "10.0.0.1", "10.0.0.1:99999", ""] {
            assert!(refused(answer(|o| {
                cbor::array(1, o);
                machine(&[1; 32], &[text], o);
            })));
        }
        // five addresses of one machine
        assert!(refused(answer(|o| {
            cbor::array(1, o);
            machine(&[1; 32], &["10.0.0.1:1"; 5], o);
        })));
        // thirty-three machines
        assert!(refused(answer(|o| {
            cbor::array(33, o);
            for n in 0..33u8 {
                machine(&[n; 32], &["10.0.0.1:1"], o);
            }
        })));
        // an address that is not text, an entry of the wrong shape, no list of entries
        assert!(refused(answer(|o| {
            cbor::array(1, o);
            cbor::array(2, o);
            cbor::bytes(&[1; 32], o);
            cbor::array(1, o);
            cbor::uint(1, o);
        })));
        assert!(refused(answer(|o| {
            cbor::array(1, o);
            cbor::uint(1, o);
        })));
        assert!(refused(answer(|o| cbor::uint(1, o))));
        // one field only
        let mut one = ok();
        cbor::array(1, &mut one);
        cbor::null(&mut one);
        assert!(refused(one));
    }

    fn full() -> Query {
        Query {
            text: Some("notes".into()),
            publisher: Some(key(1)),
            interface: Some(("offers".into(), "note".into())),
            origin: Some((key(2), "slug".into())),
            limit: 200,
        }
    }

    #[test]
    fn ac2_a_query_round_trips() {
        for q in [
            full(),
            Query {
                text: None,
                publisher: None,
                interface: None,
                origin: None,
                limit: 0,
            },
        ] {
            let bytes = Request::Query(q.clone()).encode();
            assert_eq!(Request::decode(&bytes), Ok(Request::Query(q)));
        }
        for mode in ["uses", "sends"] {
            let q = Query {
                interface: Some((mode.into(), "x".into())),
                ..full()
            };
            let bytes = Request::Query(q.clone()).encode();
            assert_eq!(Request::decode(&bytes), Ok(Request::Query(q)));
        }
    }

    #[test]
    fn ac2_a_bad_query_is_a_bad_request() {
        let bad = |q: Query| Request::decode(&Request::Query(q).encode());
        assert_eq!(
            bad(Query {
                limit: 201,
                ..full()
            }),
            Err(Code::BadRequest),
            "limit"
        );
        assert_eq!(
            bad(Query {
                interface: Some(("frobs".into(), "x".into())),
                ..full()
            }),
            Err(Code::BadRequest),
            "mode"
        );
        // by hand: fields that are not what 4.4 names
        let by_hand = |fields: Vec<Vec<u8>>| {
            let mut out = Vec::new();
            cbor::array(2, &mut out);
            cbor::text("query", &mut out);
            cbor::array(fields.len(), &mut out);
            for f in fields {
                out.extend(f);
            }
            Request::decode(&out)
        };
        let null = || vec![0xf6u8];
        let uint = |n| {
            let mut o = Vec::new();
            cbor::uint(n, &mut o);
            o
        };
        let text = |t: &str| {
            let mut o = Vec::new();
            cbor::text(t, &mut o);
            o
        };
        let bytes = |b: &[u8]| {
            let mut o = Vec::new();
            cbor::bytes(b, &mut o);
            o
        };
        let ok = by_hand(vec![null(), null(), null(), null(), uint(5)]);
        assert!(matches!(ok, Ok(Request::Query(_))));
        // text that is not text
        assert_eq!(
            by_hand(vec![uint(1), null(), null(), null(), uint(5)]),
            Err(Code::BadRequest)
        );
        // a publisher of the wrong length
        assert_eq!(
            by_hand(vec![null(), bytes(&[1; 31]), null(), null(), uint(5)]),
            Err(Code::BadRequest)
        );
        // a slug that is not text
        let origin = {
            let mut o = Vec::new();
            cbor::array(2, &mut o);
            o.extend(bytes(key(2).as_bytes()));
            o.extend(uint(3));
            o
        };
        assert_eq!(
            by_hand(vec![null(), null(), null(), origin, uint(5)]),
            Err(Code::BadRequest)
        );
        // four fields; a limit that is not a number
        assert_eq!(
            by_hand(vec![null(), null(), null(), uint(5)]),
            Err(Code::BadRequest)
        );
        assert_eq!(
            by_hand(vec![null(), null(), null(), null(), text("5")]),
            Err(Code::BadRequest)
        );
    }

    #[test]
    fn ac2_a_query_answer_round_trips_and_a_bad_one_is_refused() {
        let hits = vec![
            Hit {
                channel: key(1),
                seq: 7,
                entry: entry_id([4; 32]),
            },
            Hit {
                channel: key(2),
                seq: 300,
                entry: entry_id([5; 32]),
            },
        ];
        assert_eq!(parse_query(&query_answer(&hits)), Ok(hits.clone()));
        assert_eq!(parse_query(&query_answer(&[])), Ok(Vec::new()));
        // a refusal
        let err = err_answer(Code::NotAllowed, "no");
        assert_eq!(
            parse_query(&err),
            Err(AnswerError::Refused(Code::NotAllowed))
        );
        // a hit whose channel is 31 bytes
        let mut out = ok();
        cbor::array(1, &mut out);
        cbor::array(1, &mut out);
        cbor::array(3, &mut out);
        cbor::bytes(&[1; 31], &mut out);
        cbor::uint(1, &mut out);
        cbor::bytes(&[2; 32], &mut out);
        assert!(matches!(parse_query(&out), Err(AnswerError::Bad(_))));
        // hits not in one inner array
        let mut out = ok();
        cbor::array(0, &mut out);
        assert!(matches!(parse_query(&out), Err(AnswerError::Bad(_))));
    }
}
