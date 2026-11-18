//! dev's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[path = "../in_process/mod.rs"]
mod in_process;
#[path = "../support/mod.rs"]
mod support;

mod add;
mod build_on;
mod builder;
mod check;
mod contract;
mod doctor;
mod envelope;
mod install;
mod journey;
mod journey_s11;
mod publish;
mod regress_s78;
mod search;
mod serve;
mod share;
mod unit_tests;
mod vendor;
mod version;
