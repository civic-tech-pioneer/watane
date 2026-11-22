use watane::host::claude_types::{SessionError, StartError};
use watane::host::streams::{Item, Outcome, SinkOutcome};
use watane::host::{caller, claude_code, claude_code_shell, claude_session, host_info, state, streams};

wit_bindgen::generate!({ path: "../../mods/claude/wit", world: "claude", generate_all });
struct Guest;

fn get(key: &str) -> Option<String> {
    state::get(key).map(|v| String::from_utf8(v).unwrap())
}
fn put(key: &str, value: &str) {
    state::put(key, value.as_bytes());
}
fn lines(key: &str) -> Vec<String> {
    get(key)
        .map(|v| v.split('\n').map(str::to_owned).collect())
        .unwrap_or_default()
}
fn add_line(key: &str, line: String) {
    let mut all = lines(key);
    all.push(line);
    put(key, &all.join("\n"));
}
fn start_error(e: StartError) -> String {
    match e {
        StartError::BadLaunch(b) => format!("bad-launch:{}:{}", b.field, b.reason),
        StartError::Destination => "destination".into(),
        StartError::ForbiddenHeader(h) => format!("forbidden-header:{h}"),
        StartError::TooLarge => "too-large".into(),
        StartError::Full => "full".into(),
        StartError::NotOwner => "not-owner".into(),
        StartError::NotDirect => "not-direct".into(),
    }
}
fn session_error(e: SessionError) -> String {
    match e {
        SessionError::NotYours => "not-yours",
        SessionError::UnknownStream => "unknown-stream",
        SessionError::SlashCommand => "slash-command",
        SessionError::UnknownAsk => "unknown-ask",
        SessionError::NotAllowedHere => "not-allowed-here",
        SessionError::NotOwner => "not-owner",
        SessionError::NotDirect => "not-direct",
    }
    .into()
}
fn session(r: Result<(), SessionError>) -> String {
    r.map_or_else(session_error, |()| "ok".into())
}
fn outcome(o: &Outcome) -> String {
    match o {
        Outcome::Closed => "closed".into(),
        Outcome::Cancelled => "cancelled".into(),
        Outcome::Lagging => "lagging".into(),
        Outcome::Replaced => "replaced".into(),
        Outcome::ConsumerFailed => "consumer-failed".into(),
        Outcome::IdleTimeout => "idle-timeout".into(),
        Outcome::Gone => "gone".into(),
        Outcome::Mismatch(f) => format!("mismatch({f})"),
        Outcome::NoBinary => "no-binary".into(),
        Outcome::Version(v) => format!("version({} need {})", v.found, v.need),
        Outcome::ConsentRefused(w) => format!("consent-refused({w})"),
        Outcome::NoApp => "no-app".into(),
        Outcome::NoAccount => "no-account".into(),
        Outcome::Failed(m) => format!("failed({m})"),
    }
}

impl exports::mods::claude::api::Guest for Guest {
    fn start(launch: String) -> String {
        claude_code::start(&launch).unwrap_or_else(start_error)
    }
    fn start_shell(launch: String) -> String {
        claude_code_shell::start(&launch).unwrap_or_else(start_error)
    }
    fn say(s: String, text: String) -> String {
        session(claude_session::say(&s, &text))
    }
    fn say_then_fail(s: String, text: String) {
        let _ = claude_session::say(&s, &text);
        panic!("failed after say");
    }
    fn answer(s: String, ask: String) -> String {
        session(claude_session::answer(&s, &ask, &watane::host::claude_types::Decision::None))
    }
    fn interrupt(s: String) -> String {
        session(claude_session::interrupt(&s))
    }
    fn compact(s: String) -> String {
        session(claude_session::compact(&s))
    }
    fn cancel(s: String) {
        streams::cancel(&s);
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
    fn generation() -> u64 {
        host_info::generation()
    }
    fn who() -> String {
        caller::identity()
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
