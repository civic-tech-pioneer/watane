//! install's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[path = "../support/mod.rs"]
mod support;

mod adv_s50b;
mod adv_s50b_fix;
mod check_signals;
mod package;
mod regress_s50b;
mod regress_s50b_fix;
mod script;
mod status_audit;
mod waits;
