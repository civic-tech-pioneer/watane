//! S99 fixture: the consumer of a stream that `claude` started and handed off in one chain.
use watane::host::claude_types::SessionError;
use watane::host::streams::{Item, Outcome, SinkOutcome};
use watane::host::{claude_session, state};

wit_bindgen::generate!({ path: "../../mods/claude-front/wit", world: "claude-front", generate_all });
struct Guest;

fn lines(key: &str) -> Vec<String> {
    state::get(key)
        .map(|v| String::from_utf8(v).unwrap().split('\n').map(str::to_owned).collect())
        .unwrap_or_default()
}
fn add_line(key: &str, line: String) {
    let mut all = lines(key);
    all.push(line);
    state::put(key, all.join("\n").as_bytes());
}
fn say_to(s: &str, text: &str) -> String {
    match claude_session::say(s, text) {
        Ok(()) => "ok",
        Err(SessionError::NotYours) => "not-yours",
        Err(SessionError::UnknownStream) => "unknown-stream",
        Err(_) => "other",
    }
    .into()
}

impl exports::mods::claude_front::api::Guest for Guest {
    fn run(launch: String) -> String {
        mods::claude::api::start_hand_off(&launch)
    }
    fn run_say(launch: String, text: String) -> String {
        let s = mods::claude::api::start_hand_off(&launch);
        // The stream is this mod's in the chain already.
        format!("{s} {}", say_to(&s, &text))
    }
    fn run_say_first(launch: String, text: String) -> String {
        mods::claude::api::start_say_hand_off(&launch, &text)
    }
    fn run_cancel(launch: String) -> String {
        let s = mods::claude::api::start_hand_off(&launch);
        watane::host::streams::cancel(&s);
        s
    }
    fn run_fail(launch: String) -> String {
        mods::claude::api::start_hand_off_fail(&launch);
        "unreachable".into()
    }
    fn say(s: String, text: String) -> String {
        say_to(&s, &text)
    }
    fn items(s: String) -> Vec<String> {
        lines(&format!("items/{s}")).into_iter().filter(|l| !l.is_empty()).collect()
    }
    fn ended() -> Vec<String> {
        lines("ended").into_iter().filter(|l| !l.is_empty()).collect()
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
        add_line("ended", format!("{s} {}", match o {
            Outcome::Cancelled => "cancelled",
            Outcome::Closed => "closed",
            Outcome::Gone => "gone",
            _ => "other",
        }));
    }
}
export!(Guest);
