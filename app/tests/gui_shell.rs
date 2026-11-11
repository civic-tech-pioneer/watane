#![cfg(feature = "driver")]
mod support;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::driver::App;
static GUI: std::sync::Mutex<()> = std::sync::Mutex::new(());
const TIMEOUT: Duration = Duration::from_secs(30);
fn until(app: &App, label: &str, js: &str) {
    let deadline = Instant::now() + TIMEOUT;
    while app.eval(label, js) != json!(true) {
        assert!(Instant::now() < deadline, "DOM condition timed out: {js}");
        std::thread::yield_now();
    }
}
#[test]
fn ac1_new_home_has_empty_launcher_and_mark() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    assert_eq!(
        app.eval("main", "document.querySelectorAll('[data-local]').length"),
        0
    );
    let mark = std::fs::read_to_string(app.home.join("app/mark")).unwrap();
    assert_eq!(
        app.eval("main", "document.querySelector('header .mark').textContent"),
        mark.trim_end()
    );
    let words: Vec<_> = mark.trim_end().split(' ').collect();
    assert_eq!(words.len(), 2);
    assert!(words.iter().all(|w| watane_app::mark::WORDS.contains(w)));
    assert!(
        !app.home.join("mods").exists()
            || std::fs::read_dir(app.home.join("mods"))
                .unwrap()
                .next()
                .is_none()
    );
    assert_eq!(app.port, 0);
    assert!(app.quit().success());
}

