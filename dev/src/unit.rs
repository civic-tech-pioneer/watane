use crate::{
    builder::{self, BuildCtx},
    outcome::{Code, Problem},
};
use serde::Serialize;
use std::{path::Path, sync::mpsc, thread, time::Duration};
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Trap,
    component::{Component, Linker, ResourceTable, types::ComponentItem},
};
use wasmtime_wasi::{
    WasiCtx, WasiCtxView, WasiView,
    p2::{bindings::sync::Command, pipe::MemoryOutputPipe},
};
use watane_pkg::Slug;

#[derive(Default, Serialize)]
pub struct UnitReport {
    pub passed: u32,
    pub failed: u32,
    pub ignored: u32,
    pub not_run: u32,
    pub failures: Vec<UnitFailure>,
}
#[derive(Serialize)]
pub struct UnitFailure {
    pub test: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    #[serde(skip)]
    code: Code,
}
impl UnitReport {
    pub(crate) fn problems(&self) -> Vec<Problem> {
        self.failures
            .iter()
            .map(|f| {
                let message = if f.code == Code::TestUnitLimit {
                    f.message.clone()
                } else {
                    format!("{}: {}", f.test, f.message)
                };
                let mut problem = Problem::new(f.code, message)
                    .fix("Correct the unit test or its logic, then run: watane-dev test --json");
                problem.file = f.file.clone();
                problem.line = f.line;
                problem
            })
            .collect()
    }
}

fn location(text: &str, slug: &str) -> (Option<String>, Option<u32>) {
    let mut parts = text.trim_end_matches(':').rsplitn(3, ':');
    let _column = parts.next();
    let line = parts.next().and_then(|s| s.parse().ok());
    let Some(path) = parts.next().filter(|_| line.is_some()) else {
        return (None, None);
    };
    let remapped = format!("/watane-dev/build/{slug}/");
    let path = path.strip_prefix(&remapped).unwrap_or(path);
    let path = if path.starts_with("src/") {
        format!("logic/{path}")
    } else {
        path.into()
    };
    (Some(path), line)
}

/// Read both completed libtest output and a binary aborted in its current test.
pub fn parse_libtest(stdout: &str, stderr: &str, slug: &str) -> UnitReport {
    let mut report = UnitReport::default();
    let mut total: u32 = 0;
    let mut current = None;
    for line in stdout.lines() {
        if let Some(count) = line
            .strip_prefix("running ")
            .and_then(|s| s.split_whitespace().next())
        {
            total = count.parse().unwrap_or(0);
        }
        let status = if let Some((name, status)) = line
            .strip_prefix("test ")
            .and_then(|s| s.split_once(" ... "))
        {
            current = Some(name);
            status.trim()
        } else if current.is_some() {
            line.trim()
        } else {
            continue;
        };
        match status {
            "ok" => {
                report.passed += 1;
                current = None;
            }
            s if s.starts_with("ignored") => {
                report.ignored += 1;
                current = None;
            }
            "FAILED" => {
                report.failed += 1;
                report.failures.push(UnitFailure {
                    test: current.take().unwrap().into(),
                    message: "unit test failed".into(),
                    file: None,
                    line: None,
                    code: Code::TestUnit,
                });
            }
            _ => {}
        }
    }
    if let Some(name) = current {
        report.failed += 1;
        report.failures.push(UnitFailure {
            test: name.into(),
            message: "unit test aborted".into(),
            file: None,
            line: None,
            code: Code::TestUnit,
        });
    }
    let combined = format!("{stdout}\n{stderr}");
    let lines: Vec<_> = combined.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let Some((thread, at)) = line.split_once(" panicked at ") else {
            continue;
        };
        let name = thread.split('\'').nth(1);
        let failure = if report.failures.len() == 1 {
            report.failures.first_mut()
        } else {
            report
                .failures
                .iter_mut()
                .find(|f| Some(f.test.as_str()) == name)
        };
        if let Some(failure) = failure {
            (failure.file, failure.line) = location(at, slug);
            let end = lines[index + 1..]
                .iter()
                .position(|s| {
                    s.starts_with("note:")
                        || s.starts_with("stack backtrace:")
                        || s.starts_with("---- ")
                        || s.starts_with("failures:")
                        || s.starts_with("test result:")
                })
                .map_or(lines.len(), |n| index + 1 + n);
            failure.message = lines[index + 1..end].join("\n").trim().to_string();
            if failure.message.is_empty() {
                failure.message = (*line).into();
            }
        }
    }
    report.not_run = total.saturating_sub(report.passed + report.failed + report.ignored);
    report
}

