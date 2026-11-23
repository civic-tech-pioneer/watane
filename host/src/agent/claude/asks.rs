//! Asks and checks of one run (TECHNICAL 19.3, steps 4 and 5): the open ones, their ids and
//! deadlines, and every way one closes. Each closes with exactly one `ask-resolved`; the items
//! of a close go out in the order the asks were pushed.

use super::control::{self, Fields};
use super::{Out, Session, paths};
use crate::agent::streams::{Hub, StreamError};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long an ask waits for an answer (19.3, step 4).
pub(crate) const ASK_TTL: Duration = Duration::from_secs(600);

/// Most asks and checks open on one stream at a time; the next is denied at once (19.3, step 5).
pub(crate) const MAX_OPEN_ASKS: usize = 64;

/// What an `answer` says (the WIT `decision`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Decision {
    /// `allow(none)` keeps the original input; `allow(some(json))` is a changed input.
    Allow(Option<String>),
    Deny(String),
    None,
}

/// Why an `answer` is refused at the call (the WIT `session-error` cases).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Refused {
    UnknownAsk,
    NotAllowedHere,
}
impl Refused {
    pub(crate) fn case(self) -> &'static str {
        match self {
            Self::UnknownAsk => "unknown-ask",
            Self::NotAllowedHere => "not-allowed-here",
        }
    }
}

#[derive(Debug)]
struct Open {
    id: String,
    request_id: String,
    hook: bool,
    tool: String,
    input: Value,
    deadline: Instant,
}

/// The state behind `Session::asks`.
#[derive(Debug, Default)]
pub(crate) struct Asks {
    next: u64,
    open: Vec<Open>,
    /// The input bytes the host keeps of each ask whose close is not yet pushed, by ask id:
    /// reserved in the budgets at open, freed with the close (19.2, Byte budgets).
    kept: HashMap<String, usize>,
    /// `ask-resolved` items decided but not yet on the stream (it was full), with their ask's
    /// id: they go before any later item.
    pending: VecDeque<(String, Vec<u8>)>,
    /// The run's stream has ended or is about to: no `answered` goes on it any more.
    sealed: bool,
    cwd: PathBuf,
    roots: Vec<PathBuf>,
    /// The run folder's plugin folder, one of `roots`: read-only for an answer.
    run: PathBuf,
}

/// The wall clock in whole ms, and the part of a ms it drops: an `expires-at` built from it
/// is not after the deadline on the host's clock (19.3, step 5).
fn now_ms() -> (u64, Duration) {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or((0, Duration::ZERO), |d| {
            let ms = u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
            (
                ms,
                Duration::from_nanos(u64::from(d.subsec_nanos() % 1_000_000)),
            )
        })
}

