//! `watane-dev <args>` run in-process for tests of JSON, codes and messages (TECHNICAL 2.5).
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::Value;
use std::ffi::{OsStr, OsString};
use std::path::Path;

/// The exit status and JSON result of `watane-dev <args>` run in `cwd` with
/// `vars` set (`Some`) or unset (`None`) over the test's environment.
pub fn dev<A: AsRef<OsStr>>(
    cwd: &Path,
    args: &[A],
    vars: &[(&str, Option<&OsStr>)],
) -> (i32, Value) {
    let args: Vec<OsString> = std::iter::once(OsStr::new("watane-dev"))
        .chain(args.iter().map(AsRef::as_ref))
        .map(OsStr::to_os_string)
        .collect();
    let mut out = Vec::new();
    let code = watane_dev::cli::main_with(&args, vars, Some(cwd), &mut out);
    let value = serde_json::from_slice(&out)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out)));
    (i32::from(code), value)
}
