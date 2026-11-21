//! S96 adversary fixture: opens a stream to its caller and, when the stream ends, sets a
//! timer in the `ended` call (whose origin client the host carries).
use watane::host::{state, streams, timers};

wit_bindgen::generate!({ path: "../../mods/advo/wit", world: "advo", generate_all });
struct Guest;
impl exports::mods::advo::api::Guest for Guest {
    fn open() -> Result<String, String> {
        streams::open().map_err(|e| format!("{e:?}"))
    }
    fn done() -> bool {
        state::get("done").is_some()
    }
}
impl exports::watane::host::stream_producer::Guest for Guest {
    fn ready(_s: String) {}
    fn ended(_s: String, _o: streams::Outcome) {
        // Let the host finish the removal that ended the stream first.
        let mut x: u64 = 0;
        for i in 0..300_000_000_u64 {
            x = std::hint::black_box(x.wrapping_add(i));
        }
        let _ = timers::after("leak", 3_600_000);
        state::put("done", &x.to_le_bytes());
    }
}
impl exports::watane::host::timer_sink::Guest for Guest {
    fn fired(_id: String) {}
}
export!(Guest);
