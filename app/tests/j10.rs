#![cfg(feature = "driver")]
mod support;
use serde_json::json;
use support::driver::App;
use support::journey::*;
use support::manage::*;

#[test]
#[ignore = "slow: journey J10"]
fn ac3_journey_j10() {
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    begin_install(&app, &[f.weather.clone(), f.notes.clone(), f.desk.clone()]);
    assert!(consent_text(&app).contains(MANAGE));
    assert_eq!(app.eval("consent", "Array.from(document.querySelectorAll('main label')).some(e => e.textContent.includes('Manage your mods:') && e.textContent.includes('new'))"), true);
    approve(&app);
    for local in ["reading-list", "recommendations", "manager"] {
        installed(&app, local);
    }
    assert_eq!(
        std::fs::read_to_string(home.path().join("app/main")).unwrap(),
        "manager\n"
    );
    f.choose(&app);
    assert!(app.quit().success());

    let app = App::start(home.path(), &[]);
    assert!(app.wait(DESK, TIMEOUT));
    assert!(!app.windows().iter().any(|w| w["label"] == "main"));
    until(
        &app,
        DESK,
        "document.querySelector('#mods-result')?.textContent.includes('recommendations') === true",
    );
    click(&app, "mods", json!({}));
    outcome(&app, "recommendations");
    let mods: serde_json::Value = serde_json::from_str(
        app.eval(DESK, "document.querySelector('#result').textContent")
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for local in ["manager", "reading-list", "recommendations"] {
        let item = mods
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["local"] == local)
            .unwrap();
        assert!(!item["grants"].as_array().unwrap().is_empty(), "{item}");
    }
    let notes = mods
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["local"] == "recommendations")
        .unwrap();
    assert!(
        notes["links"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["provider"] == "reading-list" && l["kind"] == "call")
    );
    app.menu("mods");
    assert!(app.wait("main", TIMEOUT));
    until(
        &app,
        "main",
        "document.body.dataset.ready === 'true' && location.hash === '#mods'",
    );
    open(&app, "reading-list");
    click(&app, "disable", json!({"local":"reading-list"}));
    outcome(&app, "ok");
    no_window(&app, "mod-reading-list");
    app.close("main");

    f.ask_install(&app);
    let text = consent_text(&app);
    for line in [ASKED, "Book club", "Ivy", &f.release] {
        assert!(text.contains(line), "{text}");
    }
    assert_eq!(
        app.eval("consent", "document.querySelector('main > p').textContent"),
        ASKED
    );
    outcome(&app, "pending");
    assert!(!home.path().join("mods/counter").exists());
    app.menu("mods");
    assert!(app.wait("main", TIMEOUT));
    until(
        &app,
        "main",
        "document.body.dataset.ready === 'true' && location.hash === '#mods'",
    );

    // Use the actual current id: the ACL must refuse even a valid answer.
    app.eval("consent", "window.currentRequest = null; window.__TAURI_INTERNALS__.invoke('consent_get').then(v => window.currentRequest = v); null");
    until(&app, "consent", "window.currentRequest !== null");
    let request = app.eval("consent", "window.currentRequest");
    app.eval(DESK, &format!("window.answerAttempt = null; window.__TAURI_INTERNALS__.invoke('consent_answer', {{id:{}, approve:true,withheld:[],choice:null}}).then(() => window.answerAttempt = {{allowed:true}}, e => window.answerAttempt = {{allowed:false,error:String(e)}}); null", request["id"]));
    until(&app, DESK, "window.answerAttempt !== null");
    let refused = app.eval(DESK, "window.answerAttempt");
    assert_eq!(refused["allowed"], false, "{refused}");
    assert!(
        refused["error"].as_str().unwrap().contains("not allowed"),
        "{refused}"
    );
    assert!(
        !app.log()["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["label"] == DESK && c["cmd"] == "consent_answer")
    );
    outcome(&app, "pending");
    assert!(!home.path().join("mods/counter").exists());
    approve(&app);
    outcome(&app, "approved");
    assert!(home.path().join("mods/counter").is_dir());

    click(
        &app,
        "request-remove",
        json!({"local":"recommendations","delete-data":true}),
    );
    assert!(consent_text(&app).contains("Desk asks to remove Notes and delete its data"));
    outcome(&app, "pending");
    // Withdraw while the request is still open: it must cancel without deleting Notes.
    invoke(
        &app,
        "grant_withdraw",
        json!({"local":"manager","key":"manage"}),
    );
    // The withdrawal reloads Desk: click only in the replacement document.
    assert!(app.wait(DESK, TIMEOUT));
    no_window(&app, "consent");
    assert!(home.path().join("mods/recommendations").is_dir());
    assert!(app.windows().iter().any(|w| w["label"] == DESK));
    click(
        &app,
        "request-remove",
        json!({"local":"recommendations","delete-data":true}),
    );
    outcome(&app, "not_granted");
    assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
    app.close("main");
    app.menu("mods");
    assert!(app.wait("main", TIMEOUT));
    until(
        &app,
        "main",
        "document.body.dataset.ready === 'true' && location.hash === '#mods'",
    );
    assert!(app.quit().success());
}
