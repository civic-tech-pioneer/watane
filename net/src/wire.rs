//! Requests and answers of `watane/1` (TECHNICAL 4.2), as deterministic CBOR.

use crate::{
    HeadInfo,
    cbor::{self, Item, Reader},
    error::Code,
};
use std::time::Duration;
use watane_log::{Entry, EntryId, MAX_ENTRY, PublicKey};

pub(crate) const ALPN: &[u8] = b"watane/1";
pub(crate) const VERSION: u64 = 1;
/// The ops of stage N2, in the order `hello` lists them.
pub(crate) const OPS: [&str; 6] = ["hello", "heads", "entries", "fork", "devices", "pair"];

/// Answer byte limits of each op: the whole stream, CBOR framing included.
pub(crate) const HELLO_ANSWER: usize = 1024;
pub(crate) const HEADS_ANSWER: usize = 1 << 20;
pub(crate) const FORK_ANSWER: usize = 2 * MAX_ENTRY + 64;
/// The answers of `devices` and `pair`: a device list and little else.
pub(crate) const LIST_ANSWER: usize = 1 << 20;
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
            ("hello" | "heads" | "entries" | "fork" | "devices" | "pair", _) => {
                Err(Code::BadRequest)
            }
            _ => Err(Code::UnknownOp),
        }
    }
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

/// `devices`: one item `[list or null, addrs]`; N2 passes on no addresses.
pub(crate) fn devices_answer(list: Option<&[u8]>) -> Vec<u8> {
    let mut out = ok();
    cbor::array(2, &mut out);
    match list {
        Some(list) => cbor::bytes(list, &mut out),
        None => cbor::null(&mut out),
    }
    cbor::array(0, &mut out);
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

/// The list of a `devices` answer, when it holds one.
pub(crate) fn parse_devices(bytes: &[u8]) -> Result<Option<&[u8]>, AnswerError> {
    let items = items(bytes)?;
    let [Item::Array(fields)] = items.as_slice() else {
        return bad("not a devices answer");
    };
    match fields.as_slice() {
        [Item::Null, Item::Array(_)] => Ok(None),
        [Item::Bytes(list), Item::Array(_)] => Ok(Some(list)),
        _ => bad("not a devices answer"),
    }
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
