#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

struct Journey {
    temp: tempfile::TempDir,
}
impl Journey {
    fn run(&self, slug: &str, args: &[&str], exit: i32) -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(self.temp.path())
            .env("WATANE_DEV_CACHE", self.temp.path().join("cache"))
            .env(
                "WATANE_BIN",
                std::env::var_os("WATANE_BIN").expect("set WATANE_BIN"),
            )
            .arg("--dir")
            .arg(self.temp.path().join(slug))
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&output.stderr)));
        assert_eq!(output.status.code(), Some(exit), "{value}");
        value
    }
}
impl Drop for Journey {
    fn drop(&mut self) {
        let _ = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .env("WATANE_DEV_CACHE", self.temp.path().join("cache"))
            .arg("--dir")
            .arg(self.temp.path().join("recommendations"))
            .args(["serve", "--stop", "--json"])
            .output();
    }
}
fn call(authority: &str, function: &str, args: Value) -> Value {
    let port: u16 = authority.rsplit_once(':').unwrap().1.parse().unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let body = json!({"fn":function,"args":args}).to_string();
    write!(stream, "POST /call HTTP/1.1\r\nHost: {authority}\r\nOrigin: http://{authority}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap()
}
const READING_LOGIC: &str = r#"
wit_bindgen::generate!({ path: "../wit", world: "reading-list", generate_all });
use watane::host::state;
struct Mod;
fn books() -> Vec<String> { state::get("books").and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default() }
impl exports::mods::reading_list::books::Guest for Mod {
    fn add(title: String) -> u32 {
        let mut titles = books(); titles.push(title.clone());
        state::put("books", &serde_json::to_vec(&titles).unwrap());
        mods::reading_list::events::added(&title); titles.len() as u32
    }
    fn all() -> Vec<String> { books() }
    fn list() -> Vec<String> { books() }
}
export!(Mod);
"#;

#[test]
#[ignore = "slow: needs WATANE_BIN; journey S11 on real programs"]
fn ac4_s11_call_and_event_across_two_served_mods() {
    let j = Journey {
        temp: tempfile::tempdir().unwrap(),
    };
    j.run("reading-list", &["new", "reading-list"], 0);
    j.run("recommendations", &["new", "recommendations"], 0);
    let reading = j.temp.path().join("reading-list");
    fs::write(
        reading.join("wit/world.wit"),
        include_str!("../fixtures/recommendations/wit/deps/reading-list/world.wit"),
    )
    .unwrap();
    fs::write(reading.join("logic/src/lib.rs"), READING_LOGIC).unwrap();
    fs::remove_file(reading.join("tests/counter.json")).unwrap();
    fs::write(reading.join("tests/books.json"), r#"{"steps":[{"call":"books.add","args":{"title":"Emma"},"returns":1},{"call":"books.list","returns":["Emma"]}]}"#).unwrap();
    fs::write(reading.join("ui/index.html"), r#"<!doctype html><title>Reading list</title><form id="add"><input id="title" required><button>Add book</button></form><pre id="books"></pre><script src="/watane.js"></script><script src="app.js"></script>"#).unwrap();
    fs::write(reading.join("ui/app.js"), r#"
const read = async () => { document.getElementById('books').textContent = JSON.stringify(await watane.call('books.list')); };
document.getElementById('add').addEventListener('submit', async (e) => {
    e.preventDefault();
    await watane.call('books.add', {title: document.getElementById('title').value});
    await read();
});
watane.on(read);
"#).unwrap();
    let added = j.run("recommendations", &["add", "reading-list"], 0);
    let import = added["result"]["imports"][0].as_str().unwrap();
    let subscription = added["result"]["subscriptions"][0].as_str().unwrap();
    assert_eq!(import, "import mods:reading-list/books@0.1.0;");
    assert_eq!(subscription, "export mods:reading-list/events@0.1.0;");
    let recs = j.temp.path().join("recommendations");
    let world = include_str!("../fixtures/recommendations/wit/world.wit")
        .replace("import mods:reading-list/books@0.1.0;", import)
        .replace("export mods:reading-list/events@0.1.0;", subscription);
    fs::write(recs.join("wit/world.wit"), world).unwrap();
    fs::write(
        recs.join("logic/src/lib.rs"),
        include_str!("../fixtures/src/recommendations/src/lib.rs")
            .replace("../../recommendations/wit", "../wit"),
    )
    .unwrap();
    fs::remove_file(recs.join("tests/counter.json")).unwrap();
    for file in ["tests/books.json", "ui/index.html", "ui/app.js"] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/recommendations")
                .join(file),
            recs.join(file),
        )
        .unwrap();
    }
    let tested = j.run("recommendations", &["test"], 0);
    assert_eq!(tested["result"]["contract"]["passed"], 3);
    let served = j.run("recommendations", &["serve"], 0);
    let pages = served["result"]["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0]["local"], "reading-list");
    assert_eq!(pages[1]["local"], "recommendations");
    let reading_url = pages[0]["url"].as_str().unwrap();
    let recs_url = pages[1]["url"].as_str().unwrap();
    assert!(
        served["next"].as_str().unwrap().contains(recs_url),
        "{served}"
    );
    let reading_authority = reading_url
        .strip_prefix("http://")
        .unwrap()
        .trim_end_matches('/');
    let recs_authority = recs_url
        .strip_prefix("http://")
        .unwrap()
        .trim_end_matches('/');
    let port: u16 = recs_authority.rsplit_once(':').unwrap().1.parse().unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    write!(
        stream,
        "GET /events HTTP/1.1\r\nHost: {recs_authority}\r\nOrigin: http://{recs_authority}\r\n\r\n"
    )
    .unwrap();
    let mut events = BufReader::new(stream);
    let mut line = String::new();
    events.read_line(&mut line).unwrap();
    assert!(line.starts_with("HTTP/1.1 200"), "{line}");
    loop {
        line.clear();
        assert!(
            events.read_line(&mut line).unwrap() > 0,
            "event headers ended"
        );
        if line == "\r\n" {
            break;
        }
    }
    // Step 11 (host 8.4): the first message of a connection carries its id. The body is
    // chunked: skip the chunk-size line before the first `data:` line.
    loop {
        line.clear();
        assert!(
            events.read_line(&mut line).unwrap() > 0,
            "event stream ended"
        );
        if line.starts_with("data:") {
            break;
        }
    }
    assert!(
        line.starts_with("data: {\"type\":\"connection\",\"id\":\""),
        "{line}"
    );
    assert_eq!(
        call(reading_authority, "books.add", json!({"title":"Dune"})),
        json!({"value":1})
    );
    assert_eq!(
        call(recs_authority, "api.list", json!({})),
        json!({"value":["Dune"]})
    );
    assert_eq!(
        call(recs_authority, "api.received", json!({})),
        json!({"value":["Dune"]})
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .expect("changed within 30 seconds");
        events.get_ref().set_read_timeout(Some(remaining)).unwrap();
        line.clear();
        assert!(
            events.read_line(&mut line).unwrap() > 0,
            "event stream ended"
        );
        if let Some(data) = line.strip_prefix("data:")
            && serde_json::from_str::<Value>(data.trim()).unwrap() == json!({"type":"changed"})
        {
            break;
        }
    }
    drop(events);
    let stopped = j.run("recommendations", &["serve", "--stop"], 0);
    assert_eq!(stopped["result"]["stopped"], true);
}
