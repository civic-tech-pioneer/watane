//! H7: timers per mod, kept in host state and armed from it at start (TECHNICAL 19.6, 7).

use super::unix_ms;
use crate::runtime::call::{Args, Start, Who, run_start};
use crate::state::{TimerOp, mod_id};
use crate::{CallCode, Host};
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
    /// Rows that `reload` skipped because their firing was in flight: armed again when that
    /// firing ends, so one that left early (its mod was disabled) is not lost.
    skipped: HashSet<(String, u64)>,
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
    /// the firings in progress. It keeps what is armed already: an `arm` between the read of the
    /// rows and this lock is not lost, and a stale entry fires nothing (its row is read again).
    pub fn reload(&self, host: &Host) {
        match host.state.timer_rows() {
            Ok(rows) => {
                let mut queue = self.lock();
                let armed: HashSet<(u64, String)> = queue
                    .heap
                    .iter()
                    .map(|Reverse(entry)| entry.clone())
                    .collect();
                for (key, due, _) in rows {
                    if queue.flight.contains(&(key.clone(), due)) {
                        queue.skipped.insert((key, due));
                    } else if !armed.contains(&(due, key.clone())) {
                        queue.heap.push(Reverse((due, key)));
                    }
                }
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
                    if queue.running.insert(owner.clone())
                        && !self.spawn_worker(weak, owner.clone())
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
        let (timers, weak) = (Arc::clone(self), Weak::clone(weak));
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
            let mut queue = self.lock();
            queue.flight.remove(&(key.clone(), due));
            if queue.skipped.remove(&(key.clone(), due)) {
                queue.heap.push(Reverse((due, key)));
                self.cv.notify_all();
            }
        }
    }
}

fn has_sink(catalog: &crate::catalog::Catalog, local: &crate::LocalName) -> bool {
    catalog
        .components
        .get(local)
        .is_some_and(|c| c.host_exports().iter().any(|e| e == TIMER_SINK))
}

/// The delete of a row whose mod has no `timer-sink`, decided in the write transaction: none
/// once the install read at the firing is no longer current or has the export (19.6).
fn no_sink_delete(
    catalog: &crate::catalog::Catalog,
    local: &crate::LocalName,
    generation: Option<u64>,
    key: &str,
    due: u64,
) -> Vec<TimerOp> {
    if catalog.install_of(local) == generation && !has_sink(catalog, local) {
        vec![TimerOp::DeleteIfDue {
            key: key.to_owned(),
            due,
        }]
    } else {
        Vec::new()
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
    let (local, exports, generation) = {
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
            has_sink(&catalog, &m.local),
            catalog.install_of(&m.local),
        )
    };
    let delete = || {
        host.state.timer_write(&[TimerOp::DeleteIfDue {
            key: key.to_owned(),
            due,
        }])
    };
    if !exports {
        // The row goes only if the same install still has no `timer-sink`, checked in the
        // write transaction that deletes it (19.6).
        let _ = host.state.timer_write_with(&|| {
            let catalog = host
                .catalog
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            no_sink_delete(&catalog, &local, generation, key, due)
        });
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
        // A mod disabled, removed or updated while the firing waited keeps its row: its timers
        // wait, and `enable`, an update or the next load arms them (19.6). A nested call that
        // failed for another mod is a failure like any other.
        if done.unavailable && matches!(error.code, CallCode::Disabled | CallCode::NoMod) {
            return;
        }
        eprintln!(
            "watane: timer {id} of {local} failed: {:?}: {}",
            error.code, error.message
        );
        let _ = delete();
    }
}

#[cfg(test)]
mod tests {
    use crate::{Host, Limits, Mode};

    /// A firing rechecks its row after waiting for the mod's call lock (19.6).
    #[test]
    fn ac3_a_timer_cancelled_while_its_firing_waits_does_not_run() {
        use super::{Arc, Duration, Mutex, TimerOp, unix_ms};
        use std::time::Instant;

        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        let advo =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods/advo");
        host.install_folders(&[advo], 0).unwrap();

        let lock = Arc::new(Mutex::new(()));
        host.call_locks
            .lock()
            .unwrap()
            .insert("local/advo".into(), Arc::clone(&lock));

        for trial in 0..=20 {
            let cancel = trial < 20;
            let key = crate::state::timer_key("local/advo", &format!("t{trial}"));
            let due = unix_ms();
            // Arm only after taking the mod's lock, so the due timer cannot beat the cancel.
            let held = lock.lock().unwrap();
            host.state
                .timer_write(&[TimerOp::After {
                    key: key.clone(),
                    due,
                    origin: "local/advo".into(),
                }])
                .unwrap();

            host.timers.arm(key.clone(), due);
            let flight = (key.clone(), due);
            let deadline = Instant::now() + Duration::from_secs(3);
            // The map and this test own two references. The firing clones the third
            // after it reads the timer row, just before waiting on the mod's lock.
            while Arc::strong_count(&lock) < 3 {
                assert!(
                    Instant::now() < deadline,
                    "the firing did not reach the call lock"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            if cancel {
                host.state.timer_write(&[TimerOp::Cancel { key }]).unwrap();
            }
            drop(held);
            let deadline = Instant::now() + Duration::from_secs(3);
            while host.timers.lock().flight.contains(&flight) {
                assert!(Instant::now() < deadline, "the firing did not finish");
                std::thread::sleep(Duration::from_millis(1));
            }

            let mut state = host.state.transaction("local/advo", &host.limits).unwrap();
            assert_eq!(
                state.get("fired").unwrap(),
                if cancel { None } else { Some(b"t20".to_vec()) },
                "only the uncancelled control timer should fire"
            );
        }
    }

    /// A timer armed after `reload` read the rows (its row committed after that read) stays
    /// armed: `reload` keeps what is armed already, and arms a row once (19.6).
    #[test]
    fn reload_keeps_a_timer_armed_after_its_read() {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        let far = super::unix_ms() + 3_600_000;
        host.state
            .timer_write(&[super::TimerOp::After {
                key: "m/y".into(),
                due: far,
                origin: "m".into(),
            }])
            .unwrap();
        host.timers.arm("m/x".into(), far);
        host.timers.reload(&host);
        host.timers.reload(&host);
        let queue = host.timers.lock();
        let mut entries: Vec<_> = queue.heap.iter().map(|e| e.0.clone()).collect();
        entries.sort();
        assert_eq!(
            entries,
            vec![(far, "m/x".to_owned()), (far, "m/y".to_owned())]
        );
    }
    /// AC4 (S96-fix4), 19.6: the no-`timer-sink` delete happens only while the install that the
    /// firing read is current; an install in between keeps the row.
    #[test]
    fn ac4_no_sink_delete_only_for_the_install_it_read() {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        let local = crate::LocalName::parse("a").unwrap();
        let mut catalog = host.catalog.lock().unwrap();
        catalog.bump_install(&local);
        let read = catalog.install_of(&local);
        let ops = super::no_sink_delete(&catalog, &local, read, "m/x", 7);
        assert!(
            matches!(ops.as_slice(), [super::TimerOp::DeleteIfDue { key, due: 7 }] if key == "m/x"),
            "{ops:?}"
        );
        catalog.bump_install(&local);
        assert!(super::no_sink_delete(&catalog, &local, read, "m/x", 7).is_empty());
    }
}
