//! net's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[path = "../support/mod.rs"]
mod support;

mod budget;
mod code;
mod faults;
mod keyfile;
mod light;
mod node;
mod pair;
mod q1;
mod reach;
mod regress_s60;
mod regress_s62;
mod regress_s67;
mod regress_s75;
mod regress_s80;
mod serve;
mod sync;
mod vectors;
