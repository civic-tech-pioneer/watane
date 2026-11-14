#![cfg(feature = "driver")]
mod support;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::{driver::App, journey::*};
static GUI: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn both() -> Vec<std::path::PathBuf> {
    vec![fixture("recommendations"), fixture("reading-list")]
}
fn plan(app: &App) {
    let text = app.eval("consent", "document.body.textContent");
    let text = text.as_str().unwrap();
    assert!(text.find("Reading list").unwrap() < text.find("Recommendations").unwrap());
    assert_eq!(app.eval("consent", "Array.from(document.querySelectorAll('main section'), e => e.textContent.includes('local'))"), json!([true, true]));
    for line in [
        "Keep its own data on this device",
        "Call mods:reading-list/books@0.1.0 of reading-list (local)",
        "Receive the mods:reading-list/events@0.1.0 events of reading-list (local)",
        "uses reading-list (local)",
    ] {
        assert!(text.contains(line), "{line}: {text}");
    }
}
#[test]
fn ac4_s2_install_order_and_s2b_withhold_storage() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    begin_install(&app, &both());
    plan(&app);
    app.eval(
        "consent",
        "document.querySelector('input[type=checkbox]').checked = false; null",
    );
    approve(&app);
    for local in ["reading-list", "recommendations"] {
        installed(&app, local);
    }
    assert_eq!(
        std::fs::read_dir(home.path().join("mods")).unwrap().count(),
        2
    );
    until(
        &app,
        "main",
        "document.querySelector('[data-local=reading-list] [data-key=storage]').textContent.includes('Not granted')",
    );
    assert!(app.quit().success());
}
#[test]
fn ac7_idle_polling_disabled_entry_and_consent_focus() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    // WebKit reports `document.hidden` for a hidden or occluded window (NOTES S12a-2);
    // the poll gate reads it, so the test declares the main page visible.
    let app = App::start(home.path(), &[]);
    begin_install(&app, &both());
    approve(&app);
    installed(&app, "reading-list");
    open(&app, "reading-list");
    let count = |log: Value| {
        log["commands"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["label"] == "main" && c["cmd"] == "mods_list")
            .count()
    };
    app.eval(
        "main",
        "Object.defineProperty(document, 'hidden', {get: () => false}); null",
    );
    let before = count(app.log());
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        std::thread::yield_now();
    }
    assert!(count(app.log()) >= before + 2);
    let ask = app.ask(json!({"kind":"install", "mods":[], "by_program":false}));
    assert!(app.wait("consent", TIMEOUT));
    let focus = app
        .windows()
        .into_iter()
        .find(|w| w["label"] == "consent")
        .unwrap()["focused"]
        .clone();
    invoke(&app, "mod_open", json!({"local":"reading-list"}));
    assert_eq!(
        app.windows()
            .into_iter()
            .find(|w| w["label"] == "consent")
            .unwrap()["focused"],
        focus
    );
    app.close("consent");
    assert_eq!(ask.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    app.eval(
        "main",
        "document.querySelector('[data-local=reading-list] button.disable').click(); null",
    );
    until(
        &app,
        "main",
        "document.querySelector('[data-local=reading-list]').textContent.includes('Disabled') && !!document.querySelector('[data-local=reading-list] button.enable') && !document.querySelector('[data-local=reading-list] button.open')",
    );
    app.eval(
        "main",
        "document.querySelector('[data-local=reading-list] button.enable').click(); null",
    );
    until(
        &app,
        "main",
        "!!document.querySelector('[data-local=reading-list] button.open')",
    );
    assert!(app.quit().success());
}
#[test]
fn ac8_remove_defaults_to_keep_data_and_unknown_keys_fail() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    begin_install(&app, &both());
    approve(&app);
    installed(&app, "recommendations");
    app.eval(
        "main",
        "document.querySelector('[data-local=recommendations] button.remove').click(); null",
    );
    until(
        &app,
        "main",
        "document.querySelector('#remove-dialog').open",
    );
    assert_eq!(
        app.eval(
            "main",
            "document.querySelector('#remove-dialog p').textContent"
        ),
        "Delete the data of Recommendations?"
    );
    assert_eq!(
        app.eval(
            "main",
            "Array.from(document.querySelectorAll('#remove-dialog button'), b => b.textContent)"
        ),
        json!(["Keep data", "Delete data"])
    );
    assert_eq!(app.eval("main", "document.activeElement.id"), "keep-data");
    app.eval("main", "document.querySelector('#keep-data').click(); null");
    until(
        &app,
        "main",
        "!document.querySelector('[data-local=recommendations]')",
    );
    for cmd in ["grant_give", "grant_withdraw"] {
        app.eval("main", &format!("window.__badKey = null; window.__TAURI_INTERNALS__.invoke({}, {{local:'reading-list',key:'unknown'}}).catch(e => window.__badKey = e.code); null", json!(cmd)));
        until(&app, "main", "window.__badKey === 'not_found'");
    }
    assert!(app.quit().success());
}
fn book(app: &App, title: &str) {
    app.eval("mod-reading-list", &format!("document.querySelector('#title').value = {}; document.querySelector('#add').click(); null", json!(title)));
    until(
        app,
        "mod-reading-list",
        &format!(
            "document.querySelector('#books')?.textContent.includes({})",
            json!(title)
        ),
    );
}
#[test]
#[ignore = "slow: journey J2"]
fn ac5_journey_j2() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    begin_install(&app, &both());
    plan(&app);
    approve(&app);
    for local in ["reading-list", "recommendations"] {
        installed(&app, local);
        open(&app, local);
    }
    book(&app, "Dune");
    until(
        &app,
        "mod-recommendations",
        "document.querySelector('#books')?.textContent.includes('Dune')",
    );
    assert!(app.quit().success());
    let app = App::start(home.path(), &[]);
    for local in ["reading-list", "recommendations"] {
        installed(&app, local);
        open(&app, local);
        until(
            &app,
            &format!("mod-{local}"),
            "document.querySelector('#books')?.textContent.includes('Dune')",
        );
    }
    let key = "call mods:reading-list/books@0.1.0 from local/reading-list";
    invoke(
        &app,
        "grant_withdraw",
        json!({"local":"recommendations","key":key}),
    );
    book(&app, "Foundation");
    until(
        &app,
        "mod-recommendations",
        "document.body.textContent.includes('not granted')",
    );
    invoke(
        &app,
        "grant_give",
        json!({"local":"recommendations","key":key}),
    );
    assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
    until(
        &app,
        "mod-recommendations",
        "document.querySelector('#books')?.textContent.includes('Foundation') && !document.querySelector('#error')?.textContent",
    );
    // S11 through the agent controls.
    app.eval("main", "document.querySelector('#toolset-name').value = 'reader'; document.querySelector('input[value=\"reading-list:books.list\"]').checked = true; document.querySelector('#save-toolset').click(); null");
    until(
        &app,
        "main",
        "document.querySelector('#toolsets').textContent.includes('reader')",
    );
    // AC2/AC3: labels and saved entries use the same name as tools/call.
    let tool_name = "reading-list_books_list";
    let text = app.eval("main", "document.body.textContent");
    assert!(text.as_str().unwrap().contains(tool_name));
    assert!(!text.as_str().unwrap().contains("reading-list:books.list"));
    assert_eq!(
        app.eval("main", "document.querySelector('input[value=\"reading-list:books.list\"]').parentElement.textContent"),
        tool_name
    );
    assert!(
        app.eval("main", "document.querySelector('#toolsets').textContent")
            .as_str()
            .unwrap()
            .contains(tool_name)
    );
    assert_eq!(
        invoke(&app, "tools_list", json!({}))
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["entry"] == "reading-list:books.list")
            .unwrap()["tool"],
        tool_name
    );
    app.eval("main", "document.querySelector('#token-toolset').value = 'reader'; document.querySelector('#handle').value = 'claude'; document.querySelector('#issue-token').click(); null");
    until(
        &app,
        "main",
        "!!document.querySelector('#token-secret').textContent",
    );
    let token = app.eval(
        "main",
        "document.querySelector('#token-secret').textContent",
    );
    let url = app.eval("main", "document.querySelector('#mcp-url').textContent");
    app.eval(
        "main",
        "document.querySelector('#close-token').click(); null",
    );
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(TIMEOUT)
        .build()
        .unwrap();
    let call = || {
        client.post(url.as_str().unwrap()).bearer_auth(token.as_str().unwrap()).json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":tool_name,"arguments":{}}})).send().unwrap()
    };
    let result: Value = call().json().unwrap();
    assert_eq!(result["result"]["isError"], false);
    assert!(result.to_string().contains("Foundation"));
    until(&app, "main", "!!document.querySelector('#tokens button')");
    app.eval(
        "main",
        "document.querySelector('#tokens button').click(); null",
    );
    until(&app, "main", "!document.querySelector('#tokens button')");
    assert_eq!(call().status(), 401);
    // S58 AC1: remove a second toolset through its saved row.
    app.eval("main", "document.querySelector('#toolset-name').value = 'temporary'; document.querySelector('#save-toolset').click(); null");
    until(
        &app,
        "main",
        "Array.from(document.querySelectorAll('#token-toolset option')).some(o => o.value === 'temporary')",
    );
    app.eval("main", "Array.from(document.querySelectorAll('#toolsets p')).find(row => row.textContent.startsWith('temporary:')).querySelector('.remove-toolset').click(); null");
    until(
        &app,
        "main",
        "!document.querySelector('#toolsets').textContent.includes('temporary') && !Array.from(document.querySelectorAll('#token-toolset option')).some(o => o.value === 'temporary')",
    );
    assert!(
        app.eval(
            "main",
            "document.querySelector('#toolsets').textContent.includes('reader')"
        )
        .as_bool()
        .unwrap()
    );
    app.eval("main", "window.__removeError = null; window.__TAURI_INTERNALS__.invoke('toolset_remove', {name:'missing'}).then(() => window.__removeError = {code:'unexpected_success'}, e => window.__removeError = e); null");
    until(&app, "main", "window.__removeError !== null");
    assert_eq!(app.eval("main", "window.__removeError.code"), "no_toolset");
    // S10: prove recommendations' own received-event state is populated.
    until(
        &app,
        "mod-recommendations",
        "JSON.parse(document.querySelector('#books').textContent)[1].includes('Foundation')",
    );
    app.eval(
        "main",
        "document.querySelector('[data-local=recommendations] button.remove').click(); null",
    );
    until(
        &app,
        "main",
        "document.querySelector('#remove-dialog').open",
    );
    app.eval(
        "main",
        "document.querySelector('#delete-data').click(); null",
    );
    until(
        &app,
        "main",
        "!document.querySelector('[data-local=recommendations]')",
    );
    assert!(!home.path().join("mods/recommendations").exists());
    begin_install(&app, &[fixture("recommendations")]);
    approve(&app);
    installed(&app, "recommendations");
    open(&app, "recommendations");
    until(
        &app,
        "mod-recommendations",
        "document.querySelector('#books')?.textContent.endsWith(',[]]')",
    );
    assert!(app.quit().success());
}
