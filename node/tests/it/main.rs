//! node's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[path = "../support/mod.rs"]
mod support;

mod causes;
mod holdings;
mod light;
mod open;
mod own;
mod program;
mod restore;
