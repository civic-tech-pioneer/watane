#![cfg(feature = "driver")]
#![allow(
    clippy::disallowed_methods,
    reason = "tests may catch panics and share lock-only mutexes that guard no data"
)]

use crate::support;
use crate::support::GUI;
use serde_json::json;
use support::driver::App;
use support::journey::*;
use support::manage::*;
use support::publish::*;

#[test]
fn ac1_desk_ui_has_every_manager_api_button() {
    let html = std::fs::read_to_string(fixture("desk-ui/ui/index.html")).unwrap();
    for function in [
        "mods",
        "disable",
        "withdraw",
        "close-page",
        "open-page",
        "request-install",
        "request-update",
        "request-enable",
        "request-grant",
        "request-remove",
        "request",
        "disable-then-fail",
        "disable-then-read",
        "who",
    ] {
        assert!(html.contains(&format!("id=\"{function}\"")), "{function}");
    }
    for field in ["local", "release", "link", "grant", "request-id", "result"] {
        assert!(html.contains(&format!("id=\"{field}\"")), "{field}");
    }
    assert!(!html.contains("<script>"));
}

#[test]
fn ac3_s82_open_page_is_unique_and_dropped_during_consent() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.install(&app);
    no_window(&app, "mod-reading-list");
    click(&app, "open-page", json!({"local":"reading-list"}));
    outcome(&app, "ok");
    let deadline = std::time::Instant::now() + TIMEOUT;
    while !app
        .windows()
        .iter()
        .any(|w| w["label"] == "mod-reading-list")
    {
        assert!(std::time::Instant::now() < deadline, "Weather did not open");
        std::thread::yield_now();
    }
    click(&app, "open-page", json!({"local":"reading-list"}));
    outcome(&app, "ok");
    // The menu round trip drains earlier posted window operations.
    app.menu("mods");
    assert_eq!(
        app.windows()
            .iter()
            .filter(|w| w["label"] == "mod-reading-list")
            .count(),
        1
    );
    assert!(
        invoke(&app, "mods_list", json!({}))
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["local"] == "reading-list")
            .unwrap()["notice"]
            .is_null()
    );

    click(&app, "disable", json!({"local":"reading-list"}));
    outcome(&app, "ok");
    no_window(&app, "mod-reading-list");
    no_window(&app, "mod-recommendations");
    let labels = || {
        let mut labels: Vec<_> = app
            .windows()
            .iter()
            .map(|w| w["label"].as_str().unwrap().to_owned())
            .collect();
        labels.sort();
        labels
    };
    let before = labels();
    click(&app, "request-enable", json!({"local":"reading-list"}));
    consent(&app);
    outcome(&app, "pending");
    let mut with_consent = before.clone();
    with_consent.push("consent".into());
    with_consent.sort();
    click(&app, "open-page", json!({"local":"recommendations"}));
    outcome(&app, "ok");
    app.menu("mods");
    assert_eq!(labels(), with_consent);
    approve(&app);
    app.menu("mods");
    assert_eq!(labels(), before);
    // Apply a later open successfully: the dropped one must not reappear.
    click(&app, "open-page", json!({"local":"reading-list"}));
    outcome(&app, "ok");
    assert!(app.wait("mod-reading-list", TIMEOUT));
    app.menu("mods");
    no_window(&app, "mod-recommendations");

    begin_install(&app, &[host_fixture("echo")]);
    approve(&app);
    installed(&app, "echo");
    click(&app, "open-page", json!({"local":"echo"}));
    outcome(&app, "bad_request");
    no_window(&app, "mod-echo");
    assert!(app.quit().success());
}

#[test]
fn ac2_s43_grant_words() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    begin_install(&app, std::slice::from_ref(&f.desk));
    let text = consent_text(&app);
    assert!(text.contains(MANAGE), "{text}");
    assert_eq!(
        app.eval(
            "consent",
            "Array.from(document.querySelectorAll('main label')).some(e => e.textContent.includes('Manage your mods:') && e.textContent.includes('new'))"
        ),
        true,
        "{text}"
    );
    approve(&app);
    installed(&app, "manager");
    until(
        &app,
        "main",
        "!!document.querySelector('[data-local=manager] [data-key=manage] button.withdraw')",
    );
    let details = app.eval(
        "main",
        "document.querySelector('[data-local=manager] [data-key=manage] .grant-text').title",
    );
    assert!(details.as_str().unwrap().contains(MANAGE), "{details}");
    assert!(app.quit().success());
}

