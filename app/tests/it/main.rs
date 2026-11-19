//! app's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[path = "../support/mod.rs"]
mod support;

mod api;
mod build_on;
mod find;
mod fixtures;
mod gui_isolation;
mod gui_shell;
mod j10;
mod j2;
mod j3;
mod j4;
mod j5;
mod j8;
mod j9;
mod linux_runner;
mod look;
mod machines;
mod manage;
mod manager;
mod static_config;
mod static_style;
mod support_selftest;
mod two_machines;
