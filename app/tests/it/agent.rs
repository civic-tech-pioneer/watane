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
use crate::support::journey::{TIMEOUT, invoke, until};
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
