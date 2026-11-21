//! S96 adversary fixture: opens a stream to its caller and, when the stream ends, sets a
//! timer in the `ended` call (whose origin client the host carries); sets and cancels timers,
//! slowly when asked, and records its firings.
use watane::host::{state, streams, timers};

wit_bindgen::generate!({ path: "../../mods/advo/wit", world: "advo", generate_all });
struct Guest;
/// Wall time without fuel: a WASI sleep.
fn spin(ms: u64) -> u64 {
    std::thread::sleep(std::time::Duration::from_millis(ms));
    ms
}
fn fired_list() -> Vec<String> {
    state::get("fired")
        .map(|v| String::from_utf8(v).unwrap().lines().map(str::to_owned).collect())
        .unwrap_or_default()
}
impl exports::mods::advo::api::Guest for Guest {
    fn open() -> Result<String, String> {
        streams::open().map_err(|e| format!("{e:?}"))
    }
    fn done() -> bool {
        state::get("done").is_some()
    }
    fn after(id: String, ms: u64) {
        timers::after(&id, ms).unwrap();
    }
    fn cancel_slowly(id: String) {
        let x = spin(800);
        timers::cancel(&id);
        state::put("spun", &x.to_le_bytes());
    }
    fn fired() -> Vec<String> {
        fired_list()
    }
}
impl exports::watane::host::stream_producer::Guest for Guest {
    fn ready(_s: String) {}
    fn ended(_s: String, _o: streams::Outcome) {
        // Let the host finish the removal that ended the stream first.
        let x = spin(300);
        let _ = timers::after("leak", 3_600_000);
        state::put("done", &x.to_le_bytes());
    }
}
impl exports::watane::host::timer_sink::Guest for Guest {
    fn fired(id: String) {
        if id == "probe" {
            let _ = timers::after("leak2", 3_600_000);
        }
        let mut all = fired_list();
        all.push(id);
        state::put("fired", all.join("\n").as_bytes());
    }
}
export!(Guest);
