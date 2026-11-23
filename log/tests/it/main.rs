//! log's integration tests as one test binary: each file of this folder is a module,
//! so a change links one binary instead of one per file (factory/README.md, test kinds).

#[path = "../oracle/mod.rs"]
mod oracle;

mod append;
mod backup;
mod concurrency;
mod corrupt;
mod entry_codec;
#[cfg(unix)] // Uses POSIX mode bits, symlinks and directory permissions.
mod identity_file;
#[cfg(windows)] // Uses NTFS reparse points and Windows ACL tools.
mod identity_windows;
mod ingest;
mod key_text;
mod keyless;
mod model;
mod open_errors;
mod restore_head;
mod strict_verify;