fn mod_view(local: &str, name: &str, page: Value) -> Value {
    json!({"local":local,"name":name,"slug":local,"version":"1.2.3","release":null,"publisher":null,"enabled":true,"page":page,"package":"abc","derivedFrom":null,"error":null,"grants":[]})
}
fn install(name: &str) -> Value {
    json!({"kind":"install","by_program":false,"mods":[{"local":"books","name":name,"slug":"books","version":"1.2.3","release":null,"publisher":null,"replaces":null,"grants":[],"uses":[]}]})
}
fn consent(app: &App) {
    assert!(app.wait("consent", TIMEOUT));
    until(
        app,
        "consent",
        "document.body.dataset.ready === 'true' && document.hasFocus()",
    );
    assert!(
        app.windows()
            .iter()
            .any(|w| w["label"] == "consent" && w["alwaysOnTop"] == true && w["focused"] == true)
    );
}
#[test]
fn ac2_fake_catalog_opens_one_window_and_no_page_is_not_found() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    // Serve a tiny page here; the shared page fixture belongs to S10-2.
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://127.0.0.1:{}/",
        listener.local_addr().unwrap().port()
    );
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut buf = [0; 4096];
            let _ = stream.read(&mut buf);
            let body = "<!doctype html><title>Books</title><p>Books page</p>";
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
        }
    });
    let home = tempfile::tempdir().unwrap();
    let fake=json!({"mods":[mod_view("books","Books",json!(url)),mod_view("logic","Logic",Value::Null)]}).to_string();
    let app = App::start(home.path(), &[("WATANE_APP_FAKE", &fake)]);
    until(&app, "main", "document.body.dataset.ready === 'true'");
    assert_eq!(
        app.eval(
            "main",
            "Array.from(document.querySelectorAll('[data-local] h2'),e=>e.textContent)"
        ),
        json!(["Books", "Logic"])
    );
    assert_eq!(
        app.eval(
            "main",
            "document.querySelector('[data-local=logic] button.open') === null"
        ),
        true
    );
    app.eval(
        "main",
        "document.querySelector('[data-local=books] button.open').click(); null",
    );
    assert!(app.wait("mod-books", TIMEOUT));
    let windows = app.windows();
    let w = windows.iter().find(|w| w["label"] == "mod-books").unwrap();
    assert_eq!(w["url"], url);
    assert_eq!(w["title"], "Books — local");
    assert_eq!(w["alwaysOnTop"], false);
    app.eval(
        "main",
        "document.querySelector('[data-local=books] button.open').click(); null",
    );
    until(&app, "main", "true");
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let windows = app.windows();
        assert_eq!(
            windows.iter().filter(|w| w["label"] == "mod-books").count(),
            1
        );
        if windows
            .iter()
            .any(|w| w["label"] == "mod-books" && w["focused"] == true)
        {
            break;
        }
        assert!(Instant::now() < deadline);
    }
    app.eval("main","window.__TAURI_INTERNALS__.invoke('mod_open',{local:'logic'}).catch(e=>{document.body.dataset.code=e.code}); null");
    until(&app, "main", "document.body.dataset.code === 'not_found'");
    app.eval(
        "main",
        "document.querySelector('[data-local=books] button.disable').click(); null",
    );
    until(
        &app,
        "main",
        "document.querySelector('[data-local=books] button.disable').disabled",
    );
    assert!(!app.windows().iter().any(|w| w["label"] == "mod-books"));
    assert!(
        app.log()["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["cmd"] == "mod_disable")
    );
    assert!(app.quit().success());
}
#[test]
fn ac3_closing_consent_denies_and_destroys_it() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    let ask = app.ask(install("Books"));
    consent(&app);
    app.close("consent");
    assert_eq!(ask.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
    assert!(app.quit().success());
}
#[test]
fn ac4_approve_delay_and_deny_focus() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    let ask = app.ask(install("Books"));
    consent(&app);
    let t = Instant::now();
    assert_eq!(app.eval("consent", "document.activeElement.id"), "deny");
    assert_eq!(
        app.eval("consent", "document.querySelectorAll('form').length"),
        0
    );
    std::thread::sleep((t + Duration::from_millis(100)).saturating_duration_since(Instant::now()));
    assert_eq!(app.eval("consent","document.querySelector('#approve').click(); document.querySelector('#approve').disabled"),true);
    assert!(ask.is_open());
    std::thread::sleep((t + Duration::from_millis(900)).saturating_duration_since(Instant::now()));
    app.eval(
        "consent",
        "document.querySelector('#approve').click(); null",
    );
    assert_eq!(
        ask.wait(TIMEOUT),
        Some(json!({"answer":"approve","withheld":[]}))
    );
    assert!(app.quit().success());
}
#[test]
fn ac5_install_and_each_statement_show_fields_and_withhold() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    let mut request = install("Local Books");
    request["by_program"] = json!(true);
    let mut published = request["mods"][0].clone();
    published["local"] = json!("published");
    published["name"] = json!("Published Books");
    published["release"] = json!("publisher/books@1.2.3");
    published["publisher"] = json!({"name":"Alice","fingerprint":"abcd1234"});
    published["replaces"] = json!("1.0.0");
    published["uses"] = json!(["provider (local)"]);
    published["grants"] = json!([
 {"key":"storage","kind":"storage","text":"Keep its own data on this device","granted":true,"new":true},
 {"key":"http example.org","kind":"http","text":"Connect to example.org","granted":true,"new":false},
 {"key":"call books","kind":"call","text":"Call books of provider","granted":true,"new":false},
 {"key":"subscribe books","kind":"subscribe","text":"Receive the books events of provider","granted":true,"new":false}]);
    request["mods"].as_array_mut().unwrap().push(published);
    let ask = app.ask(request);
    consent(&app);
    let text = app
        .eval("consent", "document.body.textContent")
        .as_str()
        .unwrap()
        .to_string();
    for field in [
        "Install 2 mod(s)",
        "Asked by a program on this device",
        "Local Books",
        "local",
        "Published Books",
        "1.2.3",
        "publisher/books@1.2.3",
        "Alice",
        "abcd1234",
        "signature verified",
        "replaces 1.0.0",
        "Keep its own data on this device",
        "Connect to example.org",
        "Call books of provider",
        "Receive the books events of provider",
        "new",
        "uses provider (local)",
    ] {
        assert!(text.contains(field), "missing {field}: {text}");
    }
    assert!(
        text.contains(
            std::fs::read_to_string(home.path().join("app/mark"))
                .unwrap()
                .trim_end()
        )
    );
    assert_eq!(
        app.eval(
            "consent",
            "Array.from(document.querySelectorAll('input[type=checkbox]'),e=>e.checked)"
        ),
        json!([true, true, true, true])
    );
    app.eval(
        "consent",
        "document.querySelector('input[type=checkbox]').checked=false; null",
    );
    until(
        &app,
        "consent",
        "!document.querySelector('#approve').disabled",
    );
    app.eval(
        "consent",
        "document.querySelector('#approve').click(); null",
    );
    assert_eq!(
        ask.wait(TIMEOUT),
        Some(json!({"answer":"approve","withheld":[{"local":"published","key":"storage"}]}))
    );
    let deadline = Instant::now() + TIMEOUT;
    while app.windows().iter().any(|w| w["label"] == "consent") {
        assert!(
            Instant::now() < deadline,
            "answered consent window stayed open"
        );
    }
    for (statement, title, details) in [
        (
            "profile",
            "Sign your profile",
            json!([{"label":"Name","value":"Alice"}]),
        ),
        (
            "release",
            "Sign a release",
            json!([{"label":"Release","value":"publisher/books@1.2.3"},{"label":"Package","value":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},{"label":"Size","value":"1024"},{"label":"Source included","value":"yes"},{"label":"Summary","value":"Reading books"}]),
        ),
        (
            "withdraw",
            "Sign a withdrawal",
            json!([{"label":"Release","value":"publisher/books@1.2.3"},{"label":"Reason","value":"Broken release"}]),
        ),
    ] {
        let request = json!({"kind":"sign","statement":statement,"signer":{"name":"Alice","fingerprint":"abcd1234"},"details":details});
        let ask = app.ask(request);
        consent(&app);
        let text = app
            .eval("consent", "document.body.textContent")
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains(title));
        assert!(text.contains("Alice"));
        assert!(text.contains("abcd1234"));
        for detail in details.as_array().unwrap() {
            assert!(text.contains(detail["label"].as_str().unwrap()));
            assert!(text.contains(detail["value"].as_str().unwrap()));
        }
        assert!(
            text.contains(
                std::fs::read_to_string(home.path().join("app/mark"))
                    .unwrap()
                    .trim_end()
            )
        );
        app.close("consent");
        assert_eq!(ask.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    }
    assert!(app.quit().success());
}
#[test]
fn ac6_fifo_and_quit_denies_open_request() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    let first = app.ask(install("First"));
    let second = app.ask(install("Second"));
    consent(&app);
    assert_eq!(app.eval("consent","document.body.textContent.includes('First') && !document.body.textContent.includes('Second')"),true);
    app.close("consent");
    assert_eq!(first.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    consent(&app);
    assert_eq!(
        app.eval("consent", "document.body.textContent.includes('Second')"),
        true
    );
    assert!(second.is_open());
    assert!(app.quit().success());
    assert_eq!(second.wait(TIMEOUT), Some(json!({"answer":"deny"})));
}
#[test]
fn ac7_untrusted_name_is_literal_text() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let name = "<img src=x onerror=alert(1)>";
    let home = tempfile::tempdir().unwrap();
    let fake = json!({"mods":[mod_view("evil",name,Value::Null)]}).to_string();
    let app = App::start(home.path(), &[("WATANE_APP_FAKE", &fake)]);
    until(&app, "main", "document.body.dataset.ready === 'true'");
    assert_eq!(
        app.eval(
            "main",
            "document.querySelector('[data-local] h2').textContent"
        ),
        name
    );
    assert_eq!(
        app.eval("main", "document.querySelectorAll('img').length"),
        0
    );
    let ask = app.ask(install(name));
    consent(&app);
    assert_eq!(
        app.eval("consent", "document.querySelector('main h2').textContent"),
        name
    );
    assert_eq!(
        app.eval("consent", "document.querySelectorAll('img').length"),
        0
    );
    app.close("consent");
    assert_eq!(ask.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    assert!(app.quit().success());
}
