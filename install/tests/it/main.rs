//! install's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod adv_s50b;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod adv_s50b_fix;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod adv_s50c;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod adv_s89;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod adv_s94;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod check_signals;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod clean_mac;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod package;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod recovery_guards;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod regress_s50b;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod regress_s50b_fix;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod regress_s50c;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod regress_s50c_sim;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod regress_s55;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod regress_s135;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod s50c;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod s89;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod script;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod sim_s89;
#[cfg(unix)] // Installer fixtures use POSIX permissions and process APIs.
#[path = "../support/mod.rs"]
mod support;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod unit;
#[cfg(unix)] // Exercises POSIX shell tools and installer fixtures.
mod waits;

mod status_audit;
mod workflow;
