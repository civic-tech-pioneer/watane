//! The launch document of `claude-code.start` and `claude-code-shell.start`: parsed by hand
//! into a typed value, each field checked against its grammar, each value turned into one
//! `--flag=value` argument (TECHNICAL 19.3, The launch document).

use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// `bad-launch {field, reason}` (19.8).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BadLaunch {
    pub field: String,
    pub reason: String,
}
fn bad<T>(field: &str, reason: impl Into<String>) -> Result<T, BadLaunch> {
    Err(BadLaunch {
        field: field.into(),
        reason: reason.into(),
    })
}

/// Which grant the start came through (19.3, Grants and tools).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Code,
    Shell,
}

const CODE_TOOLS: [&str; 9] = [
    "Read",
    "Write",
    "Edit",
    "Glob",
    "Grep",
    "NotebookEdit",
    "Skill",
    "Task",
    "TodoWrite",
];
const SHELL_TOOLS: [&str; 3] = ["Bash", "WebFetch", "WebSearch"];
/// The file tools whose `allow` rules must be relative patterns (Protected reads).
const FILE_TOOLS: [&str; 6] = ["Read", "Glob", "Grep", "Edit", "Write", "NotebookEdit"];
const FIELDS: [&str; 22] = [
    "version",
    "workspace",
    "extra-workspaces",
    "model",
    "fallback-models",
    "effort",
    "system-prompt",
    "tools",
    "allow",
    "deny",
    "mode",
    "ask",
    "pre-tool-check",
    "agent-handle",
    "subagents",
    "subagent-depth",
    "skills",
    "sandbox",
    "json-schema",
    "max-turns",
    "max-budget-usd",
    "session",
];
const FIELDS_2: [&str; 3] = ["persist", "partial", "profile"];

const STAGE_SKILLS: &str = "skills wait for a later slice of stage A2";

/// A workspace named by the launch: a new scratch one, one that the origin client holds, or a
/// picked folder (an absolute path; the call resolves it, `resolve_picked`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Workspace {
    NewScratch,
    Scratch(String),
    Picked(String),
}

/// The `session` of a launch (19.3, Sessions are the origin client's).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SessionUse {
    New(String),
    Resume(String),
    Fork(String),
}
impl SessionUse {
    /// `(id, must the origin client hold it)`.
    pub(crate) fn id(&self) -> (&str, bool) {
        match self {
            Self::New(id) => (id, false),
            Self::Resume(id) | Self::Fork(id) => (id, true),
        }
    }
}

/// A parsed launch document (19.3). Its flags are in the order of the argument list.
#[derive(Debug, Clone)]
pub(crate) struct Launch {
    pub workspace: Workspace,
    /// The scratch ids of `extra-workspaces`.
    pub extra: Vec<String>,
    /// The picked folders of `extra-workspaces`, in launch order (canonical after the call).
    pub extra_picked: Vec<String>,
    /// `profile`: only the words of the workspace consent (19.3).
    pub profile: Option<String>,
    pub session: Option<SessionUse>,
    /// `--flag=value` arguments from the launch values, in order, without `--add-dir=`,
    /// `--settings=` and `--plugin-dir=` (the run adds those).
    pub flags: Vec<String>,
    /// The built-in tools that `claude` is given (`Task` already removed at depth 0).
    pub tools: Vec<String>,
    /// The names of the launch's subagents.
    pub agents: Vec<String>,
    /// The permission mode as `system/init` names it.
    pub init_mode: &'static str,
    pub depth: u8,
    /// `ask: true`: `claude` sends `can_use_tool` requests, which become `ask` items.
    pub ask: bool,
    /// `pre-tool-check {timeout-s}`: the hook's timeout in seconds; its callback gives `check` items.
    pub check: Option<u64>,
    /// `agent-handle`: a handle (19.5) whose MCP server the run hands `claude` on fd 3. Whether
    /// the origin client holds it is the call's check.
    pub agent_handle: Option<String>,
}

