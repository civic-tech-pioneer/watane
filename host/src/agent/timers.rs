//! H7: timers per mod, kept in host state and armed from it at start (TECHNICAL 19.6, 7).

use super::unix_ms;
use crate::Host;
use crate::runtime::call::{Args, Start, Who, run_start};
use crate::state::{TimerOp, mod_id};
use serde_json::json;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::Duration;

pub(crate) const TIMER_SINK: &str = "watane:host/timer-sink@0.1.0";

/// The timers waiting for their due time, the firings queued per mod, and the firings that are
/// queued or running (19.6: one firing per row due; a worker per mod with due timers).
#[derive(Debug, Default)]
struct Queue {
    heap: BinaryHeap<Reverse<(u64, String)>>,
    /// Due firings per owner (`mod_id`) in due order.
    due: HashMap<String, VecDeque<(u64, String)>>,
    /// The owners whose worker runs: at most one waiting firing per mod.
    running: HashSet<String>,
    /// (key, due) of every firing that is queued or running: `reload` and `arm` skip them.
    flight: HashSet<(String, u64)>,
}
#[derive(Debug, Default)]
pub(crate) struct Timers {
    queue: Mutex<Queue>,
    cv: Condvar,
}
impl Timers {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }
    /// Arms a timer after its row is committed.
    pub fn arm(&self, key: String, due: u64) {
        let mut queue = self.lock();
        if !queue.flight.contains(&(key.clone(), due)) {
            queue.heap.push(Reverse((due, key)));
        }
        drop(queue);
        self.cv.notify_all();
    }
    /// Arms every row of the `timers` table (at start, and when a mod is enabled again), except
    /// the firings in progress.
    pub fn reload(&self, host: &Host) {
        match host.state.timer_rows() {
            Ok(rows) => {
                let mut queue = self.lock();
                queue.heap.clear();
                let armed: Vec<_> = rows
                    .into_iter()
                    .filter(|(key, due, _)| !queue.flight.contains(&(key.clone(), *due)))
                    .map(|(key, due, _)| Reverse((due, key)))
                    .collect();
                queue.heap.extend(armed);
                drop(queue);
                self.cv.notify_all();
            }
            Err(error) => eprintln!("watane: cannot read the timers: {}", error.message),
        }
    }
    /// Starts the firing scheduler; it holds a `Weak` of the host (19.2, Driver).
    pub fn start(self: &Arc<Self>, host: &Arc<Host>) -> std::io::Result<()> {
        self.reload(host);
        let timers = Arc::clone(self);
        let weak = Arc::downgrade(host);
        std::thread::Builder::new()
            .name("watane-timers".into())
            .spawn(move || timers.run(&weak))
            .map(|_| ())
    }
    fn run(self: &Arc<Self>, weak: &Weak<Host>) {
        let mut queue = self.lock();
        loop {
            if weak.strong_count() == 0 {
                return;
            }
            let now = unix_ms();
            match queue.heap.peek() {
                Some(Reverse((due, _))) if *due <= now => {
                    let Reverse((due, key)) = queue.heap.pop().expect("peeked");
                    if !queue.flight.insert((key.clone(), due)) {
                        continue;
                    }
                    let owner = key.split_once('/').map_or("", |(o, _)| o).to_owned();
                    queue
                        .due
                        .entry(owner.clone())
                        .or_default()
                        .push_back((due, key.clone()));
                    if queue.running.insert(owner.clone()) && !self.spawn_worker(weak, owner.clone())
                    {
                        queue.running.remove(&owner);
                        queue.due.remove(&owner);
                        queue.flight.remove(&(key, due));
                    }
                }
                next => {
                    let wait = next.map_or(100, |Reverse((due, _))| (due - now).min(100));
                    queue = self
                        .cv
                        .wait_timeout(queue, Duration::from_millis(wait))
                        .expect("lock poisoned: another thread panicked while holding it")
                        .0;
                }
            }
        }
    }
    /// One worker per mod with due timers: it fires them one at a time, in due order.
    fn spawn_worker(self: &Arc<Self>, weak: &Weak<Host>, owner: String) -> bool {
        let (timers, weak) = (Arc::clone(self), weak.clone());
        let spawned = std::thread::Builder::new()
            .name("watane-timer".into())
            .stack_size(16 << 20)
            .spawn(move || timers.work(&weak, &owner));
        if spawned.is_err() {
            eprintln!("watane: cannot start a timer thread");
        }
        spawned.is_ok()
    }
    fn work(&self, weak: &Weak<Host>, owner: &str) {
        loop {
            let next = {
                let mut queue = self.lock();
                let next = queue.due.get_mut(owner).and_then(VecDeque::pop_front);
                if next.is_none() {
                    queue.due.remove(owner);
                    queue.running.remove(owner);
                }
                next
            };
            let Some((due, key)) = next else {
                return;
            };
            if let Some(host) = weak.upgrade() {
                fire(&host, &key, due);
            }
            self.lock().flight.remove(&(key, due));
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
    let (local, exports) = {
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
            catalog
                .components
                .get(&m.local)
                .is_some_and(|c| c.host_exports().iter().any(|e| e == TIMER_SINK)),
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
    let mut start = Start::internal(
        Who::Host,
        local.clone(),
        format!("{TIMER_SINK}.fired"),
        Args::Json(json!({"id": id})),
        origin,
    );
    start.timer = Some((key.to_owned(), due));
    // The call reads the row again and checks `origin` under the mod's lock (19.6, Terms); a
    // firing it dropped for a row cancelled or replaced meanwhile is no failure.
    let done = run_start(host, start);
    if done.deferred {
        return;
    }
    if let Err(error) = done.result {
        eprintln!(
            "watane: timer {id} of {local} failed: {:?}: {}",
            error.code, error.message
        );
        let _ = delete();
    }
}
