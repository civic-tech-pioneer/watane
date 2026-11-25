//! Real Settings journeys for agent accounts (TECHNICAL 19.6).

#![cfg(all(feature = "driver", unix))]
#![allow(
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "test fixtures and the shared lock-only GUI mutex"
)]

#[allow(
    dead_code,
    reason = "the issuer fault fixtures are exercised in unit tests"
)]
#[path = "../support/issuer.rs"]
mod issuer;

use crate::support::GUI;
use crate::support::driver::App;
use crate::support::journey::{
    TIMEOUT, approve, begin_install, copy_dir, deny, fixture, installed, invoke, open, until,
};
use crate::support::manage::host_fixture;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn settings(home: &Path, env: &[(&str, &str)]) -> App {
    let app = App::start(home, env);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    app.menu("settings");
    until(
        &app,
        "main",
        "location.hash === '#settings' && !document.querySelector('#settings').hidden",
    );
    app
}

fn click(app: &App, selector: &str) {
    app.eval(
        "main",
        &format!("document.querySelector({}).click(); null", json!(selector)),
    );
}

fn text_is(app: &App, selector: &str, expected: &str) {
    until(
        app,
        "main",
        &format!(
            "document.querySelector({}).textContent.trim() === {}",
            json!(selector),
            json!(expected)
        ),
    );
}

fn stand_in_browser(app: &App) -> String {
    let end = Instant::now() + TIMEOUT;
    loop {
        let url = app.open_url();
        if let Some(url) = url.as_str() {
            return url.to_owned();
        }
        assert_eq!(url, Value::Null);
        assert!(Instant::now() < end, "the app opened no browser URL");
        std::thread::yield_now();
    }
}

#[test]
#[ignore = "slow: the installed webview command and callback change the visible Settings account"]
fn ac1_ac2_ac4_s54_s57_chatgpt_sign_in_and_out_in_settings() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let issuer = issuer::Issuer::start();
    let home = tempfile::tempdir().unwrap();
    let app = settings(home.path(), &[("WATANE_APP_OPENAI_ISSUER", &issuer.url)]);
    let account = home.path().join("app/accounts/openai.json");
    text_is(&app, "#chatgpt-status", "Not signed in");
    click(&app, "#chatgpt-sign-in");
    let url = stand_in_browser(&app);
    assert!(url.starts_with(&format!("{}/", issuer.url)), "{url}");
    app.eval(
        "main",
        "window.__secondSignIn = null; window.__TAURI_INTERNALS__.invoke('chatgpt_sign_in').then(() => window.__secondSignIn = 'success', error => window.__secondSignIn = error.code); null",
    );
    until(&app, "main", "window.__secondSignIn !== null");
    assert_eq!(app.eval("main", "window.__secondSignIn"), "unavailable");
    issuer.follow(&url);
    text_is(&app, "#chatgpt-status", "Signed in with ChatGPT · plus");
    let saved: Value = serde_json::from_slice(&fs::read(&account).unwrap()).unwrap();
    assert_eq!(saved["version"], 1);
    assert_eq!(saved["plan"], "plus");
    assert_eq!(
        fs::metadata(&account).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let status = invoke(&app, "status", json!({}));
    assert_eq!(
        status["chatgpt"],
        json!({"signedIn": true, "plan": "plus", "error": null})
    );
    for (field, token) in [
        ("access_token", "stand-in-access"),
        ("refresh_token", "stand-in-refresh"),
    ] {
        assert_eq!(saved[field], token);
        assert!(!status.to_string().contains(token));
    }

    click(&app, "#chatgpt-sign-out");
    text_is(&app, "#chatgpt-status", "Not signed in");
    assert!(!account.exists());
    assert_eq!(
        invoke(&app, "status", json!({}))["chatgpt"],
        json!({"signedIn": false, "plan": null, "error": null})
    );
    assert!(app.quit().success());
}

fn fake_claude(directory: &Path, version: &str) -> PathBuf {
    let file = directory.join(format!("claude-{version}"));
    fs::write(
        &file,
        format!("#!/bin/sh\n[ \"$#\" = 1 ] && [ \"$1\" = --version ] || exit 1\nprintf '%s\\n' '{version} (Claude Code)'\n"),
    )
    .unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o700)).unwrap();
    file.canonicalize().unwrap()
}

