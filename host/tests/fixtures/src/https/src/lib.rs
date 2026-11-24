use watane::host::claude_types::StartError;
use watane::host::streams::{Item, Outcome, SinkOutcome};
use watane::host::{caller, https_stream, state, streams};

wit_bindgen::generate!({ path: "../../mods/https/wit", world: "https", generate_all });
struct Guest;

fn get(key: &str) -> Option<String> {
    state::get(key).map(|v| String::from_utf8(v).unwrap())
}
fn put(key: &str, value: &str) {
    state::put(key, value.as_bytes());
}
fn number(key: &str, default: u64) -> u64 {
    get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}
fn add_line(key: &str, line: String) {
    let mut all = get(key).unwrap_or_default();
    all.push_str(&line);
    all.push('\n');
    put(key, &all);
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
fn micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}
/// The `n` of a body that starts `{"n":<n>,`, and its `t` (microseconds) when it has one.
fn stamp(body: &str) -> Option<(u64, Option<u64>)> {
    let rest = body.strip_prefix(r#"{"n":"#)?;
    let n = rest.split(',').next()?.parse().ok()?;
    let t = rest
        .split(r#""t":"#)
        .nth(1)
        .and_then(|r| r.split([',', '}']).next())
        .and_then(|t| t.parse().ok());
    Some((n, t))
}

impl exports::mods::https::api::Guest for Guest {
    fn start(request: String) -> String {
        https_stream::start(&request).unwrap_or_else(start_error)
    }
    fn start_then_sleep(request: String, ms: u32) -> String {
        let s = https_stream::start(&request).unwrap_or_else(start_error);
        std::thread::sleep(std::time::Duration::from_millis(u64::from(ms)));
        s
    }
    fn start_many(request: String, n: u32) -> Vec<String> {
        (0..n)
            .map(|_| https_stream::start(&request).unwrap_or_else(start_error))
            .collect()
    }
    fn start_cancel(request: String) -> String {
        match https_stream::start(&request) {
            Ok(s) => {
                streams::cancel(&s);
                s
            }
            Err(e) => start_error(e),
        }
    }
    fn start_hand_off(request: String) -> String {
        match https_stream::start(&request) {
            Ok(s) => {
                let r = streams::hand_off(&s).map_or_else(|e| format!("{e:?}"), |()| "ok".into());
                format!("{s} {r}")
            }
            Err(e) => start_error(e),
        }
    }
    fn cancel(s: String) {
        streams::cancel(&s);
    }
    fn resume(s: String) -> String {
        streams::resume(&s).map_or_else(|e| format!("{e:?}"), |()| "ok".into())
    }
    /// `defer`: every batch is deferred. `quiet`: only the tally. `stamp`: the tally and the
    /// delay of each item, from its `t` to this call. Else every item is recorded.
    fn mode(m: String) {
        put("mode", &m);
    }
    fn items(s: String) -> Vec<String> {
        (1..=number(&format!("count/{s}"), 0))
            .map(|seq| get(&format!("item/{s}/{seq}")).unwrap_or_default())
            .collect()
    }
    fn ended() -> Vec<String> {
        get("ended")
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
    /// `n=<events>;bad=<out of order>;bytes=<total>;lat=<micros>,...`
    fn tally(s: String) -> String {
        format!(
            "n={};bad={};bytes={};lat={}",
            number(&format!("n/{s}"), 0),
            number(&format!("bad/{s}"), 0),
            number(&format!("bytes/{s}"), 0),
            get(&format!("lat/{s}")).unwrap_or_default()
        )
    }
    fn who() -> String {
        caller::identity()
    }
}
impl exports::watane::host::stream_sink::Guest for Guest {
    fn items(s: String, items: Vec<Item>) -> SinkOutcome {
        let mode = get("mode").unwrap_or_default();
        if mode == "defer" {
            return SinkOutcome::Defer;
        }
        if mode == "quiet" || mode == "stamp" {
            let now = micros();
            let (mut n, mut bad, mut bytes) = (
                number(&format!("n/{s}"), 0),
                number(&format!("bad/{s}"), 0),
                number(&format!("bytes/{s}"), 0),
            );
            let mut lat = get(&format!("lat/{s}")).unwrap_or_default();
            for item in &items {
                bytes += item.body.len() as u64;
                // The head item and any other that is no `{"n":...}` event is not counted.
                if let Some((k, t)) = stamp(&String::from_utf8_lossy(&item.body)) {
                    n += 1;
                    if k != n {
                        bad += 1;
                    }
                    if let (true, Some(t)) = (mode == "stamp", t) {
                        lat.push_str(&format!("{},", now.saturating_sub(t)));
                    }
                }
            }
            put(&format!("n/{s}"), &n.to_string());
            put(&format!("bad/{s}"), &bad.to_string());
            put(&format!("bytes/{s}"), &bytes.to_string());
            put(&format!("lat/{s}"), &lat);
            return SinkOutcome::Taken;
        }
        for item in &items {
            put(
                &format!("item/{s}/{}", item.seq),
                &String::from_utf8_lossy(&item.body),
            );
            put(&format!("count/{s}"), &item.seq.to_string());
        }
        SinkOutcome::Taken
    }
    fn ended(s: String, o: Outcome) {
        add_line("ended", format!("{s} {}", outcome(&o)));
    }
}
export!(Guest);
