//! dev's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[path = "../in_process/mod.rs"]
mod in_process;
#[path = "../support/mod.rs"]
mod support;

mod add;
mod build_on;
#[cfg(unix)] // Fixture toolchains use executable POSIX scripts.
mod builder;
mod check;
mod contract;
#[cfg(unix)] // Fixture programs use executable POSIX scripts.
mod doctor;
mod envelope;
#[cfg(unix)] // Session fixtures use POSIX symlinks and signals.
mod fake_claude;
mod install;
#[cfg(unix)] // The journey starts and stops Unix preview process groups.
mod journey;
#[cfg(unix)] // The journey starts and stops Unix preview process groups.
mod journey_s11;
mod publish;
mod regress_s78;
mod scripted;
mod scripted_host;
#[cfg(all(unix, debug_assertions, feature = "fake-claude"))]
mod scripted_preview;
mod search;
#[cfg(unix)] // Preview start and stop depend on Unix process groups.
mod serve;
#[cfg(windows)] // Verifies both Windows preview commands fail before starting a process.
mod serve_windows;
mod share;
mod unit_tests;
mod vendor;
mod version;