#[test]
#[ignore = "slow: the installed webview command connects the picker and the visible Settings choice"]
fn ac3_s61_claude_picker_refuses_old_version_and_keeps_valid_choice() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let old = fake_claude(bin.path(), "2.1.294");
    let minimum = fake_claude(bin.path(), "2.1.295");
    let app = settings(home.path(), &[]);
    let path = home.path().join("app/claude.json");
    let error = "Claude Code 2.1.294 is too old; watane needs 2.1.295 or later.";
    app.pick(std::slice::from_ref(&old));
    click(&app, "#claude-choose");
    text_is(&app, "#claude-error", error);
    assert!(!path.exists());
    assert!(invoke(&app, "status", json!({}))["claude"].is_null());

    app.pick(std::slice::from_ref(&minimum));
    click(&app, "#claude-choose");
    text_is(&app, "#claude-path", minimum.to_str().unwrap());
    text_is(&app, "#claude-status", "Claude Code 2.1.295");
    text_is(&app, "#claude-error", "");
    let stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored, json!({"version": 1, "path": minimum}));

    app.pick(std::slice::from_ref(&old));
    click(&app, "#claude-choose");
    text_is(&app, "#claude-error", error);
    text_is(&app, "#claude-path", minimum.to_str().unwrap());
    assert_eq!(
        invoke(&app, "status", json!({}))["claude"],
        json!({"path": minimum, "version": "2.1.295"})
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        stored
    );
    assert!(app.quit().success());

    let app = settings(home.path(), &[]);
    text_is(&app, "#claude-path", minimum.to_str().unwrap());
    text_is(&app, "#claude-status", "Claude Code 2.1.295");
    click(&app, "#claude-clear");
    text_is(&app, "#claude-status", "No program chosen");
    assert!(!path.exists());
    assert!(app.quit().success());
}

const INIT: &str = r#"{"type":"system","subtype":"init","tools":[],"mcp_servers":[],"agents":[],"permissionMode":"default","plugins":[{"name":"watane-run"}]}"#;
const GRANT_CLAUDE: &str = "Can start Claude Code with your Claude account, to read and change files in folders you choose. Not checked by watane.";
const GRANT_SHELL: &str = "Can start Claude Code with your Claude account and let it run programs and use the web, inside Claude's sandbox. Not checked by watane.";

