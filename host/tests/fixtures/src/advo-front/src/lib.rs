//! S96 adversary fixture: a front mod. It calls `advo` or `relay` and then sleeps by WASI
//! (no fuel), so its chain stays in flight with the effect queued while others call.
use watane::host::streams::{Item, Outcome, SinkOutcome};

wit_bindgen::generate!({ path: "../../mods/advo-front/wit", world: "advo-front", generate_all });
struct Guest;
fn sleep(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}
impl exports::mods::advo_front::api::Guest for Guest {
    fn open() -> Result<String, String> {
        mods::advo::api::open()
    }
    fn open_then_sleep(ms: u64) -> Result<String, String> {
        let answer = mods::advo::api::open();
        sleep(ms);
        answer
    }
    fn push_then_sleep(s: String, bytes: u32, ms: u64) -> String {
        let answer = mods::advo::api::push(&s, bytes);
        sleep(ms);
        answer
    }
    fn try_after_then_sleep(id: String, ms: u64, sleep_ms: u64) -> String {
        let answer = mods::advo::api::try_after(&id, ms);
        sleep(sleep_ms);
        answer
    }
    fn hand_off_then_sleep(s: String, ms: u64) -> String {
        let answer = match mods::relay::api::hand_off(&s) {
            Ok(()) => "ok".to_owned(),
            Err(e) => e,
        };
        sleep(ms);
        answer
    }
    fn cancel_then_sleep(s: String, ms: u64) {
        mods::relay::api::cancel(&s);
        sleep(ms);
    }
    fn resume_then_sleep(s: String, ms: u64) {
        let _ = mods::relay::api::resume(&s);
        sleep(ms);
    }
    fn mode(m: String) {
        watane::host::state::put("mode", m.as_bytes());
    }
    fn ended() -> Vec<String> {
        watane::host::state::get("ended")
            .map(|v| String::from_utf8(v).unwrap().lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }
}
impl exports::watane::host::stream_sink::Guest for Guest {
    fn items(_s: String, _items: Vec<Item>) -> SinkOutcome {
        match watane::host::state::get("mode").as_deref() {
            Some(b"slow") => {
                sleep(1500);
                SinkOutcome::Taken
            }
            _ => SinkOutcome::Taken,
        }
    }
    fn ended(s: String, outcome: Outcome) {
        let mut all = <Guest as exports::mods::advo_front::api::Guest>::ended();
        all.push(format!("{s} {}", match outcome {
            Outcome::Gone => "gone",
            Outcome::Closed => "closed",
            _ => "other",
        }));
        watane::host::state::put("ended", all.join("\n").as_bytes());
    }
}
export!(Guest);
