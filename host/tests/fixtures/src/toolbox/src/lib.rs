use watane::host::agent_tools::{self, ToolsError};
use watane::host::streams::{Item, Outcome, SinkOutcome};
use watane::host::claude_types::StartError;
use watane::host::{caller, claude_code, state};

wit_bindgen::generate!({ path: "../../mods/toolbox/wit", world: "toolbox", generate_all });
struct Guest;

fn get(key: &str) -> Option<String> {
    state::get(key).map(|v| String::from_utf8(v).unwrap())
}
fn lines(key: &str) -> Vec<String> {
    get(key)
        .map(|v| v.split('\n').map(str::to_owned).collect())
        .unwrap_or_default()
}
fn add_line(key: &str, line: String) {
    let mut all = lines(key);
    all.push(line);
    state::put(key, all.join("\n").as_bytes());
}
fn error(e: ToolsError) -> String {
    match e {
        ToolsError::NotGranted => "not-granted".into(),
        ToolsError::UnknownHandle => "unknown-handle".into(),
        ToolsError::UnknownTool => "unknown-tool".into(),
        ToolsError::ConsentPending(w) => format!("consent-pending:{w}"),
        ToolsError::ConsentRefused(w) => format!("consent-refused:{w}"),
    }
}
fn outcome(o: &Outcome) -> String {
    match o {
        Outcome::Closed => "closed".into(),
        Outcome::Cancelled => "cancelled".into(),
        Outcome::Gone => "gone".into(),
        Outcome::Failed(m) => format!("failed({m})"),
        _ => "other".into(),
    }
}
fn tool_lines(handle: &str) -> Result<Vec<String>, String> {
    agent_tools::describe(handle)
        .map(|tools| {
            tools
                .into_iter()
                .map(|t| format!("{}|{}|{}|{}|{}", t.name, t.parallel, t.idempotent, t.description, t.input_schema))
                .collect()
        })
        .map_err(error)
}

impl exports::mods::toolbox::api::Guest for Guest {
    fn open(toolsets: Vec<String>) -> Result<String, String> {
        agent_tools::open(&toolsets).map_err(error)
    }
    fn describe(handle: String) -> Result<Vec<String>, String> {
        tool_lines(&handle)
    }
    fn call(handle: String, key: String, tool: String, args: String) -> Result<String, String> {
        agent_tools::call(&handle, &key, &tool, &args).map_err(error)
    }
    fn call_twice(handle: String, key: String, tool: String, args: String) -> Vec<String> {
        (0..2)
            .map(|_| {
                agent_tools::call(&handle, &key, &tool, &args).unwrap_or_else(error)
            })
            .collect()
    }
    fn open_describe(toolsets: Vec<String>) -> String {
        match agent_tools::open(&toolsets) {
            Ok(h) => match tool_lines(&h) {
                Ok(_) => "ok".into(),
                Err(e) => e,
            },
            Err(e) => error(e),
        }
    }
    fn open_many(toolsets: Vec<String>, n: u32) -> Vec<String> {
        (0..n)
            .map(|_| agent_tools::open(&toolsets).unwrap_or_else(error))
            .collect()
    }
    fn close(handle: String) {
        agent_tools::close(&handle);
    }
    fn items(s: String) -> Vec<String> {
        lines(&format!("items/{s}"))
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect()
    }
    fn ended() -> Vec<String> {
        lines("ended").into_iter().filter(|l| !l.is_empty()).collect()
    }
    fn who() -> String {
        caller::identity()
    }
    fn start(launch: String) -> String {
        match claude_code::start(&launch) {
            Ok(s) => s,
            Err(StartError::BadLaunch(b)) => format!("bad-launch:{}:{}", b.field, b.reason),
            Err(e) => format!("{e:?}"),
        }
    }
}
impl exports::watane::host::stream_sink::Guest for Guest {
    fn items(s: String, items: Vec<Item>) -> SinkOutcome {
        for item in &items {
            add_line(&format!("items/{s}"), String::from_utf8_lossy(&item.body).into_owned());
        }
        SinkOutcome::Taken
    }
    fn ended(s: String, o: Outcome) {
        add_line("ended", format!("{s} {}", outcome(&o)));
    }
}
export!(Guest);