/// A running app with host's fake `claude` chosen and host's `claude` (with a page added) and
/// `claude-front` fixtures installed, the page of `claude` open.
struct Claude {
    app: App,
    home: tempfile::TempDir,
    _work: tempfile::TempDir,
}
fn claude_app() -> Claude {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let (fake, claude) = (work.path().join("bin"), work.path().join("mods/claude"));
    fs::create_dir(&fake).unwrap();
    let program = fake.join("claude");
    let script = host_fixture("../fake-claude/claude");
    fs::copy(script, &program).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(fake.join("init"), format!("{INIT}\n")).unwrap();
    copy_dir(&host_fixture("claude"), &claude);
    fs::create_dir(claude.join("ui")).unwrap();
    let page = r#"<!doctype html><title>Claude</title><script src="/watane.js"></script>"#;
    fs::write(claude.join("ui/index.html"), page).unwrap();
    let app = App::start(home.path(), &[]);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    app.pick(&[program.canonicalize().unwrap()]);
    invoke(&app, "claude_binary_pick", json!({}));
    begin_install(&app, &[claude, host_fixture("claude-front")]);
    // The plan of the consumer of a Claude Code provider says so (S60).
    assert_eq!(
        app.eval(
            "consent",
            "[...document.querySelectorAll('section p')].map(e => e.textContent).filter(t => t.startsWith('Uses Claude Code'))"
        ),
        json!(["Uses Claude Code through claude (local)."])
    );
    approve(&app);
    installed(&app, "claude");
    installed(&app, "claude-front");
    open(&app, "claude");
    Claude {
        app,
        home,
        _work: work,
    }
}
impl Claude {
    fn page(&self, function: &str, args: Value) -> Value {
        self.app.eval(
            "mod-claude",
            &format!("window.__r = null; watane.call({}, {args}).then(v => window.__r = {{ok: v}}, e => window.__r = {{error: String(e.message || e)}}); null", json!(function)),
        );
        until(&self.app, "mod-claude", "window.__r !== null");
        let result = self.app.eval("mod-claude", "window.__r");
        assert!(result.get("error").is_none(), "{function}: {result}");
        result["ok"].clone()
    }
    fn start(&self, folder: &str) -> String {
        let launch = json!({"version": 1, "workspace": folder, "profile": "coder"}).to_string();
        let s = self.page("api.start", json!({"launch": launch}));
        assert_eq!(s.as_str().unwrap().len(), 32, "no stream id: {s}");
        s.as_str().unwrap().to_owned()
    }
    fn items(&self, s: &str) -> Vec<String> {
        serde_json::from_value(self.page("api.items", json!({"s": s}))).unwrap()
    }
    fn ended(&self) -> Vec<String> {
        serde_json::from_value(self.page("api.ended", Value::Null)).unwrap()
    }
    fn until_items(&self, s: &str, what: &str, test: impl Fn(&[String]) -> bool) {
        let end = Instant::now() + TIMEOUT;
        while !test(&self.items(s)) {
            assert!(Instant::now() < end, "{what}: {:?}", self.items(s));
            std::thread::yield_now();
        }
    }
    fn until_ended(&self, line: &str) {
        let end = Instant::now() + TIMEOUT;
        while !self.ended().iter().any(|e| e == line) {
            assert!(Instant::now() < end, "{line}: {:?}", self.ended());
            std::thread::yield_now();
        }
    }
    /// The consent view shows `question` (the call that began it has returned).
    fn view(&self, title: &str, question: &str) {
        assert!(
            self.app.wait("consent", TIMEOUT),
            "no consent view; {}",
            self.app.stderr()
        );
        until(
            &self.app,
            "consent",
            "document.body.dataset.ready === 'true'",
        );
        assert_eq!(
            self.app
                .eval("consent", "document.querySelector('h1').textContent"),
            title
        );
        assert_eq!(
            self.app.eval(
                "consent",
                "[...document.querySelector('main').children].map(e => e.textContent)"
            ),
            json!([question])
        );
        assert_eq!(
            self.app
                .eval("consent", "document.querySelector('#stop').hidden"),
            true
        );
    }
    fn consents(&self) -> Value {
        let text = fs::read_to_string(self.home.path().join("installed.json")).unwrap();
        let all: Value = serde_json::from_str(&text).unwrap();
        let m = all["mods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["local"] == "claude")
            .unwrap();
        m.get("consents").cloned().unwrap_or_else(|| json!([]))
    }
}
fn new_folder() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap().display().to_string();
    (dir, path)
}
fn pending(folder: &str) -> String {
    json!({"type": "consent-pending", "what": format!("workspace:local/claude:{folder}")})
        .to_string()
}
fn question(folder: &str) -> String {
    format!("Let claude (local) have agents work in {folder} (coder, through claude (local))?")
}

#[test]
#[ignore = "slow: the real app, host and fake claude: the consent view, a returned call, deny and approve"]
fn s58_consent_for_agents_in_a_new_folder_deny_keeps_nothing_approve_starts() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let c = claude_app();
    let (_dir, folder) = new_folder();
    let first = c.start(&folder);
    c.view("Agents in a folder", &question(&folder));
    // The call has returned: its stream holds `consent-pending` while the view waits.
    c.until_items(&first, "consent-pending", |i| i == [pending(&folder)]);
    deny(&c.app);
    c.until_ended(&format!(
        "{first} consent-refused(workspace:local/claude:{folder})"
    ));
    assert_eq!(c.consents(), json!([]));
    // The next ask shows the view again; approving lets the stream start.
    let second = c.start(&folder);
    c.view("Agents in a folder", &question(&folder));
    c.until_items(&second, "consent-pending", |i| i == [pending(&folder)]);
    approve(&c.app);
    c.until_items(&second, "initialized", |i| {
        i.iter().any(|l| l.contains("\"initialized\""))
    });
    assert_eq!(
        c.consents(),
        json!([{"kind": "workspace", "via": "local/claude", "folder": folder}])
    );
    assert!(c.app.quit().success());
}

