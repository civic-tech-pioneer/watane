//! host's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

mod adv_s96;
mod agent_load;
mod binds;
#[cfg(unix)] // Uses the Unix Claude process-group fixtures.
mod claude;
#[cfg(unix)] // Uses the Unix Claude process-group fixtures.
mod claude_launch;
mod collect;
mod harness;
#[cfg(unix)] // Uses the Unix Claude process-group fixtures.
mod host_info;
mod json;
mod manage;
mod mcp;
mod program;
mod r7;
mod requests;
mod runtime;
mod streams;
mod verify;
