// The same mod as `claude`, under the name `claude-b`: a second consumer (S100 load).
use watane::host::claude_types::{SessionError, StartError};
use watane::host::streams::{Item, Outcome, SinkOutcome};
use watane::host::{caller, claude_code, claude_code_shell, claude_session, host_info, state, streams};

wit_bindgen::generate!({ path: "../../mods/claude-b/wit", world: "claude-b", generate_all });
struct Guest;

fn get(key: &str) -> Option<String> {
    state::get(key).map(|v| String::from_utf8(v).unwrap())
}
fn put(key: &str, value: &str) {
    state::put(key, value.as_bytes());
}
fn lines(key: &str) -> Vec<String> {
    get(key)
        .map(|v| v.split('\n').map(str::to_owned).collect())
        .unwrap_or_default()
}
fn add_line(key: &str, line: String) {
    let mut all = lines(key);
    all.push(line);
    put(key, &all.join("\n"));
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
fn session_error(e: SessionError) -> String {
    match e {
        SessionError::NotYours => "not-yours",
        SessionError::UnknownStream => "unknown-stream",
        SessionError::SlashCommand => "slash-command",
        SessionError::UnknownAsk => "unknown-ask",
        SessionError::NotAllowedHere => "not-allowed-here",
        SessionError::NotOwner => "not-owner",
        SessionError::NotDirect => "not-direct",
    }
    .into()
}
/// `none`, `allow`, `allow:<json>`, or `deny:<message>`.
fn decision_of(text: &str) -> watane::host::claude_types::Decision {
    use watane::host::claude_types::Decision;
    match text.split_once(':') {
        None if text == "allow" => Decision::Allow(None),
        None => Decision::None,
        Some(("allow", json)) => Decision::Allow(Some(json.to_owned())),
        Some((_, message)) => Decision::Deny(message.to_owned()),
    }
}
fn session(r: Result<(), SessionError>) -> String {
    r.map_or_else(session_error, |()| "ok".into())
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

fn micros() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros()
}
/// The load test's quiet sink: one small state write per batch. The tally of a stream is
/// `key=value` lines: `next` (the next `n` of an `assistant` line), `bad` (lines out of order),
/// `last` (when the latest `assistant` line came, in microseconds), `ask<id>` and `res<id>`
/// (when that `ask` or `ask-resolved` came).
fn tally_items(s: &str, items: &[Item]) {
    let key = format!("tally/{s}");
    let old = get(&key).unwrap_or_default();
    let mut map: std::collections::BTreeMap<String, String> = old
        .lines()
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
        .collect();
    let num = |map: &std::collections::BTreeMap<String, String>, k: &str, d: u64| {
        map.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    };
    let (mut next, mut bad) = (num(&map, "next", 1), num(&map, "bad", 0));
    let now = micros();
    for item in items {
        let body = String::from_utf8_lossy(&item.body);
        if let Some(rest) = body.strip_prefix(r#"{"type":"ask","id":""#) {
            let id = rest.split('"').next().unwrap_or("");
            map.insert(format!("ask{id}"), now.to_string());
            let tuid = body.split(r#""tool_use_id":""#).nth(1).and_then(|r| r.split('"').next());
            map.insert(format!("tu{id}"), tuid.unwrap_or("").to_owned());
        } else if let Some(rest) = body.strip_prefix(r#"{"type":"ask-resolved","ask":""#) {
            let id = rest.split('"').next().unwrap_or("");
            map.insert(format!("res{id}"), now.to_string());
        } else if let Some(rest) = body.strip_prefix(r#"{"type":"assistant","n":"#) {
            let n: u64 = rest.split(',').next().and_then(|v| v.parse().ok()).unwrap_or(0);
            if n != next {
                bad += 1;
            }
            next = n + 1;
            map.insert("last".into(), now.to_string());
        }
    }
    map.insert("next".into(), next.to_string());
    map.insert("bad".into(), bad.to_string());
    let text: Vec<String> = map.iter().map(|(k, v)| format!("{k}={v}")).collect();
    put(&key, &text.join("\n"));
}

impl exports::mods::claude_b::api::Guest for Guest {
    fn quiet() {
        put("quiet", "1");
    }
    fn tally(s: String) -> String {
        get(&format!("tally/{s}")).unwrap_or_default()
    }
    fn start(launch: String) -> String {
        claude_code::start(&launch).unwrap_or_else(start_error)
    }
    fn start_shell(launch: String) -> String {
        claude_code_shell::start(&launch).unwrap_or_else(start_error)
    }
    fn say(s: String, text: String) -> String {
        session(claude_session::say(&s, &text))
    }
    fn say_then_fail(s: String, text: String) {
        let _ = claude_session::say(&s, &text);
        panic!("failed after say");
    }
    fn answer(s: String, ask: String) -> String {
        session(claude_session::answer(&s, &ask, &watane::host::claude_types::Decision::None))
    }
    fn decide(s: String, ask: String, decision: String) -> String {
        session(claude_session::answer(&s, &ask, &decision_of(&decision)))
    }
    fn decide_then_fail(s: String, ask: String, decision: String) {
        let _ = claude_session::answer(&s, &ask, &decision_of(&decision));
        panic!("failed after answer");
    }
    fn interrupt(s: String) -> String {
        session(claude_session::interrupt(&s))
    }
    fn compact(s: String) -> String {
        session(claude_session::compact(&s))
    }
    fn cancel(s: String) {
        streams::cancel(&s);
    }
    fn items(s: String) -> Vec<String> {
        lines(&format!("items/{s}"))
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect()
    }
    fn ended() -> Vec<String> {
        lines("ended").into_iter().filter(|l| !l.is_empty()).collect()
    }
    fn generation() -> u64 {
        host_info::generation()
    }
    fn start_hand_off(launch: String) -> String {
        match claude_code::start(&launch) {
            Ok(s) => match streams::hand_off(&s) {
                Ok(()) => s,
                Err(e) => format!("hand-off:{e:?}"),
            },
            Err(e) => start_error(e),
        }
    }
    fn start_say_hand_off(launch: String, text: String) -> String {
        let Ok(s) = claude_code::start(&launch) else {
            return "start failed".into();
        };
        let said = session(claude_session::say(&s, &text));
        let handed = streams::hand_off(&s).map_or_else(|e| format!("{e:?}"), |()| "ok".into());
        let late = session(claude_session::say(&s, "late"));
        format!("{s} {said} {handed} {late}")
    }
    fn start_cancel(launch: String) -> String {
        match claude_code::start(&launch) {
            Ok(s) => {
                streams::cancel(&s);
                s
            }
            Err(e) => start_error(e),
        }
    }
    fn start_hand_off_fail(launch: String) {
        let s = claude_code::start(&launch).unwrap();
        let _ = streams::hand_off(&s);
        panic!("failed after hand-off");
    }
    fn open_stream() -> String {
        streams::open().unwrap_or_else(|e| format!("open:{e:?}"))
    }
    fn who() -> String {
        caller::identity()
    }
}
impl exports::watane::host::stream_sink::Guest for Guest {
    fn items(s: String, items: Vec<Item>) -> SinkOutcome {
        if get("quiet").is_some() {
            tally_items(&s, &items);
            return SinkOutcome::Taken;
        }
        for item in &items {
            add_line(&format!("items/{s}"), String::from_utf8_lossy(&item.body).into_owned());
        }
        SinkOutcome::Taken
    }
    fn ended(s: String, o: Outcome) {
        add_line("ended", format!("{s} {}", outcome(&o)));
    }
}
export!(Guest);
