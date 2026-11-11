use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::Command,
    time::Duration,
};

struct Journey {
    temp: tempfile::TempDir,
    dir: PathBuf,
    host: PathBuf,
}

impl Journey {
    fn run(&self, args: &[&str], expected_exit: i32) -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(if self.dir.exists() {
                &self.dir
            } else {
                self.temp.path()
            })
            .env("WATANE_DEV_CACHE", self.temp.path().join("cache"))
            .env("WATANE_BIN", &self.host)
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.code(), Some(expected_exit), "{value}");
        value
    }
}

impl Drop for Journey {
    fn drop(&mut self) {
        // Also stop the preview if an assertion fails before the final step.
        let _ = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(self.temp.path())
            .env("WATANE_DEV_CACHE", self.temp.path().join("cache"))
            .args([
                "--dir",
                self.dir.to_str().unwrap(),
                "serve",
                "--stop",
                "--json",
            ])
            .output();
    }
}

fn request(port: u16, method: &str, path: &str, headers: &str, body: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(stream, "{method} {path} HTTP/1.1\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    assert!(
        response.starts_with(b"HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&response)
    );
    let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    response[split + 4..].to_vec()
}

const LOGIC: &str = r#"
wit_bindgen::generate!({ path: "../wit", world: "reading-list", generate_all });
use exports::mods::reading_list::api::Guest;
use mods::reading_list::changes;
use watane::host::state;
struct Mod;
fn books() -> Vec<String> {
    state::get("books").and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
impl Guest for Mod {
    fn add(title: String) -> u32 {
        let mut titles = books();
        titles.push(title.clone());
        state::put("books", &serde_json::to_vec(&titles).unwrap());
        changes::added(&title);
        titles.len() as u32
    }
    fn list() -> Vec<String> {
        let mut titles = books();
        titles.reverse(); // the bug the contract catches
        titles
    }
}
export!(Mod);
"#;

#[test]
#[ignore = "needs WATANE_BIN"]
fn ac1_j1_reading_list_on_real_programs() {
    let temp = tempfile::tempdir().unwrap();
    let j = Journey {
        dir: temp.path().join("reading-list"),
        host: std::env::var_os("WATANE_BIN")
            .expect("set WATANE_BIN")
            .into(),
        temp,
    };
    j.run(&["new", "reading-list"], 0);
    fs::write(
        j.dir.join("wit/world.wit"),
        r#"package mods:reading-list@0.1.0;
interface api {
    add: func(title: string) -> u32;
    %list: func() -> list<string>;
}
interface changes { added: func(title: string); }
world reading-list {
    import watane:host/state@0.1.0;
    import changes;
    export api;
}
"#,
    )
    .unwrap();
    fs::write(j.dir.join("logic/src/lib.rs"), LOGIC).unwrap();
    let html = b"<!doctype html><meta charset=\"utf-8\"><title>Reading list</title><h1>Books</h1><form id=\"add\"><input id=\"title\" required><button>Add book</button></form><ul id=\"books\"></ul><p id=\"error\" role=\"alert\"></p><script src=\"/watane.js\"></script><script src=\"app.js\"></script>";
    fs::write(j.dir.join("ui/index.html"), html).unwrap();
    let js = r#"const fail = (e) => { document.getElementById('error').textContent = e.message; };
const read = async () => {
  const titles = await watane.call('api.list');
  const items = titles.map((title) => { const li = document.createElement('li'); li.textContent = title; return li; });
  document.getElementById('books').replaceChildren(...items);
};
document.getElementById('add').addEventListener('submit', async (e) => {
  e.preventDefault();
  try { await watane.call('api.add', {title: document.getElementById('title').value}); await read(); } catch (e) { fail(e); }
});
watane.on(() => read().catch(fail));
"#;
    fs::write(j.dir.join("ui/app.js"), js).unwrap();
    fs::remove_file(j.dir.join("tests/counter.json")).unwrap();
    fs::write(j.dir.join("tests/books.json"), r#"{"steps":[
        {"call":"api.add","args":{"title":"Dune"},"returns":1,"emits":[{"event":"changes.added","args":{"title":"Dune"}}]},
        {"call":"api.add","args":{"title":"Emma"},"returns":2},
        {"call":"api.list","returns":["Dune","Emma"]}
    ]}"#).unwrap();

    let failed = j.run(&["test"], 1);
    assert_eq!(failed["errors"][0]["code"], "test.contract");
    assert_eq!(failed["errors"][0]["file"], "tests/books.json");
    let failure = &failed["result"]["contract"]["failures"][0];
    assert_eq!(failure["file"], "tests/books.json");
    assert_eq!(failure["call"], "api.list");
    assert_eq!(failure["expected"], json!(["Dune", "Emma"]));
    assert_eq!(failure["got"], json!(["Emma", "Dune"]));

    fs::write(
        j.dir.join("logic/src/lib.rs"),
        LOGIC.replace(
            "let mut titles = books();\n        titles.reverse(); // the bug the contract catches",
            "let titles = books();",
        ),
    )
    .unwrap();
    let tested = j.run(&["test"], 0);
    assert_eq!(tested["next"], "watane-dev serve --json");
    let first = j.run(&["serve"], 0)["result"].clone();
    assert_eq!(first["reused"], false);
    let page = first["pages"][0]["url"].as_str().unwrap();
    let authority = page.strip_prefix("http://").unwrap().trim_end_matches('/');
    let (local, port_text) = authority.split_once(".localhost:").unwrap();
    // Host 3.2: a fresh local mod uses the slug, truncated to 21 bytes.
    let slug = "reading-list";
    let expected_local = slug[..slug.len().min(21)].trim_end_matches('-');
    assert_eq!(local, expected_local);
    let port: u16 = port_text.parse().unwrap();
    let page_headers = format!("Host: {authority}\r\n");
    assert_eq!(request(port, "GET", "/", &page_headers, ""), html);
    assert_eq!(
        request(port, "GET", "/app.js", &page_headers, ""),
        js.as_bytes()
    );
    let headers = format!(
        "Host: {authority}\r\nOrigin: http://{authority}\r\nContent-Type: application/json\r\n"
    );
    let added: Value = serde_json::from_slice(&request(
        port,
        "POST",
        "/call",
        &headers,
        r#"{"fn":"api.add","args":{"title":"Dune"}}"#,
    ))
    .unwrap();
    assert_eq!(added, json!({"value":1}));

    let edited = String::from_utf8(html.to_vec())
        .unwrap()
        .replace("<h1>Books</h1>", "<h1>Alice's books</h1>");
    fs::write(j.dir.join("ui/index.html"), &edited).unwrap();
    let second = j.run(&["serve"], 0)["result"].clone();
    assert_eq!(second["reused"], true);
    assert_eq!(second["api"], first["api"]);
    assert_eq!(second["pages"][0]["url"], first["pages"][0]["url"]);
    assert_eq!(
        request(port, "GET", "/", &page_headers, ""),
        edited.as_bytes()
    );
    assert_eq!(
        second["mcp"]["url"],
        format!("http://localhost:{port}/mcp/preview")
    );
    let token = second["mcp"]["token"].as_str().unwrap();
    let headers = format!(
        "Host: localhost:{port}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\n"
    );
    let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
        "params":{"name":format!("{expected_local}_api_list"),"arguments":{}}})
    .to_string();
    let response: Value =
        serde_json::from_slice(&request(port, "POST", "/mcp/preview", &headers, &body)).unwrap();
    assert_eq!(response["id"], 1);
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["isError"], false, "{response}");
    let text = &response["result"]["content"][0];
    assert_eq!(text["type"], "text");
    assert_eq!(
        serde_json::from_str::<Value>(text["text"].as_str().unwrap()).unwrap(),
        json!(["Dune"])
    );
    let stopped = j.run(&["serve", "--stop"], 0);
    assert_eq!(stopped["result"]["stopped"], true);
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    assert!(!PathBuf::from(first["home"].as_str().unwrap()).exists());
}
