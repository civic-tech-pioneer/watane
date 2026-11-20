//! The shell unit level of TECHNICAL 12.1: one `sh` loads the functions of `install.sh` and runs
//! many cases of one rule, each in its own subshell, so a rule costs one process, not one install.

use std::fs;
use std::process::Command;

use super::{Fx, LAUNCH, PERL, baked_script, exec};

/// What one case printed and how it ended.
#[derive(Debug)]
pub struct Unit {
    pub name: String,
    pub out: String,
    pub err: String,
    pub code: i32,
}

impl Unit {
    /// The first line of stderr.
    pub fn first_err(&self) -> &str {
        self.err.lines().next().unwrap_or("")
    }
    pub fn has_out_line(&self, line: &str) -> bool {
        self.out.lines().any(|l| l == line)
    }
    pub fn has_err_line(&self, line: &str) -> bool {
        self.err.lines().any(|l| l == line)
    }
}

/// The functions of `install.sh` for `version`: the script without its last line `main "$@"`,
/// which the test asserts is there (as `baked_script` asserts `@RELEASE@`).
pub fn functions_of_script(version: &str) -> String {
    let s = baked_script(version);
    let body = s
        .strip_suffix("main \"$@\"\n")
        .expect("install.sh ends with the line `main \"$@\"`");
    body.to_string()
}

impl Fx {
    /// Runs the cases against the functions of `install.sh` in one `/bin/sh` with this world's
    /// environment (cleared, `HOME`, the fakes first on the `PATH`), after `prelude`
    /// (for example `check_home; set_entries`), both under `set -eu` as `install.sh` sets them. Each
    /// case is `(name, shell text)` and runs in a subshell that sets `set -eu` itself, so `die`'s
    /// `exit` and a command that `set -e` ends end only the case (its status is `code`, its output
    /// so far stays its own), and its variables are gone after it. A case that calls a function
    /// the script calls in `||` or `if` (where `set -e` is off) does the same, as `if f; then
    /// rc=0; else rc=$?; fi`; one that needs `set +e` says it in its own text. `$ROOT` is
    /// the folder of the world; `extra` adds environment variables.
    pub fn units(
        &self,
        prelude: &str,
        extra: &[(&str, &str)],
        cases: &[(&str, &str)],
    ) -> Vec<Unit> {
        let functions = self.root.path().join("functions.sh");
        fs::write(&functions, functions_of_script("0.1.0")).expect("write functions");
        let mut prog = format!(". '{}'\nset -eu\n{prelude}\n", functions.display());
        for (name, body) in cases {
            assert!(
                name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "a case name is a word: {name}"
            );
            prog.push_str(&format!(
                "printf '@@BEGIN {name}\\n'; printf '@@BEGIN {name}\\n' >&2\nset +e\n(\nset -eu\n{body}\n)\nunit_rc=$?\nprintf '\\n@@END %s\\n' \"$unit_rc\"; printf '\\n@@END\\n' >&2\n"
            ));
        }
        let file = self.root.path().join("units.sh");
        fs::write(&file, prog).expect("write units");
        // as a run: in a new session, so `/dev/tty` cannot be opened, with the signals at their defaults
        let mut c = Command::new(PERL);
        c.args(LAUNCH).arg("/bin/sh").arg(&file);
        c.env_clear().envs(self.env_pairs());
        c.env("ROOT", self.root.path());
        for (k, v) in extra {
            c.env(k, v);
        }
        let out = exec(c);
        cases
            .iter()
            .map(|(name, _)| Unit {
                name: (*name).to_string(),
                out: between(&out.stdout, name, "\n@@END "),
                err: between(&out.stderr, name, "\n@@END\n"),
                code: end_code(&out.stdout, name),
            })
            .collect()
    }

    /// `units` with the prelude every rule that reads the home needs.
    pub fn units_in_home(&self, cases: &[(&str, &str)]) -> Vec<Unit> {
        self.units("check_home; set_entries", &[], cases)
    }
}

fn between(text: &str, name: &str, end: &str) -> String {
    let begin = format!("@@BEGIN {name}\n");
    let from = text
        .find(&begin)
        .unwrap_or_else(|| panic!("case {name} never started:\n{text}"))
        + begin.len();
    let to = text[from..]
        .find(end)
        .unwrap_or_else(|| panic!("case {name} never ended:\n{text}"));
    text[from..from + to].to_string()
}

fn end_code(stdout: &str, name: &str) -> i32 {
    let rest = &stdout[stdout.find(&format!("@@BEGIN {name}\n")).expect("begin")..];
    let at = rest.find("\n@@END ").expect("end") + "\n@@END ".len();
    rest[at..]
        .lines()
        .next()
        .expect("code")
        .parse()
        .expect("code is a number")
}

/// Looks a case up by name.
pub fn case<'a>(units: &'a [Unit], name: &str) -> &'a Unit {
    units
        .iter()
        .find(|u| u.name == name)
        .unwrap_or_else(|| panic!("no case {name}"))
}