#[test]
#[ignore = "slow: the real app, host and fake claude: the details, the switch and the next ask"]
fn s60_consent_in_the_details_and_its_withdrawal() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let c = claude_app();
    let (_dir, folder) = new_folder();
    let s = c.start(&folder);
    c.view("Agents in a folder", &question(&folder));
    approve(&c.app);
    c.until_items(&s, "initialized", |i| {
        i.iter().any(|l| l.contains("\"initialized\""))
    });
    let row = |key: &str| {
        format!(
            "section[data-local=claude] .grant-row[data-key={}]",
            json!(key)
        )
    };
    let text = |selector: &str| {
        format!(
            "(document.querySelector({}) || {{textContent: null}}).textContent",
            json!(selector)
        )
    };
    let line = format!("Let claude have agents work in {folder} (through claude (local))?");
    let consent_row = "section[data-local=claude] .consent-row .grant-text";
    until(
        &c.app,
        "main",
        &format!("{} === {}", text(consent_row), json!(line)),
    );
    assert_eq!(
        c.app.eval(
            "main",
            "document.querySelector('section[data-local=claude] .agents .label').textContent"
        ),
        "Agents"
    );
    for (key, words) in [
        ("claude-code", GRANT_CLAUDE),
        ("claude-code-shell", GRANT_SHELL),
    ] {
        assert_eq!(
            c.app
                .eval("main", &text(&format!("{} .grant-text", row(key)))),
            words
        );
    }
    // A key that does not parse is `bad_input` and takes nothing back.
    c.app.eval("main", "window.__result = null; window.__TAURI_INTERNALS__.invoke('consent_withdraw', {local: 'claude', key: 'nonsense'}).then(v => window.__result = {ok: v}, error => window.__result = {error}); null");
    until(&c.app, "main", "window.__result !== null");
    assert_eq!(
        c.app.eval("main", "window.__result.error.code"),
        "bad_input"
    );
    assert_eq!(c.consents().as_array().unwrap().len(), 1);
    // The switch withdraws: the line goes, the file agrees, the stream ends `gone`.
    c.app.eval(
        "main",
        "document.querySelector('section[data-local=claude] .consent-row .sw').click(); null",
    );
    until(
        &c.app,
        "main",
        "!document.querySelector('section[data-local=claude] .consent-row')",
    );
    assert_eq!(c.consents(), json!([]));
    c.until_ended(&format!("{s} gone"));
    // The next start asks again.
    c.start(&folder);
    c.view("Agents in a folder", &question(&folder));
    deny(&c.app);
    assert!(c.app.quit().success());
}