impl Session {
    fn asks(&self) -> std::sync::MutexGuard<'_, Asks> {
        self.asks
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }

    /// The working folder and the folders an `allow` to a file tool may name (canonical).
    pub(crate) fn set_roots(&self, cwd: PathBuf, roots: Vec<PathBuf>, run: PathBuf) {
        let mut asks = self.asks();
        asks.cwd = cwd;
        asks.roots = roots;
        asks.run = run;
    }

    /// Pushes the closes that waited for room. Stops at the first that does not fit.
    fn flush(&self, hub: &Hub, asks: &mut Asks) -> Result<(), StreamError> {
        while let Some((id, body)) = asks.pending.front() {
            let kept = asks.kept.get(id).copied().unwrap_or(0);
            hub.host_close_keeping(&self.id, body, kept)?;
            asks.kept.remove(id);
            asks.pending.pop_front();
        }
        Ok(())
    }
    /// Closes the asks at `at` (in push order): the item is built and queued, never lost.
    fn close(&self, hub: &Hub, asks: &mut Asks, at: &[usize], by: &str, outcome: Option<&str>) {
        let mut taken = vec![];
        for (n, i) in at.iter().enumerate() {
            taken.push(asks.open.remove(i - n));
        }
        for open in taken {
            let item = control::resolved_item(&open.id, by, outcome);
            asks.pending.push_back((open.id, item));
        }
        let _ = self.flush(hub, asks);
    }

    /// Pushes one item of the run's stream, behind the closes that wait. Never waits: `Full`
    /// is for the caller to retry.
    pub(crate) fn push(&self, hub: &Hub, body: &[u8]) -> Result<(), StreamError> {
        let mut asks = self.asks();
        self.flush(hub, &mut asks)?;
        hub.host_push(&self.id, body)
    }

    /// Opens an ask or a check of `claude` and pushes its item. `Full` and `TooLarge` are for
    /// the caller (retry, deny at once).
    pub(crate) fn open_request(
        &self,
        hub: &Hub,
        request_id: &str,
        hook: bool,
        fields: &Fields,
        ttl: Duration,
    ) -> Result<(), StreamError> {
        let mut asks = self.asks();
        self.flush(hub, &mut asks)?;
        if asks.kept.len() >= MAX_OPEN_ASKS {
            return Err(StreamError::TooLarge);
        }
        let id = (asks.next + 1).to_string();
        let path = paths::resolved(
            &fields.tool,
            &fields.input,
            &asks.cwd,
            &asks.roots,
            &asks.run,
        );
        // The deadline is taken first and less the ms part `expires-at` drops, so an answer at
        // `expires-at` never finds the ask open (19.3, step 5).
        let start = hub.now();
        let (ms, dropped) = now_ms();
        let expires_at = ms.saturating_add(u64::try_from(ttl.as_millis()).unwrap_or(0));
        // What the host keeps of the ask counts in the budgets while it is open.
        let kept = request_id.len()
            + fields.tool.len()
            + serde_json::to_vec(&fields.input).map_or(0, |v| v.len());
        hub.host_open_keeping(
            &self.id,
            &control::request_item(hook, &id, fields, path.as_deref(), expires_at),
            kept,
        )?;
        asks.kept.insert(id.clone(), kept);
        asks.next += 1;
        asks.open.push(Open {
            id,
            request_id: request_id.to_owned(),
            hook,
            tool: fields.tool.clone(),
            input: fields.input.clone(),
            deadline: start + ttl - dropped,
        });
        Ok(())
    }

    /// The checks of an `answer`, made at the call against the asks open then (19.3, step 5).
    pub(crate) fn check_answer(&self, hub: &Hub, ask: &str, d: &Decision) -> Result<(), Refused> {
        let asks = self.asks();
        let now = hub.now();
        // An ask whose `expires-at` has come is closed, whether or not the sweep has run.
        let Some(open) = asks.open.iter().find(|o| o.id == ask && o.deadline > now) else {
            return Err(Refused::UnknownAsk);
        };
        let ok = if open.hook {
            !matches!(d, Decision::Allow(_))
        } else {
            match d {
                Decision::None => false,
                Decision::Deny(_) => true,
                Decision::Allow(None) => !paths::refused_for_answer(
                    &open.tool,
                    &open.input,
                    &asks.cwd,
                    &asks.roots,
                    &asks.run,
                ),
                Decision::Allow(Some(json)) => {
                    matches!(open.tool.as_str(), "AskUserQuestion" | "ExitPlanMode")
                        && serde_json::from_str::<Value>(json).is_ok_and(|v| v.is_object())
                }
            }
        };
        if ok {
            Ok(())
        } else {
            Err(Refused::NotAllowedHere)
        }
    }

    /// Applies an `answer` of a committed chain: when the ask is still open and not due, queues
    /// the answer; the writer reports it (`answered`) once the line is written. An ask due at or
    /// before now is closed `expired` first, so an answer that arrives late writes nothing
    /// (19.3, step 5).
    pub(crate) fn apply_answer(&self, hub: &Hub, ask: &str, d: &Decision) {
        let mut asks = self.asks();
        self.expire(hub, &mut asks);
        let Some(i) = asks.open.iter().position(|o| o.id == ask) else {
            return;
        };
        let open = &asks.open[i];
        let (line, outcome) = match (open.hook, d) {
            (true, Decision::None) => (control::pass(&open.request_id), "none"),
            (_, Decision::Deny(why)) => (control::deny(&open.request_id, open.hook, why), "deny"),
            (false, Decision::Allow(changed)) => {
                let input = changed
                    .as_deref()
                    .and_then(|j| serde_json::from_str::<Value>(j).ok())
                    .unwrap_or_else(|| open.input.clone());
                (control::allow(&open.request_id, &input), "allow")
            }
            _ => return,
        };
        let open = asks.open.remove(i);
        // A failed enqueue stops the session: no `answered`, the stream ends as the process does.
        self.enqueue(Out {
            line,
            answered: Some((open.id, outcome)),
        });
    }

    /// Pushes every close that waits. A close waits on neither the stream nor a budget: its bytes
    /// were reserved when its ask opened (19.2, In flight), so this is one flush.
    pub(crate) fn drain(&self, hub: &Hub) {
        let mut asks = self.asks();
        let _ = self.flush(hub, &mut asks);
    }

    /// The stream is ending, after the writer ended and `drain` ran: a writer that was let go
    /// and finishes a write later pushes nothing (19.3, step 5).
    pub(crate) fn seal(&self) {
        let mut asks = self.asks();
        asks.sealed = true;
        asks.pending.clear();
    }

    /// The writer wrote the line of an answer: now `answered` goes on the stream.
    pub(crate) fn answered(&self, hub: &Hub, id: &str, outcome: &str) {
        let mut asks = self.asks();
        if asks.sealed {
            return;
        }
        asks.pending.push_back((
            id.to_owned(),
            control::resolved_item(id, "answered", Some(outcome)),
        ));
        let _ = self.flush(hub, &mut asks);
    }

    /// `claude` cancelled a request: closes the ask or check it names, if it is open.
    pub(crate) fn cancel_request(&self, hub: &Hub, request_id: &str) {
        let mut asks = self.asks();
        // An ask or check that is due is expired first, never cancelled (19.3, step 5).
        self.expire(hub, &mut asks);
        if let Some(i) = asks.open.iter().position(|o| o.request_id == request_id) {
            self.close(hub, &mut asks, &[i], "cancelled", None);
        }
    }

    /// `interrupt`: a deny for each open ask (none for a check), then the interrupt itself;
    /// every open ask and check closes `cancelled`.
    pub(crate) fn interrupt(&self, hub: &Hub, line: Vec<u8>) {
        let mut asks = self.asks();
        self.expire(hub, &mut asks);
        for open in asks.open.iter().filter(|o| !o.hook) {
            self.write(control::deny(&open.request_id, false, "interrupted"));
        }
        self.write(line);
        let all: Vec<usize> = (0..asks.open.len()).collect();
        self.close(hub, &mut asks, &all, "cancelled", None);
    }

    /// Closes what is due at the hub's now: an ask is denied, a check closes with nothing
    /// written.
    fn expire(&self, hub: &Hub, asks: &mut Asks) {
        let now = hub.now();
        let due: Vec<usize> = (0..asks.open.len())
            .filter(|i| asks.open[*i].deadline <= now)
            .collect();
        for i in &due {
            let open = &asks.open[*i];
            if !open.hook {
                self.write(control::deny(&open.request_id, false, "the ask expired"));
            }
        }
        self.close(hub, asks, &due, "expired", None);
    }

    /// Closes what is due; also pushes closes that waited for room.
    pub(crate) fn tick(&self, hub: &Hub) {
        let mut asks = self.asks();
        self.expire(hub, &mut asks);
        let _ = self.flush(hub, &mut asks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LocalName;
    use crate::agent::ModRef;
    use crate::agent::streams::TestClock;
    use serde_json::json;

    /// A host stream `s`, a session on it with its queue read, and a clock the test moves.
    fn rig() -> (
        std::sync::Arc<Hub>,
        Session,
        std::sync::mpsc::Receiver<Out>,
        TestClock,
    ) {
        let hub = Hub::new();
        let clock = TestClock::new();
        hub.set_clock(&clock);
        hub.open_host(
            "s".into(),
            ModRef {
                local: LocalName::parse("m").unwrap(),
                identity: "i".into(),
            },
            ("o".into(), None),
            Box::new(|| {}),
        );
        let session = Session::new("s".into(), "o".into(), None);
        let rx = session.started(0);
        rx.try_recv().unwrap(); // `initialize`
        (hub, session, rx, clock)
    }
    fn bash() -> Fields {
        Fields {
            tool_use_id: "t1".into(),
            tool: "Bash".into(),
            input: json!({"command": "ls"}),
        }
    }

    #[test]
    fn s100_fix12_ac3_a_close_waits_on_no_budget_because_its_bytes_were_reserved_at_open() {
        use crate::agent::streams::Consumer;
        let hub = Hub::new();
        let s = hub
            .test_open(Consumer::Held { origin: "o".into() }, "o".into())
            .unwrap();
        let session = Session::new(s.clone(), "o".into(), None);
        session
            .open_request(&hub, "r1", false, &bash(), Duration::from_secs(10))
            .unwrap();
        // The host budget is at its limit, the reserve of the open ask inside it.
        hub.test_fill_host_budget();
        assert_eq!(hub.host_push(&s, b"{}"), Err(StreamError::Full));
        session.apply_answer(&hub, "1", &Decision::Allow(None));
        // The writer reports the answer: the resolution goes on at once, as the last item.
        session.answered(&hub, "1", "allow");
        let start = std::time::Instant::now();
        session.drain(&hub);
        assert!(start.elapsed() < Duration::from_millis(500));
        session.seal();
        let last = hub.test_take(&s).pop().map(|(_, b)| b);
        assert_eq!(
            last.as_deref(),
            Some(&br#"{"type":"ask-resolved","id":"1","by":"answered","outcome":"allow"}"#[..])
        );
    }

    #[test]
    fn s100_fix14_ac3_asks_that_open_and_close_return_the_budgets_to_zero() {
        use crate::agent::streams::Consumer;
        let hub = Hub::new();
        let s = hub
            .test_open(Consumer::Held { origin: "o".into() }, "o".into())
            .unwrap();
        let session = Session::new(s.clone(), "o".into(), None);
        let big = Fields {
            tool_use_id: "t".into(),
            tool: "Bash".into(),
            input: json!({"command": "x".repeat(10_000)}),
        };
        for n in 1..=5 {
            session
                .open_request(&hub, &format!("r{n}"), false, &big, ASK_TTL)
                .unwrap();
        }
        assert!(hub.test_budget(&s).0 > 5 * 10_000);
        session.interrupt(&hub, b"{}".to_vec());
        session.drain(&hub);
        hub.test_take(&s);
        let (host, origin) = hub.test_budget(&s);
        assert!(host < 5 * 10_000 && origin < 5 * 10_000, "{host} {origin}");
        hub.test_close(&s, json!("cancelled"));
        hub.test_take(&s);
        assert_eq!(hub.test_budget(&s), (0, 0));
    }

    #[test]
    fn s100_fix14_ac3_an_ask_whose_input_passes_the_budget_opens_nothing() {
        use crate::agent::streams::Consumer;
        let hub = Hub::new();
        let s = hub
            .test_open(Consumer::Held { origin: "o".into() }, "o".into())
            .unwrap();
        let session = Session::new(s.clone(), "o".into(), None);
        let huge = Fields {
            tool_use_id: "t".into(),
            tool: "Bash".into(),
            input: json!({"command": "x"}),
        };
        hub.test_fill_host_budget();
        assert_eq!(
            session.open_request(&hub, "r1", false, &huge, ASK_TTL),
            Err(StreamError::Full)
        );
        assert!(hub.test_take(&s).is_empty());
        assert_eq!(session.asks().kept.len(), 0);
    }

    #[test]
    fn s100_fix14_ac4_the_65th_open_ask_is_refused_and_pushes_nothing() {
        use crate::agent::streams::Consumer;
        let hub = Hub::new();
        let s = hub
            .test_open(Consumer::Held { origin: "o".into() }, "o".into())
            .unwrap();
        let session = Session::new(s.clone(), "o".into(), None);
        for n in 0..MAX_OPEN_ASKS {
            session
                .open_request(&hub, &format!("r{n}"), n % 2 == 0, &bash(), ASK_TTL)
                .unwrap();
        }
        let before = hub.test_budget(&s);
        assert_eq!(
            session.open_request(&hub, "r65", false, &bash(), ASK_TTL),
            Err(StreamError::TooLarge)
        );
        assert_eq!(hub.test_budget(&s), before);
        assert_eq!(hub.test_take(&s).len(), MAX_OPEN_ASKS);
    }

    #[test]
    fn s100_fix12_ac3_the_largest_close_item_fits_the_reserve() {
        let item = control::resolved_item("18446744073709551615", "cancelled", Some("allow"));
        assert!(
            item.len() <= crate::agent::streams::CLOSE_RESERVE,
            "{}",
            item.len()
        );
    }

    #[test]
    fn ac4_an_answer_whose_apply_comes_after_the_deadline_writes_only_the_expiry_deny() {
        let (hub, session, rx, clock) = rig();
        session
            .open_request(&hub, "r1", false, &bash(), Duration::from_secs(10))
            .unwrap();
        // Accepted at the call, before the deadline ...
        assert_eq!(
            session.check_answer(&hub, "1", &Decision::Allow(None)),
            Ok(())
        );
        // ... its apply comes at the deadline, before any sweep ran.
        clock.advance(Duration::from_secs(10));
        session.apply_answer(&hub, "1", &Decision::Allow(None));
        let out = rx.try_recv().unwrap();
        assert_eq!(out.answered, None);
        let line: Value = serde_json::from_slice(&out.line).unwrap();
        assert_eq!(
            line["response"]["response"],
            json!({"behavior":"deny","message":"the ask expired"})
        );
        session.tick(&hub);
        assert!(rx.try_recv().is_err(), "the expiry deny is written once");
    }

    #[test]
    fn ac4_a_check_answer_whose_apply_comes_after_the_deadline_writes_nothing() {
        let (hub, session, rx, clock) = rig();
        session
            .open_request(&hub, "r1", true, &bash(), Duration::from_secs(10))
            .unwrap();
        assert_eq!(session.check_answer(&hub, "1", &Decision::None), Ok(()));
        clock.advance(Duration::from_secs(10));
        session.apply_answer(&hub, "1", &Decision::None);
        session.tick(&hub);
        assert!(rx.try_recv().is_err(), "no stdin line for a late check");
    }

    #[test]
    fn ac4_an_answer_at_the_deadline_is_unknown_ask_at_the_call() {
        let (hub, session, _rx, clock) = rig();
        session
            .open_request(&hub, "r1", false, &bash(), Duration::from_secs(10))
            .unwrap();
        clock.advance(Duration::from_secs(10));
        assert_eq!(
            session.check_answer(&hub, "1", &Decision::Deny("x".into())),
            Err(Refused::UnknownAsk)
        );
    }

    #[test]
    fn ac5_answered_is_not_pushed_by_the_apply_but_after_the_writer_wrote_the_line() {
        let (hub, session, rx, _clock) = rig();
        session
            .open_request(&hub, "r1", false, &bash(), Duration::from_secs(10))
            .unwrap();
        session.apply_answer(&hub, "1", &Decision::Allow(None));
        let out = rx.try_recv().unwrap();
        assert_eq!(out.answered, Some(("1".into(), "allow")));
        // The apply leaves `answered` to the writer: the line carries it.
        session.answered(&hub, "1", "allow");
        assert_eq!(
            session.check_answer(&hub, "1", &Decision::None),
            Err(Refused::UnknownAsk)
        );
    }

    #[test]
    fn ac5_drain_puts_a_waiting_answered_on_a_full_stream_before_the_end() {
        use crate::agent::streams::Consumer;
        let hub = Hub::new();
        let s = hub
            .test_open(Consumer::Held { origin: "o".into() }, "o".into())
            .unwrap();
        let session = Session::new(s.clone(), "o".into(), None);
        session
            .open_request(&hub, "r1", false, &bash(), Duration::from_secs(10))
            .unwrap();
        while hub.test_push(&s, b"{}").is_ok() {}
        // The stream is full: a close goes past the limit and nothing waits (review r3, F3.2).
        session.answered(&hub, "1", "allow");
        session.drain(&hub);
        // On the stream when `drain` returns, so before the run's `finish`.
        let last = hub.test_take(&s).pop().map(|(_, body)| body);
        assert_eq!(
            last.as_deref(),
            Some(&br#"{"type":"ask-resolved","id":"1","by":"answered","outcome":"allow"}"#[..])
        );
    }

    #[test]
    fn ac5_a_failed_enqueue_is_reported_and_stops_the_session() {
        let (_hub, session, rx, _clock) = rig();
        drop(rx);
        assert!(!session.write(b"x".to_vec()));
        assert!(session.stopped());
    }

    /// The expiry deny of `r1`, as written.
    fn expiry_line(out: &Out) -> Value {
        let line: Value = serde_json::from_slice(&out.line).unwrap();
        assert_eq!(out.answered, None);
        line["response"].clone()
    }
    const EXPIRED: &[u8] = br#"{"type":"ask-resolved","id":"1","by":"expired"}"#;
    fn taken(hub: &Hub, s: &str) -> Vec<Vec<u8>> {
        hub.test_take(s).into_iter().map(|(_, b)| b).collect()
    }
    /// Like `rig`, on a held stream whose items the test takes.
    fn held_rig() -> (
        std::sync::Arc<Hub>,
        String,
        Session,
        std::sync::mpsc::Receiver<Out>,
        TestClock,
    ) {
        use crate::agent::streams::Consumer;
        let hub = Hub::new();
        let clock = TestClock::new();
        hub.set_clock(&clock);
        let s = hub
            .test_open(Consumer::Held { origin: "o".into() }, "o".into())
            .unwrap();
        let session = Session::new(s.clone(), "o".into(), None);
        let rx = session.started(0);
        rx.try_recv().unwrap(); // `initialize`
        (hub, s, session, rx, clock)
    }

    #[test]
    fn s100_fix9_ac2_an_interrupt_at_the_deadline_writes_the_expiry_deny_not_interrupted() {
        let (hub, s, session, rx, clock) = held_rig();
        session
            .open_request(&hub, "r1", false, &bash(), Duration::from_secs(10))
            .unwrap();
        hub.test_take(&s);
        clock.advance(Duration::from_secs(10));
        session.interrupt(&hub, b"INT".to_vec());
        let deny = expiry_line(&rx.try_recv().unwrap());
        assert_eq!(deny["request_id"], "r1");
        assert_eq!(
            deny["response"],
            json!({"behavior":"deny","message":"the ask expired"})
        );
        assert_eq!(rx.try_recv().unwrap().line, b"INT");
        session.tick(&hub);
        rx.try_recv().unwrap_err();
        assert_eq!(taken(&hub, &s), [EXPIRED]);
    }

    #[test]
    fn s100_fix9_ac2_a_cancel_request_at_the_deadline_is_an_expiry_not_a_cancel() {
        let (hub, s, session, rx, clock) = held_rig();
        session
            .open_request(&hub, "r1", false, &bash(), Duration::from_secs(10))
            .unwrap();
        hub.test_take(&s);
        clock.advance(Duration::from_secs(10));
        session.cancel_request(&hub, "r1");
        let deny = expiry_line(&rx.try_recv().unwrap());
        assert_eq!(deny["response"]["message"], "the ask expired");
        session.tick(&hub);
        rx.try_recv().unwrap_err();
        assert_eq!(taken(&hub, &s), [EXPIRED]);
    }

    #[test]
    fn s100_fix9_ac2_a_check_at_its_deadline_is_expired_with_no_stdin_line() {
        let (hub, s, session, rx, clock) = held_rig();
        session
            .open_request(&hub, "r1", true, &bash(), Duration::from_secs(10))
            .unwrap();
        hub.test_take(&s);
        clock.advance(Duration::from_secs(10));
        session.cancel_request(&hub, "r1");
        session.tick(&hub);
        rx.try_recv().unwrap_err();
        assert_eq!(taken(&hub, &s), [EXPIRED]);
        // An interrupt at the deadline writes the interrupt only.
        session
            .open_request(&hub, "r2", true, &bash(), Duration::from_secs(10))
            .unwrap();
        hub.test_take(&s);
        clock.advance(Duration::from_secs(10));
        session.interrupt(&hub, b"INT".to_vec());
        assert_eq!(rx.try_recv().unwrap().line, b"INT");
        rx.try_recv().unwrap_err();
        assert_eq!(
            taken(&hub, &s),
            [br#"{"type":"ask-resolved","id":"2","by":"expired"}"#]
        );
    }
}
