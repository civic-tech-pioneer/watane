//! Asks and checks of one run (TECHNICAL 19.3, steps 4 and 5): the open ones, their ids and
//! deadlines, and every way one closes. Each closes with exactly one `ask-resolved`; the items
//! of a close go out in the order the asks were pushed.

use super::control::{self, Fields};
use super::{Session, paths};
use crate::agent::streams::{Hub, StreamError};
use serde_json::Value;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long an ask waits for an answer (19.3, step 4).
pub(crate) const ASK_TTL: Duration = Duration::from_secs(600);

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
    /// `ask-resolved` items decided but not yet on the stream (it was full): they go before
    /// any later item.
    pending: VecDeque<Vec<u8>>,
    cwd: PathBuf,
    roots: Vec<PathBuf>,
    /// The run folder's plugin folder, one of `roots`: read-only for an answer.
    run: PathBuf,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
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
        while let Some(body) = asks.pending.front() {
            hub.host_push(&self.id, body)?;
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
            asks.pending
                .push_back(control::resolved_item(&open.id, by, outcome));
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
        let id = (asks.next + 1).to_string();
        let expires_at = now_ms().saturating_add(u64::try_from(ttl.as_millis()).unwrap_or(0));
        hub.host_push(
            &self.id,
            &control::request_item(hook, &id, fields, expires_at),
        )?;
        asks.next += 1;
        asks.open.push(Open {
            id,
            request_id: request_id.to_owned(),
            hook,
            tool: fields.tool.clone(),
            input: fields.input.clone(),
            deadline: hub.now() + ttl,
        });
        Ok(())
    }

    /// The checks of an `answer`, made at the call against the asks open then (19.3, step 5).
    pub(crate) fn check_answer(&self, ask: &str, d: &Decision) -> Result<(), Refused> {
        let asks = self.asks();
        let Some(open) = asks.open.iter().find(|o| o.id == ask) else {
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

    /// Applies an `answer` of a committed chain: when the ask is still open, writes the answer
    /// and closes it `answered`. Otherwise nothing.
    pub(crate) fn apply_answer(&self, hub: &Hub, ask: &str, d: &Decision) {
        let mut asks = self.asks();
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
        self.write(line);
        self.close(hub, &mut asks, &[i], "answered", Some(outcome));
    }

    /// `claude` cancelled a request: closes the ask or check it names, if it is open.
    pub(crate) fn cancel_request(&self, hub: &Hub, request_id: &str) {
        let mut asks = self.asks();
        if let Some(i) = asks.open.iter().position(|o| o.request_id == request_id) {
            self.close(hub, &mut asks, &[i], "cancelled", None);
        }
    }

    /// `interrupt`: a deny for each open ask (none for a check), then the interrupt itself;
    /// every open ask and check closes `cancelled`.
    pub(crate) fn interrupt(&self, hub: &Hub, line: Vec<u8>) {
        let mut asks = self.asks();
        for open in asks.open.iter().filter(|o| !o.hook) {
            self.write(control::deny(&open.request_id, false, "interrupted"));
        }
        self.write(line);
        let all: Vec<usize> = (0..asks.open.len()).collect();
        self.close(hub, &mut asks, &all, "cancelled", None);
    }

    /// Closes what is due at `now`: an ask is denied, a check closes with nothing written.
    /// Also pushes closes that waited for room.
    pub(crate) fn tick(&self, hub: &Hub) {
        let now = hub.now();
        let mut asks = self.asks();
        let due: Vec<usize> = (0..asks.open.len())
            .filter(|i| asks.open[*i].deadline <= now)
            .collect();
        for i in &due {
            let open = &asks.open[*i];
            if !open.hook {
                self.write(control::deny(&open.request_id, false, "the ask expired"));
            }
        }
        self.close(hub, &mut asks, &due, "expired", None);
        let _ = self.flush(hub, &mut asks);
    }
}
