use watane::host::state;
use watane::host::streams::{self, Item, Outcome, SinkOutcome, StreamError};
use watane::host::timers::{self, TimerError};

wit_bindgen::generate!({ path: "../../mods/relay/wit", world: "relay", generate_all });
struct Guest;

fn get(key: &str) -> Option<String> {
    state::get(key).map(|v| String::from_utf8(v).unwrap())
}
fn put(key: &str, value: &str) {
    state::put(key, value.as_bytes());
}
fn lines(key: &str) -> Vec<String> {
    get(key)
        .map(|v| v.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}
fn add_line(key: &str, line: String) {
    let mut all = lines(key);
    all.push(line);
    put(key, &all.join("\n"));
}
fn error(e: StreamError) -> String {
    match e {
        StreamError::NotYours => "not-yours",
        StreamError::UnknownStream => "unknown-stream",
        StreamError::Full => "full",
        StreamError::TooLarge => "too-large",
    }
    .into()
}
fn timer_error(e: TimerError) -> String {
    match e {
        TimerError::TooMany => "too-many",
        TimerError::BadId => "bad-id",
        TimerError::BadDelay => "bad-delay",
    }
    .into()
}
fn case(o: &Outcome) -> &'static str {
    match o {
        Outcome::Closed => "closed",
        Outcome::Cancelled => "cancelled",
        Outcome::Lagging => "lagging",
        Outcome::Replaced => "replaced",
        Outcome::ConsumerFailed => "consumer-failed",
        Outcome::IdleTimeout => "idle-timeout",
        Outcome::Gone => "gone",
        Outcome::Mismatch(_) => "mismatch",
        Outcome::NoBinary => "no-binary",
        Outcome::Version(_) => "version",
        Outcome::ConsentRefused(_) => "consent-refused",
        Outcome::NoApp => "no-app",
        Outcome::NoAccount => "no-account",
        Outcome::Failed(_) => "failed",
    }
}
fn resume_ups(down: &str) {
    for up in lines(&format!("ups/{down}")) {
        let _ = streams::resume(&up);
    }
}

impl exports::mods::relay::api::Guest for Guest {
    fn open() -> Result<String, String> {
        streams::open().map_err(error)
    }
    fn push(s: String, text: String) -> Result<(), String> {
        streams::push(&s, text.as_bytes()).map_err(error)
    }
    fn close(s: String) {
        let outcome = if get("mode").as_deref() == Some("close-gone") {
            Outcome::Gone
        } else {
            Outcome::Closed
        };
        streams::close(&s, &outcome);
    }
    fn cancel(s: String) {
        streams::cancel(&s);
    }
    fn hand_off(s: String) -> Result<(), String> {
        streams::hand_off(&s).map_err(error)
    }
    fn resume(s: String) -> Result<(), String> {
        streams::resume(&s).map_err(error)
    }
    fn after(id: String, ms: u64) -> Result<(), String> {
        timers::after(&id, ms).map_err(timer_error)
    }
    fn cancel_timer(id: String) {
        timers::cancel(&id);
    }
    fn after_then_fail(id: String, ms: u64) {
        let _ = timers::after(&id, ms);
        panic!("failed after");
    }
    fn mode(m: String) {
        put("mode", &m);
    }
    fn forward(up: String, down: String, write: bool) {
        put(&format!("fwd/{up}"), &down);
        put(&format!("w/{up}"), if write { "1" } else { "0" });
        let mut ups = lines(&format!("ups/{down}"));
        if !ups.contains(&up) {
            ups.push(up);
        }
        put(&format!("ups/{down}"), &ups.join("\n"));
    }
    fn received() -> Vec<String> {
        lines("index")
            .iter()
            .map(|key| get(key).unwrap_or_default())
            .collect()
    }
    fn ended() -> Vec<String> {
        lines("ended")
    }
    fn fired() -> Vec<String> {
        lines("fired")
    }
    fn who() -> String {
        watane::host::caller::identity()
    }
}
impl exports::watane::host::stream_sink::Guest for Guest {
    fn items(s: String, items: Vec<Item>) -> SinkOutcome {
        if let Some(down) = get(&format!("fwd/{s}")) {
            for item in &items {
                match streams::push(&down, &item.body) {
                    Ok(()) | Err(StreamError::UnknownStream) => {}
                    Err(_) => return SinkOutcome::Defer,
                }
            }
            if get(&format!("w/{s}")).as_deref() == Some("1")
                && let Some(last) = items.last()
            {
                put(&format!("last/{s}"), &last.seq.to_string());
            }
            return SinkOutcome::Taken;
        }
        let mut index = lines("index");
        for item in &items {
            let key = format!("got/{s}/{:020}", item.seq);
            put(&key, &String::from_utf8_lossy(&item.body));
            if !index.contains(&key) {
                index.push(key);
            }
        }
        index.sort();
        put("index", &index.join("\n"));
        put(&format!("who/{s}"), &watane::host::caller::identity());
        match get("mode").as_deref() {
            Some("defer") => SinkOutcome::Defer,
            Some("trap") => panic!("mode trap"),
            Some("trap-once") => {
                let first = items.first().map_or(0, |i| i.seq);
                if mods::latch::api::trip(&format!("{s}/{first}")) {
                    panic!("mode trap-once");
                }
                SinkOutcome::Taken
            }
            _ => SinkOutcome::Taken,
        }
    }
    fn ended(s: String, outcome: Outcome) {
        add_line("ended", format!("{s} {}", case(&outcome)));
    }
}
impl exports::watane::host::stream_producer::Guest for Guest {
    fn ready(s: String) {
        add_line("ended", format!("{s} ready"));
        resume_ups(&s);
    }
    fn ended(s: String, outcome: Outcome) {
        add_line("ended", format!("{s} {}", case(&outcome)));
        resume_ups(&s);
    }
}
impl exports::watane::host::timer_sink::Guest for Guest {
    fn fired(id: String) {
        add_line("fired", id);
    }
}
export!(Guest);
