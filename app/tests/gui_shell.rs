#![cfg(feature = "driver")]
mod support;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::{
    driver::App,
    journey::{approve, begin_install, copy_dir, fixture, installed},
};
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
    assert_eq!(
        app.eval("main", "document.body.textContent.includes('No mods yet')"),
        true
    );
    assert_ne!(app.port, 0);
    let response = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(TIMEOUT)
        .build()
        .unwrap()
        .get(format!("http://localhost:{}/api/status", app.port))
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(app.quit().success());
}

fn install(name: &str) -> Value {
    json!({"kind":"install","by_program":false,"mods":[{"local":"books","name":name,"slug":"books","version":"1.2.3","release":null,"publisher":null,"replaces":null,"grants":[],"uses":[]}]})
}
fn consent(app: &App) {
    assert!(app.wait("consent", TIMEOUT));
    until(app, "consent", "document.body.dataset.ready === 'true'");
    assert!(
        app.windows()
            .iter()
            .any(|w| w["label"] == "consent" && w["alwaysOnTop"] == true)
    );
    assert_eq!(
        app.eval("consent", "document.querySelectorAll('form').length"),
        0
    );
}
fn install_books(app: &App, name: &str, with_logic: bool) {
    let copied = tempfile::tempdir().unwrap();
    let books = copied.path().join("books");
    std::fs::create_dir_all(books.join("wit")).unwrap();
    std::fs::create_dir(books.join("ui")).unwrap();
    std::fs::write(
        books.join("mod.toml"),
        format!(
            "name = {}\nsummary = \"Books fixture\"\n",
            serde_json::to_string(name).unwrap()
        ),
    )
    .unwrap();
    std::fs::write(
        books.join("wit/world.wit"),
        "package mods:books@1.2.3; world books {} ",
    )
    .unwrap();
    std::fs::write(
        books.join("ui/index.html"),
        "<!doctype html><title>Books</title><p>Books page</p>",
    )
    .unwrap();
    let mut paths = vec![books];
    if with_logic {
        let logic = copied.path().join("logic");
        copy_dir(&fixture("reading-list"), &logic);
        std::fs::remove_dir_all(logic.join("ui")).unwrap();
        std::fs::write(
            logic.join("mod.toml"),
            "name = \"Logic\"\nsummary = \"Logic fixture\"\n",
        )
        .unwrap();
        paths.push(logic);
    }
    begin_install(app, &paths);
    approve(app);
    installed(app, "books");
    if with_logic {
        installed(app, "reading-list");
    }
}
#[test]
fn ac2_real_catalog_opens_one_window_and_no_page_is_not_found() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    install_books(&app, "Books", true);
    let url = format!("http://books.localhost:{}/", app.port);
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
            "document.querySelector('[data-local=reading-list] button.open') === null"
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
    assert_eq!(w["visible"], false);
    assert_eq!(w["focused"], false);
    app.eval("main", "window.__reopened = false; window.__TAURI_INTERNALS__.invoke('mod_open', {local:'books'}).then(() => { window.__reopened = true; }); null");
    until(&app, "main", "window.__reopened === true");
    let windows = app.windows();
    assert_eq!(
        windows.iter().filter(|w| w["label"] == "mod-books").count(),
        1
    );
    assert_eq!(
        windows.iter().find(|w| w["label"] == "mod-books").unwrap()["focused"],
        false
    );
    app.eval("main","window.__TAURI_INTERNALS__.invoke('mod_open',{local:'reading-list'}).catch(e=>{document.body.dataset.code=e.code}); null");
    until(&app, "main", "document.body.dataset.code === 'not_found'");
    app.eval(
        "main",
        "document.querySelector('[data-local=books] button.disable').click(); null",
    );
    until(
        &app,
        "main",
        "document.querySelector('[data-local=books] button.disable')?.disabled === true",
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
#[ignore = "front: takes the screen"]
fn ac4_approve_delay_and_deny_focus() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::front(home.path(), &[]);
    let ask = app.ask(install("Books"));
    consent(&app);
    until(&app, "consent", "document.hasFocus()");
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
        "Install 2 mods",
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
    // This tests fields and withholding; real focus and the gate belong to ac4.
    app.eval(
        "consent",
        "document.querySelector('#approve').disabled = false; null",
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
    let app = App::start(home.path(), &[]);
    install_books(&app, name, false);
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

#[test]
fn ac1_hidden_screen_loads_evaluates_and_runs_timers() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[("WATANE_DRIVER_SCREEN", "front")]);
    let windows = app.windows();
    let main = windows.iter().find(|w| w["label"] == "main").unwrap();
    assert_eq!(main["visible"], false);
    assert_eq!(main["focused"], false);
    assert_eq!(
        app.screen(),
        json!({"mode":"hidden","accessory":cfg!(target_os = "macos")})
    );
    let ask = app.ask(install("Timer probe"));
    consent(&app);
    let windows = app.windows();
    let consent = windows.iter().find(|w| w["label"] == "consent").unwrap();
    assert_eq!(consent["visible"], false);
    assert_eq!(consent["focused"], false);
    // Probe the WKWebView scheduler independently of the intentional focus gate.
    app.eval("consent", "window.__timerProbe = null; const begin = performance.now(); setTimeout(() => { window.__timerProbe = performance.now() - begin; }, 800); null");
    until(&app, "consent", "window.__timerProbe !== null");
    let elapsed = app.eval("consent", "window.__timerProbe").as_f64().unwrap();
    assert!(
        (750.0..2000.0).contains(&elapsed),
        "hidden timer took {elapsed} ms"
    );
    app.close("consent");
    assert_eq!(ask.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    assert!(app.quit().success());
}

#[test]
fn ac2_background_screen_is_visible_without_focus() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::background(home.path(), &[("WATANE_DRIVER_SCREEN", "front")]);
    let windows = app.windows();
    let main = windows.iter().find(|w| w["label"] == "main").unwrap();
    assert_eq!(main["visible"], true);
    assert_eq!(main["focused"], false);
    assert_eq!(
        app.screen(),
        json!({"mode":"background","accessory":cfg!(target_os = "macos")})
    );
    assert!(app.quit().success());
}