#[test]
fn ac2_s44_disable_closes_page_and_restart_clears_notice() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.install(&app);
    open(&app, "reading-list");
    click(&app, "disable", json!({"local":"reading-list"}));
    outcome(&app, "ok");
    no_window(&app, "mod-reading-list");
    refresh(&app);
    until(
        &app,
        "main",
        "document.querySelector('[data-local=reading-list]').textContent.includes('Turned off by Desk (local)')",
    );
    assert!(app.quit().success());
    let app = App::start(home.path(), &[]);
    app.menu("mods");
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    let mods = invoke(&app, "mods_list", json!({}));
    let weather = mods
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["local"] == "reading-list")
        .unwrap();
    assert_eq!(weather["enabled"], false);
    assert!(weather["notice"].is_null(), "{weather}");
    assert_eq!(app.eval("main", "document.querySelector('[data-local=reading-list]').textContent.includes('Turned off by')"), false);
    assert!(app.quit().success());
}

#[test]
fn ac5_s45_install_denied_approved_and_unread_link_never_connects() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.install(&app);
    f.choose(&app);
    let before = mods_on_disk(&app);
    f.ask_install(&app);
    let text = consent_text(&app);
    assert!(text.contains(ASKED), "{text}");
    assert!(text.contains("Book club"), "{text}");
    assert_eq!(
        app.eval("consent", "document.querySelector('main > p').textContent"),
        ASKED
    );
    assert!(!app.home.join("mods/counter").exists());
    outcome(&app, "pending");
    deny(&app);
    outcome(&app, "denied");
    assert_eq!(mods_on_disk(&app), before);
    f.ask_install(&app);
    assert!(consent_text(&app).contains(ASKED));
    approve(&app);
    outcome(&app, "approved");
    assert!(app.home.join("mods/counter").is_dir());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let unknown = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy";
    let link = format!(
        "watane:{unknown}@http://{}/#counter",
        listener.local_addr().unwrap()
    );
    let before = mods_on_disk(&app);
    click(
        &app,
        "request-install",
        json!({"release":format!("{unknown}/counter@0.1.0"), "link":link}),
    );
    assert!(consent_text(&app).contains(&format!("Desk asks to read {link}")));
    deny(&app);
    outcome(&app, "denied");
    no_window(&app, "consent");
    assert!(
        matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
        "unread channel received a connection"
    );
    assert_eq!(mods_on_disk(&app), before);
    assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
    assert!(app.quit().success());
}

#[test]
fn ac2_s46_update_without_new_grants_still_asks() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.install(&app);
    f.choose(&app);
    f.ask_install(&app);
    consent(&app);
    approve(&app);
    outcome(&app, "approved");
    let release = f.ivy.publish(book_package("0.2.0")).unwrap();
    // Re-read the chosen index without the app's background update command:
    // channels_refresh would automatically install this grant-free version.
    invoke(
        &app,
        "index_choose",
        json!({"link":f.ivy.own_link(None).unwrap().to_string()}),
    );
    let mods = invoke(&app, "mods_list", json!({}));
    let book = mods
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["local"] == "counter")
        .unwrap();
    assert_eq!(book["offer"], release.info.reference.to_string());
    assert_eq!(book["offerGrants"], json!([]));
    click(&app, "request-update", json!({"local":"counter"}));
    let text = consent_text(&app);
    assert!(text.contains(ASKED), "{text}");
    assert_eq!(
        app.eval("consent", "document.querySelector('h1').textContent"),
        "Update Book club to 0.2.0"
    );
    outcome(&app, "pending");
    approve(&app);
    outcome(&app, "approved");
    let mods = invoke(&app, "mods_list", json!({}));
    assert!(
        mods.as_array()
            .unwrap()
            .iter()
            .any(|m| m["local"] == "counter" && m["version"] == "0.2.0")
    );
    assert!(app.quit().success());
}

