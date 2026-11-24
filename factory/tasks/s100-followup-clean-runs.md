Subsystem: host
Effort: medium
Depends on: none
Slice: follow-up of S100 (for the product owner; not dispatched: S100's last round was limited to four items)
## Goal
Opening a home never kills a process group that reused a pid recorded in `<home>/runs/pids`. Found by
S100's ninth adversary pass in `clean_runs` (S99 code; host TECHNICAL 3.1 step 3 mandates the kill).
Needs a spec decision first: narrow 3.1 step 3, e.g. record the boot id and each process's start
time (`/proc/<pid>/stat` field 22 on Linux, `kinfo_proc` on macOS) beside its pgid and kill only on a
match; same for `probe()`'s kill after its thread reaped the child.
## The adversary's red test (commit f7cf970, kept off slice/s100 so CI stays green)
Add to host/src/agent/claude/run.rs; run `cargo test -p watane-host --lib adv_s100_clean_runs`; it
fails today (the unrelated `sleep` is killed):
```diff
diff --git a/host/src/agent/claude/run.rs b/host/src/agent/claude/run.rs
index 5923c76..ae7aa87 100644
--- a/host/src/agent/claude/run.rs
+++ b/host/src/agent/claude/run.rs
@@ -1110,3 +1110,39 @@ mod tests {
         assert_eq!(host.state.scratch_removed("owner").unwrap(), ["kept"]);
     }
 }
+
+/// Adversary step of S100, ninth pass (red): `clean_runs` signals every pgid in `runs/pids` at
+/// open with no check that the group is still a run of this home. After a crash the recorded
+/// `claude` is reaped by init and its pid can be reused (always so after a reboot), so the open
+/// kills an unrelated process group of the participant: a signal to a reaped pid (19.3 step 5,
+/// S100-CAP "no kill reaches a pid reused since").
+#[cfg(test)]
+mod adv_s100_clean_runs {
+    use super::*;
+
+    #[test]
+    fn adv_s100_open_kills_a_group_that_reused_a_recorded_pid() {
+        let home = tempfile::tempdir().unwrap();
+        // A group the host never started, whose id a crashed run had recorded.
+        let mut unrelated = Command::new("sleep")
+            .arg("30")
+            .process_group(0)
+            .spawn()
+            .unwrap();
+        std::fs::create_dir_all(home.path().join("runs")).unwrap();
+        std::fs::write(
+            home.path().join("runs/pids"),
+            format!("{}\n", unrelated.id()),
+        )
+        .unwrap();
+        clean_runs(home.path());
+        std::thread::sleep(Duration::from_millis(200));
+        let alive = unrelated.try_wait().unwrap().is_none();
+        let _ = unrelated.kill();
+        let _ = unrelated.wait();
+        assert!(
+            alive,
+            "the open killed a process group that is not a run of this home"
+        );
+    }
+}
```