struct State {
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
}
impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

fn run(
    engine: &Engine,
    component: &Component,
    slug: &Slug,
    seconds: u64,
) -> wasmtime::Result<UnitReport> {
    let stdout = MemoryOutputPipe::new(1 << 20);
    let stderr = MemoryOutputPipe::new(1 << 20);
    let mut wasi = WasiCtx::builder();
    wasi.args(&["test", "--test-threads=1", "--nocapture"])
        .stdout(stdout.clone())
        .stderr(stderr.clone())
        .allow_tcp(false)
        .allow_udp(false)
        .allow_ip_name_lookup(false);
    let mut store = Store::new(
        engine,
        State {
            wasi: wasi.build(),
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new()
                .memory_size(512 << 20)
                .trap_on_grow_failure(true)
                .build(),
        },
    );
    store.limiter(|s| &mut s.limits);
    store.set_epoch_deadline(seconds.saturating_mul(10));
    store.epoch_deadline_trap();
    let mut linker = Linker::new(engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
    // Wasmtime 48's define_unknown_imports_as_traps also stubs resource aliases
    // in already linked WASI interfaces, breaking their resource identities.
    // Stub only the mod/provider functions; WASI retains its typed, empty context.
    for (interface, item) in component.component_type().imports(engine) {
        if interface.starts_with("wasi:") {
            continue;
        }
        let ComponentItem::ComponentInstance(instance) = item.ty else {
            return Err(wasmtime::Error::msg(format!(
                "unsupported unit import {interface}"
            )));
        };
        let mut imported = linker.instance(interface)?;
        for (name, item) in instance.exports(engine) {
            if let ComponentItem::ComponentFunc(_) = item.ty {
                let message = format!("unknown import: {interface}#{name}");
                imported.func_new(name, move |_, _, _, _| {
                    Err(wasmtime::Error::msg(message.clone()))
                })?;
            }
        }
    }
    let (stop, stopped) = mpsc::channel();
    let clock_engine = engine.clone();
    let clock = thread::spawn(move || {
        while stopped.recv_timeout(Duration::from_millis(100))
            == Err(mpsc::RecvTimeoutError::Timeout)
        {
            clock_engine.increment_epoch();
        }
    });
    let result = Command::instantiate(&mut store, component, &linker)
        .and_then(|command| command.wasi_cli_run().call_run(&mut store));
    let _ = stop.send(());
    let _ = clock.join();
    let out = String::from_utf8_lossy(&stdout.contents()).into_owned();
    let err = String::from_utf8_lossy(&stderr.contents()).into_owned();
    let mut report = parse_libtest(&out, &err, slug.as_str());
    let error = match result {
        Ok(Ok(())) => None,
        Ok(Err(())) => Some((
            Code::TestUnit,
            "unit binary exited unsuccessfully".to_string(),
        )),
        Err(error) => {
            let detail = format!("{error:#}");
            let test = report.failures.last().map_or("<unit binary>", |f| &f.test);
            if error.downcast_ref::<Trap>() == Some(&Trap::Interrupt) {
                Some((
                    Code::TestUnitLimit,
                    format!("unit tests stopped at {test} after {seconds} s"),
                ))
            } else if detail.contains("forcing trap when growing memory")
                || detail.contains("memory growth failure")
                || err.contains("memory allocation of")
            {
                Some((
                    Code::TestUnitLimit,
                    format!("unit tests stopped at {test}: used more than 512 MiB"),
                ))
            } else {
                Some((Code::TestUnit, detail))
            }
        }
    };
    if let Some((code, message)) = error {
        if let Some(failure) = report.failures.last_mut() {
            failure.code = code;
            if code == Code::TestUnitLimit || failure.file.is_none() {
                failure.message = if code == Code::TestUnitLimit {
                    message
                } else {
                    format!("{message}\n{err}").trim().into()
                };
            }
        } else {
            report.failed += 1;
            report.failures.push(UnitFailure {
                test: "<unit binary>".into(),
                message,
                file: None,
                line: None,
                code,
            });
        }
    }
    Ok(report)
}

/// The caller checks the mod before invoking this function. Mod code runs only in Wasmtime.
pub fn unit_tests(dir: &Path, slug: &Slug, ctx: &BuildCtx<'_>) -> Result<UnitReport, Vec<Problem>> {
    let _lock = builder::build_lock(slug, ctx)?;
    let (copy, _) = builder::prepare(dir, slug, ctx)?;
    let binaries = builder::compile(
        ctx,
        &copy,
        slug,
        &[
            "test",
            "--locked",
            "--no-run",
            "--target",
            "wasm32-wasip2",
            "--message-format",
            "json",
        ],
    )
    .inspect_err(|errors| {
        if errors.iter().any(|e| e.code == Code::BuildReadsOutside) {
            let _ = std::fs::remove_file(dir.join("logic.wasm"));
        }
    })?;
    let seconds = ctx.env.unit_seconds;
    let mut config = Config::new();
    config
        .epoch_interruption(true)
        .wasm_component_model(true)
        .wasm_threads(false);
    let engine = Engine::new(&config).map_err(|e| vec![Problem::internal(e)])?;
    let mut report = UnitReport::default();
    for binary in binaries {
        let component =
            Component::from_file(&engine, binary).map_err(|e| vec![Problem::internal(e)])?;
        let result =
            run(&engine, &component, slug, seconds).map_err(|e| vec![Problem::internal(e)])?;
        report.passed += result.passed;
        report.failed += result.failed;
        report.ignored += result.ignored;
        report.not_run += result.not_run;
        report.failures.extend(result.failures);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac1_parse_passing_and_ignored_tests() {
        let report = parse_libtest(
            "running 3 tests\ntest a ... ok\ntest b ... ignored, later\ntest c ... ok\n",
            "",
            "x",
        );
        assert_eq!(
            (report.passed, report.failed, report.ignored, report.not_run),
            (2, 0, 1, 0)
        );
        assert!(report.failures.is_empty());
    }

    #[test]
    fn ac1_parse_uncaptured_test_output() {
        let report = parse_libtest(
            "running 2 tests\ntest a ... diagnostic\nmore output\nok\ntest b ... ok\n",
            "",
            "x",
        );
        assert_eq!((report.passed, report.failed, report.not_run), (2, 0, 0));
    }

    #[test]
    fn ac2_parse_abort_after_test_output() {
        let report = parse_libtest(
            "running 2 tests\ntest a ... diagnostic\n",
            "thread 'main' panicked at src/lib.rs:9:1:\nwrong value\n",
            "x",
        );
        assert_eq!((report.passed, report.failed, report.not_run), (0, 1, 1));
        assert_eq!(report.failures[0].test, "a");
        assert_eq!(report.failures[0].line, Some(9));
    }

    #[test]
    fn ac2_parse_completed_failure() {
        let report = parse_libtest(
            "running 2 tests\ntest a ... FAILED\ntest b ... ok\n",
            "thread 'a' panicked at src/lib.rs:9:1:\nwrong value\nnote: details\n",
            "x",
        );
        assert_eq!((report.passed, report.failed, report.not_run), (1, 1, 0));
        assert_eq!(report.failures[0].test, "a");
        assert_eq!(report.failures[0].message, "wrong value");
        assert_eq!(report.failures[0].file.as_deref(), Some("logic/src/lib.rs"));
        assert_eq!(report.failures[0].line, Some(9));
    }

    #[test]
    fn ac2_parse_aborted_run_and_remapped_panic() {
        let report = parse_libtest(
            "running 4 tests\ntest a ... ok\ntest b ... ",
            "thread 'main' panicked at /watane-dev/build/x/logic/src/lib.rs:40:9:\nassertion failed\n",
            "x",
        );
        assert_eq!(
            (report.passed, report.failed, report.ignored, report.not_run),
            (1, 1, 0, 2)
        );
        assert_eq!(report.failures[0].test, "b");
        assert_eq!(report.failures[0].file.as_deref(), Some("logic/src/lib.rs"));
        assert_eq!(report.failures[0].line, Some(40));
    }
}