#[test]
#[ignore = "front: takes the screen"]
fn ac3_reopening_an_open_mod_focuses_it() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::front(home.path(), &[]);
    install_books(&app, "Books", false);
    until(&app, "main", "document.body.dataset.ready === 'true'");
    app.eval(
        "main",
        "document.querySelector('[data-local=books] button.open').click(); null",
    );
    assert!(app.wait("mod-books", TIMEOUT));
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
    assert!(app.quit().success());
}

#[test]
#[ignore = "front: takes the screen"]
fn ac3_consent_takes_focus() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::front(home.path(), &[("WATANE_DRIVER_SCREEN", "hidden")]);
    assert_eq!(app.screen(), json!({"mode":"front","accessory":false}));
    let ask = app.ask(install("Books"));
    consent(&app);
    until(&app, "consent", "document.hasFocus()");
    assert!(
        app.windows()
            .iter()
            .any(|w| w["label"] == "consent" && w["alwaysOnTop"] == true && w["focused"] == true)
    );
    app.close("consent");
    assert_eq!(ask.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    assert!(app.quit().success());
}

#[test]
fn ac1_busy_port_shows_recovery_in_main_status_and_stderr() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let home = tempfile::tempdir().unwrap();
    let app = App::start_port(home.path(), port);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    let expected = format!(
        "Port {port} is in use, maybe by another watane app. Start this one with --port <n> or WATANE_PORT=<n> to use another port, or --port 0 for any free port."
    );
    assert_eq!(
        app.eval("main", "document.querySelector('main').textContent"),
        expected
    );
    app.eval("main", "window.__status = null; window.__TAURI_INTERNALS__.invoke('status').then(s => window.__status = s); null");
    until(&app, "main", "window.__status !== null");
    assert_eq!(app.eval("main", "window.__status.error"), expected);
    assert!(!home.path().join("run.json").exists());
    assert_eq!(
        app.stderr()
            .lines()
            .filter(|line| *line == expected)
            .count(),
        1
    );
    assert!(app.quit().success());
}

