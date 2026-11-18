//! log's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[path = "../oracle/mod.rs"]
mod oracle;

mod append;
mod backup;
mod concurrency;
mod corrupt;
mod entry_codec;
mod identity_file;
mod ingest;
mod key_text;
mod keyless;
mod model;
mod open_errors;
mod restore_head;
mod strict_verify;