fn is_model(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'[' | b']'))
}
fn is_slug(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn is_profile(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}
fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && p.bytes().all(|b| b.is_ascii_hexdigit()))
}
/// 32 lowercase hex characters.
pub(crate) fn is_scratch_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn string<'a>(field: &str, v: &'a Value) -> Result<&'a str, BadLaunch> {
    v.as_str()
        .map_or_else(|| bad(field, "expected a string"), Ok)
}
fn boolean(field: &str, v: &Value) -> Result<bool, BadLaunch> {
    v.as_bool()
        .map_or_else(|| bad(field, "expected true or false"), Ok)
}
fn strings(field: &str, v: &Value) -> Result<Vec<String>, BadLaunch> {
    let Some(items) = v.as_array() else {
        return bad(field, "expected a list of strings");
    };
    items
        .iter()
        .map(|i| string(field, i).map(str::to_owned))
        .collect()
}
fn model(field: &str, v: &Value) -> Result<String, BadLaunch> {
    let s = string(field, v)?;
    if is_model(s) {
        Ok(s.to_owned())
    } else {
        bad(
            field,
            "use 1 to 64 of letters, digits, `.`, `-`, `[` and `]`",
        )
    }
}
fn workspace_id(field: &str, v: &Value, scratch_ok: bool) -> Result<Workspace, BadLaunch> {
    let s = string(field, v)?;
    if s == "scratch" {
        return if scratch_ok {
            Ok(Workspace::NewScratch)
        } else {
            bad(
                field,
                "`scratch` makes a new workspace: name an existing one here",
            )
        };
    }
    match s.strip_prefix("scratch:") {
        Some(id) if is_scratch_id(id) => Ok(Workspace::Scratch(id.to_owned())),
        Some(_) => bad(
            field,
            "a scratch id is `scratch:` and 32 lowercase hex characters",
        ),
        None if s.starts_with('/') => Ok(Workspace::Picked(s.to_owned())),
        None => bad(
            field,
            "use an absolute path, `scratch` or `scratch:<id>`: a picked folder is absolute",
        ),
    }
}

/// The tool names a grant allows.
fn allowed_names(kind: Kind) -> Vec<&'static str> {
    let mut names = CODE_TOOLS.to_vec();
    if kind == Kind::Shell {
        names.extend(SHELL_TOOLS);
    }
    names
}

/// `Name` or `Name(arg)`, split; the checks of the table (19.3).
fn rule<'a>(
    field: &str,
    text: &'a str,
    kind: Kind,
) -> Result<(&'a str, Option<&'a str>), BadLaunch> {
    if text.chars().any(char::is_control) {
        return bad(field, "a rule holds a control character");
    }
    let (name, arg) = match text.find('(') {
        None => (text, None),
        Some(open) => {
            let Some(inner) = text[open + 1..].strip_suffix(')') else {
                return bad(field, "a rule with an argument ends with `)`");
            };
            if inner.contains(['(', ')']) {
                return bad(field, "an argument holds no parentheses");
            }
            (&text[..open], Some(inner))
        }
    };
    if name.is_empty() || name.contains([',', ' ', '(', ')']) || name.starts_with('-') {
        return bad(field, "a rule name has no comma, space or leading `-`");
    }
    let mcp = name.strip_prefix("mcp__watane__").is_some_and(|t| {
        !t.is_empty()
            && t.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    });
    if !mcp && !allowed_names(kind).contains(&name) {
        return bad(field, format!("`{name}` is not a tool of this grant"));
    }
    Ok((name, arg))
}

fn relative_pattern(arg: &str) -> bool {
    !(arg.starts_with('/') || arg.starts_with('~') || arg.split('/').any(|s| s == ".."))
}

/// Interpreters and runners that an allow `Bash` rule may not start with (19.3, the Bash grammar).
const BASH_DENIED: &[&str] = &[
    ".",
    "ash",
    "at",
    "awk",
    "bash",
    "batch",
    "builtin",
    "bun",
    "bunx",
    "busybox",
    "byobu",
    "capsh",
    "chroot",
    "chrt",
    "command",
    "coproc",
    "corepack",
    "crontab",
    "csh",
    "dash",
    "deno",
    "doas",
    "dtach",
    "elixir",
    "enable",
    "env",
    "eval",
    "exec",
    "expect",
    "fakeroot",
    "fc",
    "fish",
    "flock",
    "gawk",
    "gdb",
    "ionice",
    "irb",
    "julia",
    "ksh",
    "lldb",
    "ltrace",
    "lua",
    "mawk",
    "mksh",
    "nawk",
    "nice",
    "node",
    "nohup",
    "npx",
    "nsenter",
    "numactl",
    "osascript",
    "parallel",
    "perl",
    "php",
    "pipx",
    "pkexec",
    "pnpx",
    "prlimit",
    "pwsh",
    "r",
    "rbash",
    "rscript",
    "ruby",
    "run0",
    "runuser",
    "screen",
    "script",
    "sem",
    "setpriv",
    "setsid",
    "sg",
    "sh",
    "source",
    "ssh",
    "stdbuf",
    "strace",
    "su",
    "sudo",
    "systemd-run",
    "taskset",
    "tclsh",
    "tcsh",
    "time",
    "timeout",
    "tmux",
    "toybox",
    "trap",
    "ts-node",
    "tsx",
    "unshare",
    "uvx",
    "valgrind",
    "watch",
    "xargs",
    "yarnpkg",
    "zellij",
    "zsh",
];
const BASH_PREFIXES: [&str; 7] = ["python", "pypy", "node", "perl", "ruby", "php", "lua"];
const RUNNERS: [(&str, &[&str]); 5] = [
    (
        "npm",
        &[
            "install", "i", "ci", "test", "ls", "list", "audit", "outdated",
        ],
    ),
    (
        "pnpm",
        &["install", "add", "test", "list", "ls", "audit", "outdated"],
    ),
    (
        "yarn",
        &["install", "add", "test", "list", "audit", "outdated"],
    ),
    (
        "uv",
        &["pip", "sync", "lock", "add", "remove", "tree", "venv"],
    ),
    ("devbox", &["install", "add"]),
];

