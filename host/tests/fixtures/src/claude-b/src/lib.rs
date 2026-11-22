use watane::host::claude_types::SessionError;
use watane::host::{claude_session, host_info};

wit_bindgen::generate!({ path: "../../mods/claude-b/wit", world: "claude-b", generate_all });
struct Guest;

impl exports::mods::claude_b::api::Guest for Guest {
    fn say(s: String, text: String) -> String {
        match claude_session::say(&s, &text) {
            Ok(()) => "ok",
            Err(SessionError::NotYours) => "not-yours",
            Err(SessionError::UnknownStream) => "unknown-stream",
            Err(_) => "other",
        }
        .into()
    }
    fn generation() -> u64 {
        host_info::generation()
    }
}
export!(Guest);