#[test]
fn ac4_single_install_and_update_headings() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    for (replaces, expected) in [
        (Value::Null, "Install Books"),
        (json!("1.0.0"), "Update Books to 1.2.3"),
    ] {
        let mut request = install("Books");
        request["mods"][0]["replaces"] = replaces;
        let ask = app.ask(request);
        consent(&app);
        assert_eq!(
            app.eval("consent", "document.querySelector('h1').textContent"),
            expected
        );
        app.close("consent");
        assert_eq!(ask.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    }
    assert!(app.quit().success());
}

#[test]
fn ac2_own_withdrawal_and_ac6_offer_button_words() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    // Exercise the same UI-only word functions used by the launcher without
    // replacing Tauri's immutable invoke property.
    app.eval("main", r#"window.__wordsResult = null;
      import(new URL('words.js', document.baseURI).href).then(words => {
        window.__wordsResult = {ok: [
          words.ownWithdrawal('breaks lists'),
          words.offerLabel('publisher/books@2.0.0', []),
          words.offerLabel('publisher/books@2.0.0', [{text:'Connect to example.org'}]),
          words.offerLabel('publisher/books@2.0.0', [{text:'Connect to example.org'}, {text:'Keep its own data on this device'}])
        ]};
      }).catch(error => { window.__wordsResult = {error: String(error)}; }); null"#);
    until(&app, "main", "window.__wordsResult !== null");
    let result = app.eval("main", "window.__wordsResult");
    assert!(
        result.get("error").is_none(),
        "UI words import failed: {result}"
    );
    assert_eq!(result["ok"][0], "Withdrawn: breaks lists");
    assert_eq!(result["ok"][1], "Install 2.0.0");
    assert_eq!(
        result["ok"][2],
        "Install 2.0.0 — asks for: Connect to example.org"
    );
    assert_eq!(
        result["ok"][3],
        "Install 2.0.0 — asks for: Connect to example.org, Keep its own data on this device"
    );
    assert!(app.quit().success());
}

#[test]
fn ac7_real_installed_version_is_a_name_sibling_outside_details() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    install_books(&app, "Books", false);
    assert_eq!(
        app.eval(
            "main",
            "document.querySelector('[data-local=books] > h2').textContent"
        ),
        "Books"
    );
    assert_eq!(
        app.eval(
            "main",
            "document.querySelector('[data-local=books] > h2 + .version')?.textContent ?? null"
        ),
        "1.2.3"
    );
    assert_eq!(
        app.eval(
            "main",
            "document.querySelector('[data-local=books] > details').open"
        ),
        false
    );
    assert!(app.quit().success());
}

#[test]
fn ac4b_index_statement_titles() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    for (statement, title) in [
        ("list", "List in your index"),
        ("include", "Include in your index"),
        ("unlist", "End a listing"),
    ] {
        let ask = app.ask(json!({"kind":"sign", "statement":statement, "signer":{"name":"Carol", "fingerprint":"abcd1234"}, "details":[]}));
        consent(&app);
        assert_eq!(
            app.eval("consent", "document.querySelector('h1').textContent"),
            title
        );
        app.close("consent");
        assert_eq!(ask.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    }
    assert!(app.quit().success());
}

#[test]
fn ac4b_index_title_words() {
    let script = format!(
        "{}\nfor (const [kind, title] of [['list','List in your index'],['include','Include in your index'],['unlist','End a listing']]) {{ if (signTitle(kind) !== title) throw Error(kind); }}",
        include_str!("../ui/words.js")
    );
    assert!(
        std::process::Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .status()
            .unwrap()
            .success()
    );
}
