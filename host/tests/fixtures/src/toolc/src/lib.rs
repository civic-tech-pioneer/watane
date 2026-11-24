use watane::host::streams::{Item, Outcome, SinkOutcome};
use watane::host::{claude_code, state};

wit_bindgen::generate!({ path: "../../mods/toolc/wit", world: "toolc", generate_all });
struct Guest;

fn note(line: &str) -> u32 {
    let mut all = state::get("seen")
        .map(|v| String::from_utf8(v).unwrap())
        .unwrap_or_default();
    all.push_str(line);
    all.push('\n');
    state::put("seen", all.as_bytes());
    u32::try_from(all.lines().count()).unwrap()
}
impl exports::mods::toolc::api::Guest for Guest {
    fn start(launch: String) -> String {
        note("start");
        claude_code::start(&launch).unwrap_or_else(|e| format!("{e:?}"))
    }
    fn touch() -> u32 {
        note("touch")
    }
    fn slow(ms: u32) -> u32 {
        std::thread::sleep(std::time::Duration::from_millis(u64::from(ms)));
        note("slow")
    }
    fn seen() -> Vec<String> {
        state::get("seen")
            .map(|v| String::from_utf8(v).unwrap().lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }
}
impl exports::watane::host::stream_sink::Guest for Guest {
    fn items(_s: String, _items: Vec<Item>) -> SinkOutcome {
        SinkOutcome::Taken
    }
    fn ended(_s: String, _o: Outcome) {}
}
export!(Guest);
