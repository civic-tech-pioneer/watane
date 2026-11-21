//! host's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

mod agent_load;
mod binds;
mod collect;
mod harness;
mod json;
mod manage;
mod mcp;
mod program;
mod requests;
mod runtime;
mod streams;
mod verify;