/// The allow grammar for `Bash` (19.3). Deny rules are not held to it.
fn bash_allow(arg: Option<&str>) -> Result<(), String> {
    let Some(arg) = arg else {
        return Err("bare `Bash` allows any command".into());
    };
    let arg = arg.trim();
    if arg.is_empty() || arg == ":*" {
        return Err("an empty command allows any command".into());
    }
    if !arg.is_ascii()
        || arg.chars().any(|c| {
            matches!(
                c,
                '`' | '$' | '{' | '}' | ';' | '|' | '&' | '<' | '>' | '\'' | '"' | '\\'
            )
        })
    {
        return Err("a command has no non-ASCII character and none of the shell's".into());
    }
    let body = match arg.find(':') {
        None => arg,
        Some(i) if &arg[i..] == ":*" => &arg[..i],
        Some(_) => return Err("a colon appears only as one trailing `:*`".into()),
    };
    let words: Vec<&str> = body.split_whitespace().collect();
    let Some(first) = words.first() else {
        return Err("an empty command allows any command".into());
    };
    if first.contains('/')
        || first.contains('~')
        || !first
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'))
    {
        return Err("the first word is a plain program name".into());
    }
    let lower = first.to_ascii_lowercase();
    if BASH_PREFIXES.iter().any(|p| lower.starts_with(p)) || BASH_DENIED.contains(&lower.as_str()) {
        return Err(format!(
            "`{first}` runs, wraps or schedules another command"
        ));
    }
    if lower == "find" {
        const BAD: [&str; 4] = ["-exec", "-execdir", "-ok", "-okdir"];
        if arg.contains('*') || words.iter().any(|w| BAD.contains(w)) {
            return Err(
                "`find` is allowed with no `*` and no -exec, -execdir, -ok or -okdir".into(),
            );
        }
    }
    if let Some((_, seconds)) = RUNNERS.iter().find(|(name, _)| *name == lower) {
        let ok = words.get(1).is_some_and(|w| seconds.contains(w))
            && !words[2..].iter().any(|w| w.starts_with('-'))
            && !arg.contains('*');
        if !ok {
            return Err(format!(
                "`{first}` needs a second word from its list, no option and no `*`"
            ));
        }
    }
    Ok(())
}

fn rules(field: &str, v: &Value, kind: Kind, allow: bool) -> Result<Vec<String>, BadLaunch> {
    let list = strings(field, v)?;
    for text in &list {
        let (name, arg) = rule(field, text, kind)?;
        if !allow {
            continue;
        }
        if FILE_TOOLS.contains(&name) {
            match arg {
                None => {
                    return bad(
                        field,
                        format!(
                            "`{name}` with no argument allows every path: use `{name}(**)` for the workspace"
                        ),
                    );
                }
                Some(a) if a.is_empty() || a == "*" => {
                    return bad(
                        field,
                        format!(
                            "`{name}({a})` is read as the bare `{name}`, which allows every path: use `{name}(**)` for the workspace"
                        ),
                    );
                }
                Some(a) if !relative_pattern(a) => {
                    return bad(
                        field,
                        format!("the pattern of `{name}` is relative to the workspace"),
                    );
                }
                Some(_) => {}
            }
        }
        if name == "Bash"
            && let Err(reason) = bash_allow(arg)
        {
            return bad(field, reason);
        }
    }
    Ok(list)
}