#[test]
fn ac2_s47_removal_deletes_state_and_names_users() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.install(&app);
    open(&app, "reading-list");
    open(&app, "recommendations");
    app.eval("mod-reading-list", "document.querySelector('#title').value = 'Before deletion'; document.querySelector('#add').click(); null");
    until(
        &app,
        "mod-recommendations",
        "document.querySelector('#books').textContent.includes('Before deletion')",
    );
    // The page can render the provider's list before Notes saves the event.
    until(
        &app,
        "mod-recommendations",
        r"(() => {
            if (window.receivedBefore?.includes('Before deletion')) return true;
            if (!window.readingReceived) {
                window.readingReceived = true;
                watane.call('api.received')
                    .then(books => window.receivedBefore = books)
                    .catch(() => {}) // Try again on the next poll if the call fails.
                    .finally(() => window.readingReceived = false);
            }
            return false;
        })()",
    );
    assert_eq!(
        app.eval("mod-recommendations", "window.receivedBefore"),
        json!(["Before deletion"])
    );
    click(
        &app,
        "request-remove",
        json!({"local":"recommendations","delete-data":true}),
    );
    let text = consent_text(&app);
    assert!(text.contains(ASKED), "{text}");
    assert!(
        text.contains("Its data is deleted. This cannot be undone."),
        "{text}"
    );
    assert!(!text.contains("Used by:"), "{text}");
    assert_eq!(
        app.eval("consent", "document.querySelector('h1').textContent"),
        "Desk asks to remove Notes and delete its data"
    );
    assert!(app.home.join("mods/recommendations").exists());
    approve(&app);
    outcome(&app, "approved");
    assert!(!app.home.join("mods/recommendations").exists());
    no_window(&app, "mod-recommendations");
    begin_install(&app, std::slice::from_ref(&f.notes));
    approve(&app);
    installed(&app, "recommendations");
    open(&app, "recommendations");
    // Received events are Notes's private state; Weather still holds the book.
    app.eval(
        "mod-recommendations",
        "window.received = null; watane.call('api.received').then(v => window.received = v); null",
    );
    until(&app, "mod-recommendations", "window.received !== null");
    assert_eq!(
        app.eval("mod-recommendations", "window.received"),
        json!([])
    );
    click(
        &app,
        "request-remove",
        json!({"local":"reading-list","delete-data":false}),
    );
    let text = consent_text(&app);
    assert!(text.contains("Used by: Notes"), "{text}");
    assert!(
        text.contains("Its data is kept. Installing it again brings it back."),
        "{text}"
    );
    deny(&app);
    outcome(&app, "denied");
    assert!(app.home.join("mods/reading-list").is_dir());
    assert!(app.quit().success());
}

#[test]
fn ac2_s48_enable_and_grant_wait_for_consent() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.install(&app);
    click(&app, "disable", json!({"local":"reading-list"}));
    outcome(&app, "ok");
    click(&app, "request-enable", json!({"local":"reading-list"}));
    assert!(consent_text(&app).contains(ASKED));
    assert_eq!(
        app.eval("consent", "document.querySelector('h1').textContent"),
        "Desk asks to turn on Weather"
    );
    assert!(
        invoke(&app, "mods_list", json!({}))
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["local"] == "reading-list" && m["enabled"] == false)
    );
    approve(&app);
    outcome(&app, "approved");
    assert!(
        invoke(&app, "mods_list", json!({}))
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["local"] == "reading-list" && m["enabled"] == true && m["notice"].is_null())
    );
    invoke(
        &app,
        "grant_withdraw",
        json!({"local":"recommendations","key":CALL_KEY}),
    );
    click(
        &app,
        "request-grant",
        json!({"local":"recommendations","grant":call_grant().to_string()}),
    );
    let text = consent_text(&app);
    assert!(text.contains(ASKED), "{text}");
    assert!(
        text.contains("Call books of reading-list (local)"),
        "{text}"
    );
    assert_eq!(
        app.eval("consent", "document.querySelector('h1').textContent"),
        "Desk asks to give Notes a grant"
    );
    assert_eq!(
        app.eval(
            "consent",
            "document.querySelector('#deny').getAttribute('aria-label')"
        ),
        "Deny: Desk asks to give Notes a grant"
    );
    assert!(
        text.contains("Call mods:reading-list/books@0.1.0 of reading-list (local)"),
        "{text}"
    );
    assert_eq!(
        app.eval(
            "consent",
            "document.querySelectorAll('input[type=checkbox]').length"
        ),
        0
    );
    approve(&app);
    outcome(&app, "approved");
    let mods = invoke(&app, "mods_list", json!({}));
    let notes = mods
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["local"] == "recommendations")
        .unwrap();
    assert!(
        notes["grants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["key"] == CALL_KEY && g["granted"] == true)
    );
    assert!(app.quit().success());
}