#[test]
#[ignore = "slow: real manager window with installed provider fixtures and an app restart"]
fn ac1_ac2_ac3_ac4_s63_s64_s66_judge_key_in_provider_details() {
    let _gui = GUI.lock().unwrap_or_else(|error| error.into_inner());
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    begin_install(&app, &[fixture("judge-provider"), host_fixture("claude")]);
    approve(&app);
    installed(&app, "claude");
    installed(&app, "judge-provider");
    let query = "s => document.querySelector('section[data-local=judge-provider] .judge ' + s)";
    click(&app, "[data-tile=judge-provider]");
    let evaluate = |script: &str| app.eval("main", &format!("(q => {script})({query})"));
    assert_eq!(evaluate("q('.label').textContent"), "Judge key");
    assert_eq!(evaluate("q('.judge-origin').value"), "");
    assert_eq!(evaluate("q('.judge-key').type"), "password");
    assert_eq!(evaluate("q('.judge-key').autocomplete"), "off");
    assert_eq!(
        app.eval(
            "main",
            "!!document.querySelector('section[data-local=claude] .judge')"
        ),
        false
    );
    let enter = |field: &str, value: &str| {
        app.eval("main", &format!(
            "(q => {{const i = q({}); i.value = {}; i.dispatchEvent(new Event('input')); return null}})({query})",
            json!(field), json!(value)
        ));
    };
    let refuse_save = |local: &str, origin: &str| {
        let args = json!({"local":local,"key":"test-key-only","origin":origin});
        app.eval("main", &format!("window.__badJudge = null; window.__TAURI_INTERNALS__.invoke('judge_set', {args}).then(() => window.__badJudge = 'success', error => window.__badJudge = error); null"));
        until(&app, "main", "window.__badJudge !== null");
        app.eval("main", "window.__badJudge")
    };
    assert_eq!(
        refuse_save("judge-provider", "https://judge.test/v1")["code"],
        "bad_request"
    );
    let refusal = refuse_save("judge-provider", "https://unlisted.test");
    assert_eq!(refusal["code"], "bad_input");
    assert_eq!(
        refusal["message"],
        "This provider cannot reach https://unlisted.test. Choose an origin at one of its granted hosts (judge.test)."
    );
    assert_eq!(
        refuse_save("claude", "https://judge.test")["code"],
        "bad_input"
    );
    let http = json!({"local":"judge-provider","key":"http judge.test"});
    invoke(&app, "grant_withdraw", http.clone());
    assert_eq!(
        refuse_save("judge-provider", "https://judge.test")["code"],
        "bad_input"
    );
    assert!(
        !home
            .path()
            .join("app/accounts/judge/local__judge-provider.json")
            .exists()
    );
    invoke(&app, "grant_give", http);
    until(&app, "main", &format!("!!({query})('.judge-save')"));
    assert_eq!(evaluate("q('.judge-save').disabled"), true);
    assert_eq!(
        evaluate("q('.judge-save-hint').textContent"),
        "Enter a judge origin."
    );
    enter(".judge-origin", "https://judge.test");
    until(
        &app,
        "main",
        &format!("({query})('.judge-save-hint').textContent === 'Enter a judge key.'"),
    );
    assert_eq!(evaluate("q('.judge-save').disabled"), true);
    enter(".judge-key", "test-key-only");
    enter(".judge-origin", "https://judge.test/v1");
    until(
        &app,
        "main",
        &format!(
            "({query})('.judge-feedback').textContent.includes('without a path, query or fragment')"
        ),
    );
    assert_eq!(evaluate("q('.judge-save').disabled"), true);
    enter(".judge-origin", "https://unlisted.test");
    until(
        &app,
        "main",
        &format!(
            "({query})('.judge-feedback').textContent.includes('cannot reach https://unlisted.test')"
        ),
    );
    assert_eq!(evaluate("q('.judge-save').disabled"), true);
    enter(".judge-origin", "https://Judge.Test:443/");
    until(
        &app,
        "main",
        &format!(
            "!({query})('.judge-save').disabled && ({query})('.judge-feedback').textContent === 'The key will go only to https://judge.test.'"
        ),
    );
    click(&app, "section[data-local=judge-provider] .judge-save");
    until(&app, "main", &format!("!!({query})('.judge-replace')"));
    let directory = home.path().join("app/accounts/judge");
    let path = directory.join("local__judge-provider.json");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        json!({"version":1,"key":"test-key-only","origin":"https://judge.test"})
    );
    let view = invoke(&app, "mods_list", json!({}));
    assert!(!view.to_string().contains("test-key-only"));
    let log = app.log();
    assert!(!log.to_string().contains("test-key-only"));
    assert!(log["commands"].as_array().unwrap().contains(&json!({
        "label":"main","cmd":"judge_set","args":{"key":"<redacted>"}
    })));
    assert!(app.quit().success());

    let stray = directory.join("local__absent.json");
    fs::copy(&path, &stray).unwrap();
    let app = App::start(home.path(), &[]);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", &format!("!!({query})('.judge-replace')"));
    assert!(path.exists());
    assert!(!stray.exists());
    click(&app, "[data-tile=judge-provider]");
    click(
        &app,
        "section[data-local=judge-provider] [data-key=claude-code] .sw",
    );
    until(
        &app,
        "main",
        "document.querySelector('section[data-local=judge-provider] [data-key=claude-code] .sw').getAttribute('aria-checked') === 'false'",
    );
    assert_eq!(
        app.eval("main", &format!("({query})('.judge-kept')?.textContent")),
        "A key is kept for https://judge.test"
    );
    assert_eq!(
        app.eval("main", &format!("!!({query})('.judge-editor')")),
        false
    );
    assert_eq!(
        app.eval("main", &format!("!!({query})('.judge-replace')")),
        false
    );
    assert_eq!(
        invoke(&app, "mods_list", json!({}))
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["local"] == "judge-provider")
            .unwrap()["judge"],
        json!({"kept":true,"origin":"https://judge.test","hosts":[]})
    );
    click(&app, "section[data-local=judge-provider] .judge-remove");
    until(
        &app,
        "main",
        "!document.querySelector('section[data-local=judge-provider] .judge')",
    );
    assert!(!path.exists());
    invoke(
        &app,
        "grant_give",
        json!({"local":"judge-provider","key":"claude-code"}),
    );
    invoke(
        &app,
        "judge_set",
        json!({"local":"judge-provider","key":"fresh","origin":"https://judge.test"}),
    );
    assert!(path.exists());
    invoke(
        &app,
        "mod_remove",
        json!({"local":"judge-provider","deleteData":false}),
    );
    assert!(!path.exists());
    assert!(app.quit().success());
}