/// `{timeout-s: 5..120}` and nothing else (19.3, The launch document).
fn pre_tool_check(v: &Value) -> Result<u64, BadLaunch> {
    let field = "pre-tool-check";
    let Some(o) = v.as_object() else {
        return bad(field, "expected {timeout-s: 5 to 120}");
    };
    if let Some(extra) = o.keys().find(|k| *k != "timeout-s") {
        return bad(field, format!("`{extra}` is not a key of the check"));
    }
    match o.get("timeout-s").and_then(Value::as_u64) {
        Some(n @ 5..=120) => Ok(n),
        _ => bad(field, "`timeout-s` is a whole number of seconds, 5 to 120"),
    }
}

fn subagents(
    v: &Value,
    tools: &[String],
    flags: &mut Vec<String>,
) -> Result<Vec<String>, BadLaunch> {
    const KEYS: [&str; 5] = ["name", "description", "prompt", "tools", "model"];
    let Some(list) = v.as_array() else {
        return bad("subagents", "expected a list");
    };
    let mut map = Map::new();
    let mut names = vec![];
    for (i, agent) in list.iter().enumerate() {
        let Some(o) = agent.as_object() else {
            return bad("subagents", format!("subagent {i} is not an object"));
        };
        if let Some(extra) = o.keys().find(|k| !KEYS.contains(&k.as_str())) {
            return bad(
                "subagents",
                format!("subagent {i} has the key `{extra}`: refused"),
            );
        }
        for key in KEYS {
            if !o.contains_key(key) {
                return bad("subagents", format!("subagent {i} lacks `{key}`"));
            }
        }
        let name = string("subagents", &o["name"])?;
        if !is_slug(name) {
            return bad(
                "subagents",
                format!("subagent {i}: name is 1 to 64 of a-z, 0-9, `-`"),
            );
        }
        string("subagents", &o["description"])?;
        string("subagents", &o["prompt"])?;
        let agent_tools = strings("subagents", &o["tools"])?;
        if let Some(t) = agent_tools.iter().find(|t| !tools.contains(t)) {
            return bad(
                "subagents",
                format!("subagent {i} names the tool `{t}`, which the launch's tools lack"),
            );
        }
        model("subagents", &o["model"])?;
        if map.contains_key(name) {
            return bad("subagents", format!("two subagents are named `{name}`"));
        }
        names.push(name.to_owned());
        let mut body = o.clone();
        body.remove("name");
        map.insert(name.to_owned(), Value::Object(body));
    }
    let text = Value::Object(map).to_string();
    if text.len() > 64 << 10 {
        return bad("subagents", "64 KiB in all");
    }
    if !names.is_empty() {
        flags.push(format!("--agents={text}"));
    }
    Ok(names)
}

fn session(v: &Value, flags: &mut Vec<String>) -> Result<SessionUse, BadLaunch> {
    let Some(o) = v.as_object() else {
        return bad("session", "expected an object");
    };
    let uuid = |key: &str| -> Result<Option<String>, BadLaunch> {
        o.get(key)
            .map(|v| {
                let s = string("session", v)?;
                if is_uuid(s) {
                    Ok(s.to_owned())
                } else {
                    bad("session", format!("`{key}` is a uuid"))
                }
            })
            .transpose()
    };
    let which: Vec<&str> = ["new", "resume", "fork"]
        .into_iter()
        .filter(|k| o.contains_key(*k))
        .collect();
    let [kind] = which.as_slice() else {
        return bad("session", "use exactly one of `new`, `resume` and `fork`");
    };
    let allowed: &[&str] = match *kind {
        "new" => &["new"],
        "resume" => &["resume", "at", "drops-turn"],
        _ => &["fork", "at"],
    };
    if let Some(k) = o.keys().find(|k| !allowed.contains(&k.as_str())) {
        return bad("session", format!("`{k}` does not go with `{kind}`"));
    }
    let main = uuid(kind)?.expect("the key is present");
    let used = match *kind {
        "new" => {
            flags.push(format!("--session-id={main}"));
            SessionUse::New(main)
        }
        "resume" => {
            flags.push(format!("--resume={main}"));
            SessionUse::Resume(main)
        }
        _ => {
            flags.push(format!("--resume={main}"));
            flags.push("--fork-session".into());
            SessionUse::Fork(main)
        }
    };
    if let Some(at) = uuid("at")? {
        flags.push(format!("--resume-session-at={at}"));
    }
    if let Some(drops) = uuid("drops-turn")? {
        flags.push(format!("--resume-drops-turn={drops}"));
    }
    Ok(used)
}