#[test]
fn ac1_fixture_packages_and_page_messages() {
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let host = watane_host::Host::open(home.path(), watane_host::Mode::Preview, Default::default())
        .unwrap();
    let plan = host
        .plan(vec![
            watane_host::Source::Folder(f.desk.clone()),
            watane_host::Source::Folder(f.weather.clone()),
            watane_host::Source::Folder(f.notes.clone()),
        ])
        .unwrap();
    for (slug, name) in [
        ("manager", "Desk"),
        ("reading-list", "Weather"),
        ("recommendations", "Notes"),
    ] {
        let entry = plan
            .entries()
            .iter()
            .find(|e| e.slug.as_str() == slug)
            .unwrap();
        assert_eq!(entry.name, name);
    }
    assert!(watane_host::watane_pkg::Mod::read(&f.desk).unwrap().main());
    let book = f
        .ivy
        .latest(&f.ivy.identity().unwrap(), &"counter".parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(book.release.name, "Book club");
    assert_eq!(book.reference.to_string(), f.release);
    // An index includes its own releases; dist refuses listing oneself explicitly.
    let reader_home = tempfile::tempdir().unwrap();
    let reader = dist(reader_home.path());
    reader.choose_index(&f.ivy.own_link(None).unwrap()).unwrap();
    let hits = reader
        .search(&watane_dist::Query {
            text: "Book club".into(),
            publisher: None,
            origin: None,
            interface: None,
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].info.reference.to_string(), f.release);

    let script = r#"
      import assert from 'node:assert/strict';
      import {readFileSync} from 'node:fs';
      import vm from 'node:vm';
      const html = readFileSync('tests/fixtures/desk-ui/ui/index.html', 'utf8');
      const ids = [...html.matchAll(/id="([^"]+)"/g)].map(m => m[1]);
      const fields = Object.fromEntries(ids.map(id => [id, {id, textContent:'',value:'',checked:false}]));
      const functions = [...html.matchAll(/<button id="([^"]+)"/g)].map(m => m[1]);
      const calls = [];
      let response, error, listener, grantListener;
      const document = {getElementById:id => fields[id], querySelectorAll:() => functions.map(id => fields[id])};
      const watane = {
        call: async (fn,args) => { calls.push([fn,args]); if(error) throw error; return fn === 'api.mods' ? [] : fn === 'api.request' ? 'approved' : response; },
        on: handler => {listener = handler;}
      };
      const window = {addEventListener(name, handler) {
        assert.equal(name, 'watane:grant-changed');
        grantListener = handler;
      }};
      vm.runInNewContext(readFileSync('tests/fixtures/desk-ui/ui/app.js','utf8'), {document,watane,window});
      await new Promise(resolve => setImmediate(resolve));
      fields.local.value = 'reading-list'; fields.release.value = 'release'; fields.link.value = 'link';
      fields.grant.value = JSON.stringify({call:{interface:'books',provider:'local/reading-list'}});
      fields['delete-data'].checked = true; fields['request-id'].value = 'chosen-id';
      for (const fn of functions) {
        calls.length = 0;
        response = fn.startsWith('request-') ? {ok:'r1'} : {ok:null};
        await fields[fn].onclick();
        assert.equal(calls[0][0], 'api.'+fn);
        const args = calls[0][1];
        if (fn === 'request-install') assert.deepEqual(JSON.parse(JSON.stringify(args)), {release:'release',link:'link'});
        else if (fn === 'request') assert.equal(args.id, fields['request-id'].value);
        else if (!['mods','who'].includes(fn)) assert.equal(args.local,'reading-list');
        if (['withdraw','request-grant'].includes(fn)) assert.equal(args.grant.call.provider,'local/reading-list');
        if (fn === 'request-remove') assert.equal(args['delete-data'],true);
        if(fn.startsWith('request-')) assert.equal(fields.result.textContent, '"approved"');
      }
      response = {err:{code:'no_mod',message:'missing'}};
      await fields.disable.onclick();
      assert.equal(JSON.parse(fields.result.textContent).err.code, 'no_mod');
      error = {code:'not_granted',message:'withdrawn'};
      await fields['request-enable'].onclick();
      assert.equal(JSON.parse(fields.result.textContent).code,'not_granted');
      error = null; response = {ok:'r2'};
      await fields['request-enable'].onclick();
      await listener({type:'catalog'});
      assert.equal(fields.result.textContent,'"approved"');
      error = {code:'not_granted',message:'withdrawn'};
      await grantListener();
      assert.equal(JSON.parse(fields['mods-result'].textContent).code,'not_granted');
      assert.equal(fields.result.textContent,'"approved"');
    "#;
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "-e", script])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// Quiet: every reload is observed through the native page-load event, then DOM.
#[test]
fn ac1_manage_calls_settle_across_grant_notification() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.install(&app);
    // Pause the real host response until the grant has changed in this document.
    app.eval(
        DESK,
        r#"window.grantChanges = 0;
        window.addEventListener('watane:grant-changed', () => window.grantChanges++);
        const originalFetch = window.fetch;
        window.fetch = async (...args) => {
          const response = await originalFetch(...args);
          if (JSON.parse(args[1]?.body || '{}').fn === 'api.request-remove') {
            window.responseValue = await response.clone().json();
            await new Promise(resolve => { window.releaseResponse = resolve; });
          }
          return response;
        };
        null"#,
    );
    click(
        &app,
        "request-remove",
        json!({"local":"recommendations","delete-data":true}),
    );
    consent(&app);
    until(&app, DESK, "typeof window.releaseResponse === 'function'");
    let cancelled = app.eval(DESK, "window.responseValue.value.ok");
    assert!(cancelled.is_string(), "{cancelled}");
    invoke(
        &app,
        "grant_withdraw",
        json!({"local":"manager","key":"manage"}),
    );
    until(&app, DESK, "window.grantChanges === 1");
    until(
        &app,
        DESK,
        "document.querySelector('#mods-result').textContent.includes('not_granted')",
    );
    no_window(&app, "consent");
    app.eval(DESK, "window.releaseResponse(); null");
    outcome(&app, cancelled.as_str().unwrap());
    assert!(home.path().join("mods/recommendations").is_dir());
    click(&app, "mods", json!({}));
    outcome(&app, "not_granted");

    invoke(
        &app,
        "grant_give",
        json!({"local":"manager","key":"manage"}),
    );
    until(&app, DESK, "window.grantChanges === 2");
    until(
        &app,
        DESK,
        "document.querySelector('#mods-result').textContent.includes('recommendations')",
    );
    app.eval(DESK, "window.fetch = originalFetch; null");
    click(&app, "request", json!({"request-id":cancelled}));
    outcome(&app, "denied");
    // The cancelled request was ended by host on revoke: no pending request
    // survives to block another request after manage is given back.
    click(
        &app,
        "request-remove",
        json!({"local":"recommendations","delete-data":true}),
    );
    consent(&app);
    outcome(&app, "pending");
    deny(&app);
    outcome(&app, "denied");
    assert!(app.quit().success());
}

