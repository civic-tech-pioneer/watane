//! dist's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[path = "../support/mod.rs"]
mod support;

mod fetch;
mod follow;
mod holder;
mod index;
mod j3_dist;
mod j4_dist;
mod keyless;
mod learned_bases;
mod open;
mod own;
mod provider;
mod restore;
mod retry;
mod search;
mod site;
