//! host's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

mod adv_s96;
mod agent_load;
#[cfg(unix)] // Its homes launch the fake Claude, a Unix script made executable by mode bits.
mod agent_tools;
mod binds;
#[cfg(unix)] // Uses the Unix Claude process-group fixtures.
mod claude;
#[cfg(unix)] // Uses the Unix Claude process-group fixtures.
mod claude_launch;
mod collect;
#[cfg(unix)] // Uses the Unix Claude rig, Unix symlinks and the fake Claude script.
mod consent;
mod harness;
#[cfg(unix)] // Uses the Unix Claude process-group fixtures.
mod host_info;
mod https_stream;
mod json;
mod manage;
mod mcp;
mod program;
mod r7;
mod requests;
mod runtime;
#[cfg(unix)]
mod scripted_preview;
mod streams;
mod verify;