#[test]
fn ac3_s82_read_asked_before_fetch_then_install_and_follow() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[("WATANE_DRIVER_LISTED_LOCAL", "1")]);
    f.install(&app);
    let before = mods_on_disk(&app);
    f.ask_install(&app);
    assert!(consent_text(&app).contains(&format!("Desk asks to read {}", f.link)));
    deny(&app);
    outcome(&app, "denied");
    assert_eq!(f.requests(), 0);
    assert_eq!(mods_on_disk(&app), before);
    f.ask_install(&app);
    assert!(consent_text(&app).contains(&format!("Desk asks to read {}", f.link)));
    // This approval immediately opens the install consent under the same
    // window label. The shared approve helper waits for that label to vanish,
    // which can miss the brief interval between the two windows.
    app.eval_may_close("consent", "document.querySelector('#approve').disabled = false; document.querySelector('#approve').click(); null");
    let deadline = std::time::Instant::now() + TIMEOUT;
    while !app.is_true_if_present(
        "consent",
        "document.querySelector('h1')?.textContent === 'Install Book club'",
    ) {
        assert!(
            std::time::Instant::now() < deadline,
            "next consent did not open"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        app.eval("consent", "document.querySelector('h1').textContent"),
        "Install Book club"
    );
    assert!(consent_text(&app).contains(ASKED));
    assert!(!app.home.join("mods/counter").exists());
    approve(&app);
    outcome(&app, "approved");
    assert!(app.home.join("mods/counter").is_dir());
    let channels = invoke(&app, "channels_list", json!({}));
    assert!(
        channels
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["channel"] == f.ivy.identity().unwrap().to_string()),
        "{channels}"
    );
    assert!(f.requests() > 0);
    assert!(app.quit().success());
}

