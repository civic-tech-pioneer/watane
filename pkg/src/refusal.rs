//! The causes and next steps `Mod::read` and `pack` both give for a link or a special file.
/// A link where a file or a directory may stand.
pub(crate) const LINK_ENTRY: &str = "a symbolic link; replace it with a regular file or directory";
/// A link where only a file may stand (mod.toml, deps.json, logic.wasm).
pub(crate) const LINK_FILE: &str = "a symbolic link; replace it with a regular file";
/// A FIFO, socket or device.
pub(crate) const SPECIAL: &str =
    "not a regular file or a directory (a FIFO, socket or device); replace or remove it";
/// An entry that was listed and then was gone or no longer a directory when opened.
pub(crate) const CHANGED: &str = "changed while the mod was read; read it again";