/// A shallow check that a value can be a JSON Schema: a boolean or an object whose `type`
/// is a type name or a list of them.
fn json_schema(v: &Value) -> Result<(), BadLaunch> {
    const TYPES: [&str; 7] = [
        "null", "boolean", "object", "array", "number", "string", "integer",
    ];
    let ok = match v {
        Value::Bool(_) => true,
        Value::Object(o) => match o.get("type") {
            None => true,
            Some(Value::String(t)) => TYPES.contains(&t.as_str()),
            Some(Value::Array(a)) => a
                .iter()
                .all(|t| t.as_str().is_some_and(|t| TYPES.contains(&t))),
            Some(_) => false,
        },
        _ => false,
    };
    if !ok {
        return bad("json-schema", "expected a JSON Schema");
    }
    if v.to_string().len() > 64 << 10 {
        return bad("json-schema", "at most 64 KiB");
    }
    Ok(())
}

/// The folders under the participant's home that hold credentials (19.3, Picked workspace).
const CREDENTIAL_DIRS: [&str; 9] = [
    ".claude",
    ".codex",
    ".ssh",
    ".aws",
    ".gnupg",
    ".config/gh",
    ".docker",
    ".kube",
    "Library/Keychains",
];

/// Where a picked folder may not be (19.3, Picked workspace): `/`, the participant's home and
/// the watane home with their ancestors, and any folder inside the folders that hold secrets or
/// the watane home.
pub(crate) struct Forbidden {
    exact: Vec<PathBuf>,
    inside: Vec<PathBuf>,
}
impl Forbidden {
    pub(crate) fn new(watane_home: &Path, user_home: Option<&Path>) -> Self {
        let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let watane = canon(watane_home);
        let mut exact = vec![PathBuf::from("/")];
        exact.extend(watane.ancestors().map(Path::to_path_buf));
        let mut inside = vec![watane];
        if let Some(home) = user_home {
            let home = canon(home);
            exact.push(home.clone());
            for dir in CREDENTIAL_DIRS {
                let named = home.join(dir);
                // Both spellings: as named under the home, and where a link there leads.
                inside.push(canon(&named));
                inside.push(named);
            }
        }
        Self { exact, inside }
    }
    fn refuses(&self, folder: &Path) -> bool {
        // On a case-insensitive file system names are compared without case.
        let fold = |p: &Path| -> PathBuf {
            if cfg!(any(target_os = "macos", target_os = "windows")) {
                PathBuf::from(p.to_string_lossy().to_lowercase())
            } else {
                p.to_path_buf()
            }
        };
        let folder = fold(folder);
        self.exact.iter().any(|p| fold(p) == folder)
            || self.inside.iter().any(|p| folder.starts_with(fold(p)))
    }
}

/// Resolves a picked folder at the call (19.3, Picked folders): absolute, canonical with symlinks
/// resolved, an existing folder, in Unicode NFC, and not one that `Forbidden` names. The reason
/// goes to `bad-launch {field}`.
pub(crate) fn resolve_picked(raw: &str, forbidden: &Forbidden) -> Result<String, String> {
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err("a picked folder is an absolute path".into());
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("the folder cannot be resolved: {e}"))?;
    if !canonical.is_dir() {
        return Err("the path is not a folder".into());
    }
    let Some(text) = canonical.to_str() else {
        return Err("the folder's name is not UTF-8".into());
    };
    if !unicode_normalization::is_nfc(text) {
        return Err("the folder's canonical name is not in Unicode NFC".into());
    }
    if text.chars().any(char::is_control) {
        return Err("the folder's name holds a control character".into());
    }
    if forbidden.refuses(&canonical) {
        return Err(
            "this folder holds the watane home, the participant's home or credentials, or lies inside one"
                .into(),
        );
    }
    Ok(text.to_owned())
}

impl Launch {
    /// The picked folders of the launch, in launch order: the working folder, then the extras.
    pub(crate) fn picked(&self) -> Vec<String> {
        let mut all = vec![];
        if let Workspace::Picked(p) = &self.workspace {
            all.push(p.clone());
        }
        for p in &self.extra_picked {
            if !all.contains(p) {
                all.push(p.clone());
            }
        }
        all
    }

