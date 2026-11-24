//! S103 fixture: starts `claude-code` from an event it subscribes to (the title is the launch) and
//! from a timer it armed, so a test sees which origin client the host carries. It keeps what its
//! streams delivered.
use watane::host::streams::{Item, Outcome, SinkOutcome};
use watane::host::{claude_code, state, timers};

wit_bindgen::generate!({ path: "../../mods/claude-evt/wit", world: "claude-evt", generate_all });
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
fn start(launch: &str) {
    if let Ok(s) = claude_code::start(launch) {
        add_line("started", s);
    }
}
impl exports::mods::claude_evt::api::Guest for Guest {
    fn arm(launch: String, ms: u64) {
        state::put("launch", launch.as_bytes());
        timers::after("go", ms).unwrap();
    }
    fn started() -> Vec<String> {
        lines("started").into_iter().filter(|l| !l.is_empty()).collect()
    }
    fn items(s: String) -> Vec<String> {
        lines(&format!("items/{s}")).into_iter().filter(|l| !l.is_empty()).collect()
    }
    fn ended() -> Vec<String> {
        lines("ended").into_iter().filter(|l| !l.is_empty()).collect()
    }
}
impl exports::mods::shelf::events::Guest for Guest {
    fn added(title: String) {
        start(&title);
    }
}
impl exports::watane::host::timer_sink::Guest for Guest {
    fn fired(_id: String) {
        if let Some(v) = state::get("launch") {
            start(&String::from_utf8(v).unwrap());
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
        let text = match o {
            Outcome::Gone => "gone".to_owned(),
            Outcome::Closed => "closed".to_owned(),
            Outcome::ConsentRefused(w) => format!("consent-refused({w})"),
            _ => "other".to_owned(),
        };
        add_line("ended", format!("{s} {text}"));
    }
}
export!(Guest);
