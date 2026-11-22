//! Starts Claude Code, keeps what the stream gave it, and says what the page tells it.
wit_bindgen::generate!({ path: "../wit", world: "probe", generate_all });

use exports::mods::probe::chat::Guest;
use exports::watane::host::stream_sink::{Guest as Sink, Item, Outcome, SinkOutcome};
use watane::host::{claude_code, claude_session, state};

struct Mod;

impl Guest for Mod {
    fn start() -> String {
        let launch = r#"{"version":1,"workspace":"scratch"}"#;
        let id = claude_code::start(launch).expect("start");
        state::put("stream", id.as_bytes());
        id
    }
    fn say(text: String) {
        let id = String::from_utf8(state::get("stream").expect("started")).expect("utf8");
        claude_session::say(&id, &text).expect("say");
    }
    fn seen() -> Vec<String> {
        serde_json::from_slice(&state::get("seen").unwrap_or_else(|| b"[]".to_vec())).unwrap_or_default()
    }
}
impl Sink for Mod {
    fn items(_s: String, items: Vec<Item>) -> SinkOutcome {
        let mut seen = <Mod as Guest>::seen();
        seen.extend(items.iter().map(|i| String::from_utf8_lossy(&i.body).into_owned()));
        state::put("seen", &serde_json::to_vec(&seen).expect("encodes"));
        SinkOutcome::Taken
    }
    fn ended(_s: String, outcome: Outcome) {
        let mut seen = <Mod as Guest>::seen();
        seen.push(format!("ended {outcome:?}"));
        state::put("seen", &serde_json::to_vec(&seen).expect("encodes"));
    }
}
export!(Mod);