    /// Parses and checks the document of a start (19.3). Whether a scratch id is held by the
    /// origin client is the caller's check.
    pub(crate) fn parse(text: &str, kind: Kind) -> Result<Self, BadLaunch> {
        let value: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => return bad("launch", format!("not JSON: {e}")),
        };
        let Some(doc) = value.as_object() else {
            return bad("launch", "expected a JSON object");
        };
        if let Some(unknown) = doc
            .keys()
            .find(|k| !FIELDS.contains(&k.as_str()) && !FIELDS_2.contains(&k.as_str()))
        {
            return bad(unknown, "unknown field");
        }
        if doc.get("version").and_then(Value::as_u64) != Some(1) {
            return bad("version", "must be 1");
        }
        let Some(ws) = doc.get("workspace") else {
            return bad("workspace", "required");
        };
        let workspace = workspace_id("workspace", ws, true)?;
        let mut extra = vec![];
        let mut extra_picked: Vec<String> = vec![];
        if let Some(v) = doc.get("extra-workspaces") {
            let Some(list) = v.as_array() else {
                return bad("extra-workspaces", "expected a list");
            };
            for item in list {
                match workspace_id("extra-workspaces", item, false)? {
                    Workspace::Scratch(id) => extra.push(id),
                    Workspace::Picked(path) => {
                        if !extra_picked.contains(&path) {
                            extra_picked.push(path);
                        }
                    }
                    Workspace::NewScratch => unreachable!("refused above"),
                }
            }
        }
        let mut flags = vec![];
        if let Some(v) = doc.get("model") {
            flags.push(format!("--model={}", model("model", v)?));
        }
        if let Some(v) = doc.get("fallback-models") {
            let Some(list) = v.as_array() else {
                return bad("fallback-models", "expected a list");
            };
            if list.len() > 4 {
                return bad("fallback-models", "at most 4");
            }
            for item in list {
                flags.push(format!(
                    "--fallback-model={}",
                    model("fallback-models", item)?
                ));
            }
        }
        if let Some(v) = doc.get("effort") {
            let s = string("effort", v)?;
            if !["low", "medium", "high", "xhigh", "max"].contains(&s) {
                return bad("effort", "use low, medium, high, xhigh or max");
            }
            flags.push(format!("--effort={s}"));
        }
        if let Some(v) = doc.get("system-prompt") {
            let Some(o) = v.as_object() else {
                return bad(
                    "system-prompt",
                    "expected {replace: text} or {append: text}",
                );
            };
            let [(key, text)] = o.iter().collect::<Vec<_>>()[..] else {
                return bad("system-prompt", "use exactly one of `replace` and `append`");
            };
            let text = string("system-prompt", text)?;
            if text.len() > 64 << 10 {
                return bad("system-prompt", "at most 64 KiB");
            }
            match key.as_str() {
                "replace" => flags.push(format!("--system-prompt={text}")),
                "append" => flags.push(format!("--append-system-prompt={text}")),
                _ => return bad("system-prompt", "use `replace` or `append`"),
            }
        }
        let depth = match doc.get("subagent-depth") {
            None => 1,
            Some(v) => match v.as_u64() {
                Some(d @ 0..=3) => d as u8,
                _ => return bad("subagent-depth", "use 0 to 3"),
            },
        };
        let allowed = allowed_names(kind);
        let mut tools: Vec<String> = vec![];
        if let Some(v) = doc.get("tools") {
            for t in strings("tools", v)? {
                if !allowed.contains(&t.as_str()) {
                    return bad("tools", format!("`{t}` is not a tool of this grant"));
                }
                if !tools.contains(&t) {
                    tools.push(t);
                }
            }
        }
        if depth == 0 {
            tools.retain(|t| t != "Task");
        }
        // `--tools=` always goes out: without it `claude` would have every built-in tool.
        flags.push(format!("--tools={}", tools.join(",")));
        if let Some(v) = doc.get("allow") {
            for r in rules("allow", v, kind, true)? {
                flags.push(format!("--allowedTools={r}"));
            }
        }
        if let Some(v) = doc.get("deny") {
            for r in rules("deny", v, kind, false)? {
                flags.push(format!("--disallowedTools={r}"));
            }
        }
        let mut init_mode = "default";
        if let Some(v) = doc.get("mode") {
            let (flag, init) = match string("mode", v)? {
                "manual" => ("default", "default"),
                "accept-edits" => ("acceptEdits", "acceptEdits"),
                "plan" => ("plan", "plan"),
                "dont-ask" => ("dontAsk", "dontAsk"),
                _ => return bad("mode", "use manual, accept-edits, plan or dont-ask"),
            };
            init_mode = init;
            flags.push(format!("--permission-mode={flag}"));
        }
        let ask = doc.get("ask").map(|v| boolean("ask", v)).transpose()? == Some(true);
        if ask {
            flags.push("--permission-prompts=host".into());
            flags.push("--permission-prompt-tool=stdio".into());
        } else {
            flags.push("--permission-prompts=none".into());
        }
        let check = doc.get("pre-tool-check").map(pre_tool_check).transpose()?;
        let agent_handle = match doc.get("agent-handle") {
            None => None,
            Some(v) => {
                let handle = string("agent-handle", v)?;
                if !is_scratch_id(handle) {
                    return bad(
                        "agent-handle",
                        "a handle is 32 lowercase hexadecimal characters",
                    );
                }
                Some(handle.to_owned())
            }
        };
        let agents = match doc.get("subagents") {
            Some(v) => subagents(v, &tools, &mut flags)?,
            None => vec![],
        };
        if let Some(v) = doc.get("skills") {
            match v.as_array() {
                Some(a) if a.is_empty() => {}
                Some(_) => return bad("skills", STAGE_SKILLS),
                None => return bad("skills", "expected a list"),
            }
        }
        match (doc.get("sandbox"), kind) {
            (Some(_), Kind::Code) => {
                return bad(
                    "sandbox",
                    "the sandbox record belongs to the shell grant, which waits on section 18, Q39 and Q42",
                );
            }
            (None, Kind::Shell) => return bad("sandbox", "required with the shell grant"),
            (Some(v), Kind::Shell) if !v.is_object() => {
                return bad("sandbox", "expected an object");
            }
            _ => {}
        }
        if let Some(v) = doc.get("json-schema") {
            json_schema(v)?;
            flags.push(format!("--json-schema={v}"));
        }
        if let Some(v) = doc.get("max-turns") {
            match v.as_u64() {
                Some(n @ 1..=1000) => flags.push(format!("--max-turns={n}")),
                _ => return bad("max-turns", "use 1 to 1000"),
            }
        }
        if let Some(v) = doc.get("max-budget-usd") {
            match v.as_f64() {
                Some(n) if n.is_finite() && n > 0.0 => {
                    flags.push(format!("--max-budget-usd={v}"));
                }
                _ => return bad("max-budget-usd", "use a positive number"),
            }
        }
        let session_use = doc
            .get("session")
            .map(|v| session(v, &mut flags))
            .transpose()?;
        if let Some(v) = doc.get("persist")
            && !boolean("persist", v)?
        {
            flags.push("--no-session-persistence".into());
        }
        if let Some(v) = doc.get("partial")
            && boolean("partial", v)?
        {
            flags.push("--include-partial-messages".into());
        }
        let mut profile = None;
        if let Some(v) = doc.get("profile") {
            let s = string("profile", v)?;
            if !is_profile(s) {
                return bad("profile", "use 1 to 64 of letters, digits, `.`, `_`, `-`");
            }
            profile = Some(s.to_owned());
        }
        let extra: Vec<String> = extra
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(Self {
            workspace,
            extra,
            extra_picked,
            profile,
            session: session_use,
            flags,
            tools,
            agents,
            init_mode,
            depth,
            ask,
            check,
            agent_handle,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 19.3: `agent-handle` is 32 lowercase hex characters; nothing else parses.
    #[test]
    fn ac5_an_agent_handle_is_32_lowercase_hex_characters() {
        let parse = |v: Value| {
            let doc = json!({"version": 1, "workspace": "scratch", "agent-handle": v});
            Launch::parse(&doc.to_string(), Kind::Code)
        };
        let ok = parse(json!("0123456789abcdef0123456789abcdef")).unwrap();
        assert_eq!(
            ok.agent_handle.as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        for v in [
            json!("h"),
            json!(""),
            json!("0123456789ABCDEF0123456789abcdef"),
            json!("0123456789abcdef0123456789abcde"),
            json!("0123456789abcdef0123456789abcdef0"),
            json!(7),
            json!(null),
        ] {
            let e = parse(v.clone()).unwrap_err();
            assert_eq!(e.field, "agent-handle", "{v}");
            assert!(!e.reason.contains("stage"), "{v}");
        }
    }

    /// AC2 (19.3, Picked workspace): the folders that hold credentials, and what lies inside them.
    #[test]
    fn ac2_credential_folders_and_the_homes_are_refused_and_ordinary_ones_pass() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let (user, watane) = (base.join("user"), base.join("user/watane-home"));
        for dir in [
            ".ssh/keys",
            ".claude",
            ".aws",
            "code/app",
            "Library/Keychains",
        ] {
            std::fs::create_dir_all(user.join(dir)).unwrap();
        }
        std::fs::create_dir_all(watane.join("mods")).unwrap();
        let forbidden = Forbidden::new(&watane, Some(&user));
        let refused = |p: PathBuf| resolve_picked(p.to_str().unwrap(), &forbidden).is_err();
        assert!(refused(user.clone()));
        assert!(refused(base.clone()));
        assert!(refused(PathBuf::from("/")));
        assert!(refused(watane.clone()));
        assert!(refused(watane.join("mods")));
        for dir in [".ssh", ".ssh/keys", ".claude", ".aws", "Library/Keychains"] {
            assert!(refused(user.join(dir)), "{dir}");
        }
        let ok = resolve_picked(user.join("code/app").to_str().unwrap(), &forbidden).unwrap();
        assert_eq!(ok, user.join("code/app").to_str().unwrap());
        resolve_picked("relative", &forbidden).unwrap_err();
        let control = user.join("code/a\u{7}b");
        std::fs::create_dir(&control).unwrap();
        let e = resolve_picked(control.to_str().unwrap(), &forbidden).unwrap_err();
        assert_eq!(e, "the folder's name holds a control character");
    }

    /// S103 fix1 AC2 (19.3, Picked workspace): the added credential folders, and a linked one in
    /// both spellings.
    #[test]
    fn ac2_s103_fix1_more_credential_folders_and_both_spellings_are_refused() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let (user, watane) = (base.join("user"), base.join("user/watane-home"));
        std::fs::create_dir_all(watane.join("mods")).unwrap();
        for dir in [
            ".gnupg/k",
            ".config/gh/h",
            ".docker/c",
            ".kube/c",
            "dotfiles/claude/sub",
        ] {
            std::fs::create_dir_all(user.join(dir)).unwrap();
        }
        std::os::unix::fs::symlink(user.join("dotfiles/claude"), user.join(".claude")).unwrap();
        let forbidden = Forbidden::new(&watane, Some(&user));
        for dir in [
            ".gnupg",
            ".gnupg/k",
            ".config/gh",
            ".config/gh/h",
            ".docker",
            ".docker/c",
            ".kube",
            ".kube/c",
            ".claude",
            ".claude/sub",
            "dotfiles/claude",
            "dotfiles/claude/sub",
        ] {
            let got = resolve_picked(user.join(dir).to_str().unwrap(), &forbidden);
            assert!(
                got.as_ref().is_err_and(|e| e.contains("credentials")),
                "{dir} was not refused as a credential folder: {got:?}"
            );
        }
        // The folder beside them is no credential folder.
        let dotfiles = user.join("dotfiles");
        resolve_picked(dotfiles.to_str().unwrap(), &forbidden).unwrap();
    }

    /// Adversary S103 (19.3, Picked workspace): a credential folder kept elsewhere and linked
    /// from the participant's home (a dotfiles manager does this) is still `~/.ssh` or
    /// `~/.claude`. Its canonical path must be refused, and so must any folder inside it; else a
    /// mod asks the consent for a path that does not look like a credential folder.
    #[test]
    fn adv_s103_a_symlinked_credential_folder_is_refused() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let (user, watane) = (base.join("user"), base.join("user/watane-home"));
        std::fs::create_dir_all(watane.join("mods")).unwrap();
        let dotfiles = user.join("dotfiles");
        for dir in ["ssh/keys", "claude", "aws"] {
            std::fs::create_dir_all(dotfiles.join(dir)).unwrap();
        }
        for (link, target) in [(".ssh", "ssh"), (".claude", "claude"), (".aws", "aws")] {
            std::os::unix::fs::symlink(dotfiles.join(target), user.join(link)).unwrap();
        }
        let forbidden = Forbidden::new(&watane, Some(&user));
        for dir in [".ssh", ".ssh/keys", ".claude", ".aws"] {
            let got = resolve_picked(user.join(dir).to_str().unwrap(), &forbidden);
            assert!(got.is_err(), "{dir} was accepted as {got:?}");
        }
    }
}