#[test]
fn adv_s113_unchanged_managed_install_refollows_without_install_consent() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[("WATANE_DRIVER_LISTED_LOCAL", "1")]);
    f.install(&app);

    f.ask_install(&app);
    assert!(consent_text(&app).contains(&format!("Desk asks to read {}", f.link)));
    approve(&app);
    assert!(consent_text(&app).contains("Install Book club"));
    approve(&app);
    outcome(&app, "approved");

    let publisher = f.ivy.identity().unwrap().to_string();
    invoke(&app, "channel_unfollow", json!({"channel":publisher}));
    app.eval(
        DESK,
        "document.querySelector('#result').textContent = ''; null",
    );
    f.ask_install(&app);
    assert!(consent_text(&app).contains(&format!("Desk asks to read {}", f.link)));
    approve(&app);
    outcome(&app, "approved");
    no_window(&app, "consent");

    let channels = invoke(&app, "channels_list", json!({}));
    assert!(
        channels
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["channel"] != publisher),
        "repeating an unchanged install followed the publisher: {channels}"
    );
    assert!(app.quit().success());
}

#[test]
fn ac4_s82_stop_asks_withdraws_manage_until_given_back() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.install(&app);
    click(&app, "disable", json!({"local":"reading-list"}));
    outcome(&app, "ok");
    click(&app, "request-enable", json!({"local":"reading-list"}));
    assert!(consent_text(&app).contains("Desk asks to turn on Weather"));
    outcome(&app, "pending");
    until(
        &app,
        DESK,
        "document.querySelector('#request-id').value.length === 32",
    );
    let id = app.eval(DESK, "document.querySelector('#request-id').value");
    app.eval(DESK, "window.grantChanges = 0; window.addEventListener('watane:grant-changed', () => window.grantChanges++); null");
    app.eval("consent", "document.querySelector('#stop').click(); null");
    until(&app, DESK, "window.grantChanges === 1");
    no_window(&app, "consent");
    let mods = invoke(&app, "mods_list", json!({}));
    let mods = mods.as_array().unwrap();
    assert!(mods.iter().any(|m| {
        m["local"] == "manager"
            && m["grants"]
                .as_array()
                .unwrap()
                .iter()
                .any(|g| g["key"] == "manage" && g["granted"] == false)
    }));
    assert!(
        mods.iter()
            .any(|m| m["local"] == "reading-list" && m["enabled"] == false)
    );
    click(&app, "request-enable", json!({"local":"reading-list"}));
    outcome(&app, "not_granted");
    invoke(
        &app,
        "grant_give",
        json!({"local":"manager","key":"manage"}),
    );
    until(&app, DESK, "window.grantChanges === 2");
    click(&app, "request", json!({"request-id":id}));
    outcome(&app, "denied");
    click(&app, "request-enable", json!({"local":"reading-list"}));
    assert!(consent_text(&app).contains("Desk asks to turn on Weather"));
    approve(&app);
    outcome(&app, "approved");
    assert!(app.quit().success());
}
