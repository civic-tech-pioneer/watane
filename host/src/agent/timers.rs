//! H7: timers per mod, kept in host state and armed from it at start (TECHNICAL 19.6, 7).

use super::unix_ms;
use crate::Host;
use crate::runtime::call::{Args, Start, Who, run_start};
use crate::state::{TimerOp, mod_id};
use serde_json::json;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::Duration;

pub(crate) const TIMER_SINK: &str = "watane:host/timer-sink@0.1.0";

#[derive(Debug, Default)]
pub(crate) struct Timers {
    heap: Mutex<BinaryHeap<Reverse<(u64, String)>>>,
    cv: Condvar,
}
impl Timers {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }
    /// Arms a timer after its row is committed.
    pub fn arm(&self, key: String, due: u64) {
        self.heap
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .push(Reverse((due, key)));
        self.cv.notify_all();
    }
    /// Arms every row of the `timers` table (at start, and when a mod is enabled again).
    pub fn reload(&self, host: &Host) {
        match host.state.timer_rows() {
            Ok(rows) => {
                let mut heap = self
                    .heap
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it");
                heap.clear();
                heap.extend(rows.into_iter().map(|(key, due, _)| Reverse((due, key))));
                drop(heap);
                self.cv.notify_all();
            }
            Err(error) => eprintln!("watane: cannot read the timers: {}", error.message),
        }
    }
    /// Starts the firing worker; it holds a `Weak` of the host (19.2, Driver).
    pub fn start(self: &Arc<Self>, host: &Arc<Host>) -> std::io::Result<()> {
        self.reload(host);
        let timers = Arc::clone(self);
        let weak = Arc::downgrade(host);
        std::thread::Builder::new()
            .name("watane-timers".into())
            .spawn(move || timers.run(&weak))
            .map(|_| ())
    }
    fn run(&self, weak: &Weak<Host>) {
        let mut heap = self
            .heap
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        loop {
            if weak.strong_count() == 0 {
                return;
            }
            let now = unix_ms();
            match heap.peek() {
                Some(Reverse((due, _))) if *due <= now => {
                    let Reverse((due, key)) = heap.pop().expect("peeked");
                    drop(heap);
                    if let Some(host) = weak.upgrade() {
                        let spawned = std::thread::Builder::new()
                            .name("watane-timer".into())
                            .stack_size(16 << 20)
                            .spawn(move || fire(&host, &key, due));
                        if spawned.is_err() {
                            eprintln!("watane: cannot start a timer thread");
                        }
                    }
                    heap = self
                        .heap
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it");
                }
                next => {
                    let wait = next.map_or(100, |Reverse((due, _))| (due - now).min(100));
                    heap = self
                        .cv
                        .wait_timeout(heap, Duration::from_millis(wait))
                        .expect("lock poisoned: another thread panicked while holding it")
                        .0;
                }
            }
        }
    }
}

/// A due timer fires as a top-level call of `timer-sink.fired` with caller `host` (19.6).
fn fire(host: &Arc<Host>, key: &str, due: u64) {
    let Ok(Some((row_due, origin))) = host.state.timer_row(key) else {
        return;
    };
    if row_due != due {
        return;
    }
    let Some((owner_id, id)) = key.split_once('/') else {
        return;
    };
    let (local, identity, exports, origin_ok) = {
        let catalog = host
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let Some(m) = catalog
            .mods
            .iter()
            .find(|m| mod_id(&m.identity) == owner_id && m.error.is_none())
        else {
            return;
        };
        if !m.enabled {
            // A disabled mod gets no firing; its timers wait (19.6). `enable` arms them again.
            return;
        }
        (
            m.local.clone(),
            m.identity.clone(),
            catalog
                .components
                .get(&m.local)
                .is_some_and(|c| c.host_exports().iter().any(|e| e == TIMER_SINK)),
            catalog
                .mods
                .iter()
                .any(|o| o.identity == origin && o.enabled && o.error.is_none()),
        )
    };
    let delete = || {
        host.state.timer_write(&[TimerOp::DeleteIfDue {
            key: key.to_owned(),
            due,
        }])
    };
    if !exports {
        let _ = delete();
        eprintln!("watane: timer {id} of {local} dropped: no timer-sink");
        return;
    }
    let origin = if origin_ok { origin } else { identity };
    let mut start = Start::internal(
        Who::Host,
        local.clone(),
        format!("{TIMER_SINK}.fired"),
        Args::Json(json!({"id": id})),
        origin,
    );
    start.timer = Some((key.to_owned(), due));
    if let Err(error) = run_start(host, start).result {
        eprintln!(
            "watane: timer {id} of {local} failed: {:?}: {}",
            error.code, error.message
        );
        let _ = delete();
    }
}
