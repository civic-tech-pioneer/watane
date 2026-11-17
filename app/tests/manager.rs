#![cfg(feature = "driver")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

mod support;
use serde_json::json;
use support::driver::App;
use support::journey::{TIMEOUT, approve, begin_install, fixture, installed, invoke, until};

fn install(app: &App, names: &[&str]) {
    begin_install(app, &names.iter().map(|n| fixture(n)).collect::<Vec<_>>());
    approve(app);
    for name in names {
        installed(app, name);
    }
}
#[test]
fn ac1_s37_start_in_main_mod() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    install(&app, &["reading-list"]);
    install(&app, &["desk"]);
    assert!(app.quit().success());
    let app = App::start(home.path(), &[]);
    assert!(app.wait("mod-desk", TIMEOUT));
    assert!(!app.windows().iter().any(|w| w["label"] == "main"));
    // Disable through the manager, then restart the same home.
    app.menu("mods");
    assert!(app.wait("main", TIMEOUT));
    invoke(&app, "mod_disable", json!({"local":"desk"}));
    assert!(app.quit().success());
    let app = App::start(home.path(), &[]);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    assert_eq!(app.eval("main", "location.hash"), "#mods");
    assert!(app.quit().success());
}

#[test]
fn ac2_s38_choose_unset_remove_and_stale_name() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    install(&app, &["reading-list"]);
    let path = home.path().join("app/main");
    invoke(&app, "main_set", json!({"local":"reading-list"}));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "reading-list\n");
    invoke(&app, "main_set", json!({"local":null}));
    assert!(!path.exists());
    invoke(&app, "main_set", json!({"local":"reading-list"}));
    invoke(
        &app,
        "mod_remove",
        json!({"local":"reading-list", "deleteData":false}),
    );
    assert!(!path.exists());
    assert!(app.quit().success());
    std::fs::write(&path, "not-installed\n").unwrap();
    let app = App::start(home.path(), &[]);
    assert!(app.wait("main", TIMEOUT));
    assert!(!path.exists());
    assert!(app.quit().success());
}

#[test]
fn ac3_s39_menu_and_last_window_exit() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    install(&app, &["desk"]);
    assert!(app.quit().success());
    let app = App::start(home.path(), &[]);
    assert!(app.wait("mod-desk", TIMEOUT));
    assert!(!app.windows().iter().any(|w| w["label"] == "main"));
    app.menu("settings");
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "location.hash === '#settings'");
    app.close("main");
    assert!(app.windows().iter().any(|w| w["label"] == "mod-desk"));
    app.menu("mods");
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "location.hash === '#mods'");
    app.close("mod-desk");
    assert!(app.close_last("main").success());
}
#[test]
fn ac4_connections_use_only_granted_installed_links() {
    let script = r#"
        import {connections} from './ui/manager.js';
        import assert from 'node:assert/strict';
        const provider = {local:'reading-list',enabled:true,links:[]};
        const consumer = {local:'recommendations',enabled:true,links:[
            {kind:'call',provider:'reading-list',granted:true},
            {kind:'subscribe',provider:'reading-list',granted:true},
            {kind:'call',provider:'absent',granted:true}]};
        assert.deepEqual(connections(consumer,[provider,consumer]), {uses:['reading-list'],addsTo:['reading-list'],usedBy:[],extendedBy:[]});
        assert.deepEqual(connections(provider,[provider,consumer]), {uses:[],addsTo:[],usedBy:['recommendations'],extendedBy:['recommendations']});
        consumer.links[0].granted = false;
        assert.deepEqual(connections(consumer,[provider,consumer]).uses, []);
        assert.deepEqual(connections(provider,[provider,consumer]).usedBy, []);
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
#[test]
fn ac4_s40_connections_and_withdrawal() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    install(&app, &["reading-list", "recommendations", "desk"]);
    for (local, texts) in [
        (
            "recommendations",
            vec![("uses", "Reading list"), ("adds-to", "Reading list")],
        ),
        (
            "reading-list",
            vec![
                ("used-by", "Recommendations"),
                ("extended-by", "Recommendations"),
            ],
        ),
        (
            "desk",
            vec![("", "Works on its own: it uses no mod and no mod uses it.")],
        ),
    ] {
        for (rel, text) in texts {
            until(
                &app,
                "main",
                &format!(
                    "document.querySelector('[data-local={local}] {}').textContent.includes({})",
                    if rel.is_empty() {
                        String::from(".connections")
                    } else {
                        format!(
                            "[data-rel={rel}] [data-target={}]",
                            if rel == "uses" || rel == "adds-to" {
                                "reading-list"
                            } else {
                                "recommendations"
                            }
                        )
                    },
                    json!(text)
                ),
            );
        }
    }
    let mods = invoke(&app, "mods_list", json!({}));
    let rec = mods
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["local"] == "recommendations")
        .unwrap();
    let key = rec["grants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["kind"] == "call")
        .unwrap()["key"]
        .clone();
    invoke(
        &app,
        "grant_withdraw",
        json!({"local":"recommendations","key":key}),
    );
    app.eval("main", "window.dispatchEvent(new Event('focus')); null");
    until(
        &app,
        "main",
        "!document.querySelector('[data-local=recommendations] .connections').querySelector('[data-rel=uses]') && !document.querySelector('[data-local=reading-list] .connections').querySelector('[data-rel=used-by]')",
    );
    assert!(app.quit().success());
}

#[test]
fn ac5_s41_warn_before_disable_and_remove() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    install(&app, &["reading-list", "recommendations"]);
    for action in ["disable", "remove"] {
        app.eval(
            "main",
            &format!(
                "document.querySelector('[data-local=reading-list] button.{action}').click(); null"
            ),
        );
        until(
            &app,
            "main",
            "!document.querySelector('#confirm-panel').hidden",
        );
        assert_eq!(
            app.eval(
                "main",
                "document.querySelector('#confirm-panel').textContent.includes('Recommendations')"
            ),
            true
        );
        assert_eq!(
            app.eval("main", "document.activeElement.id"),
            "confirm-cancel"
        );
        app.eval(
            "main",
            "document.querySelector('#confirm-cancel').click(); null",
        );
        let mods = invoke(&app, "mods_list", json!({}));
        assert!(
            mods.as_array()
                .unwrap()
                .iter()
                .all(|m| m["enabled"] == true)
        );
        assert_eq!(
            app.eval("main", "!document.querySelector('#confirm-panel').hidden"),
            false
        );
    }
    assert!(app.quit().success());
}
#[test]
fn ac5_warning_names_only_enabled_granted_dependents() {
    let script = r#"
        import {dependents} from './ui/manager.js';
        import assert from 'node:assert/strict';
        const source = {local:'reading-list', links:[]};
        const mods = [source, {local:'recommendations', enabled:true, links:[{kind:'call',provider:source.local,granted:true},{kind:'subscribe',provider:source.local,granted:true}]},
            {local:'disabled',enabled:false,links:[{kind:'call',provider:source.local,granted:true}]},
            {local:'withdrawn',enabled:true,links:[{kind:'call',provider:source.local,granted:false}]}];
        assert.deepEqual(dependents(source,mods), {usedBy:['recommendations'], extendedBy:['recommendations']});
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
#[test]
fn ac6_s42_settings_apart() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    until(&app, "main", "document.body.dataset.ready === 'true'");
    for area in ["library", "find", "updates"] {
        assert_eq!(app.eval("main", &format!("document.querySelector('#{area}').querySelectorAll('#identity, #site, #backup, #agents, #machines').length")), 0);
    }
    for id in ["identity", "site", "backup", "agents", "machines"] {
        assert_eq!(
            app.eval(
                "main",
                &format!("!!document.querySelector('#settings #{id}')")
            ),
            true
        );
    }
    app.menu("settings");
    until(
        &app,
        "main",
        "!document.querySelector('#settings').hidden && document.querySelector('#mods').hidden",
    );
    app.menu("mods");
    until(
        &app,
        "main",
        "document.querySelector('#settings').hidden && !document.querySelector('#mods').hidden",
    );
    assert!(app.quit().success());
}

#[test]
fn ac7_mod_view_commands_and_hostile_page_refusal() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    install(&app, &["desk", "hostile"]);
    let mods = invoke(&app, "mods_list", json!({}));
    let desk = mods
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["local"] == "desk")
        .unwrap();
    assert_eq!(desk["main"], true);
    assert_eq!(desk["proposesMain"], true);
    assert_eq!(desk["links"], json!([]));
    assert_eq!(invoke(&app, "updates_done", json!({})), json!([]));
    assert_eq!(
        invoke(&app, "install_paths", json!({"paths":[]})),
        json!({"installed":[]})
    );
    invoke(&app, "mod_open", json!({"local":"hostile"}));
    assert!(app.wait("mod-hostile", TIMEOUT));
    app.eval("mod-hostile", r#"window.__stageI = null;
        Promise.all([['install_paths',{paths:[]}],['main_set',{local:'hostile'}],['updates_done',{}]].map(async ([cmd,args]) => {
            try { await window.__TAURI_INTERNALS__.invoke(cmd,args); return {cmd,allowed:true}; }
            catch (error) { return {cmd,allowed:false,error:String(error)}; }
        })).then(result => window.__stageI = result); null"#);
    until(&app, "mod-hostile", "window.__stageI !== null");
    let results = app.eval("mod-hostile", "window.__stageI");
    for result in results.as_array().unwrap() {
        assert_eq!(result["allowed"], false, "{result}");
        assert!(
            result["error"].as_str().unwrap().contains("not allowed"),
            "{result}"
        );
    }
    let commands = app.log()["commands"].clone();
    assert!(
        !commands
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["label"] == "mod-hostile")
    );
    assert!(app.quit().success());
}
#[test]
fn ac8_each_install_entry_adopts_a_proposed_main() {
    use support::publish::{Site, begin, finish};
    let publisher_home = tempfile::tempdir().unwrap();
    let publisher = support::publish::dist(publisher_home.path());
    let site = Site::serve(publisher.site_dir());
    publisher.set_site_url(&site.base).unwrap();
    let package = watane_host::watane_pkg::pack(&fixture("desk"), false).unwrap();
    let release = publisher.publish(package.into_bytes()).unwrap();
    for command in [
        "install_folders",
        "install_paths",
        "install_link",
        "release_install",
    ] {
        let home = tempfile::tempdir().unwrap();
        let app = App::start(home.path(), &[]);
        until(&app, "main", "document.body.dataset.ready === 'true'");
        if command == "install_folders" {
            app.pick(&[fixture("desk")]);
            begin(&app, command, json!({}));
        } else if command == "install_paths" {
            begin(&app, command, json!({"paths":[fixture("desk")]}));
        } else if command == "install_link" {
            begin(
                &app,
                command,
                json!({"link":release.link.as_ref().unwrap().to_string()}),
            );
        } else {
            invoke(
                &app,
                "channel_follow",
                json!({"link":publisher.own_link(None).unwrap().to_string()}),
            );
            begin(
                &app,
                command,
                json!({"release":release.info.reference.to_string()}),
            );
        }
        assert!(app.wait("consent", TIMEOUT));
        until(&app, "consent", "document.body.dataset.ready === 'true'");
        approve(&app);
        let result = finish(&app);
        assert!(result.get("error").is_none(), "{command}: {result}");
        assert_eq!(
            std::fs::read_to_string(home.path().join("app/main")).unwrap(),
            "desk\n",
            "{command}"
        );
        assert!(app.quit().success());
    }
}

#[test]
fn ac6_library_filters_and_updates_need_attention() {
    let script = r#"
        import {matchesLibrary, updateOffers} from './ui/manager.js';
        import assert from 'node:assert/strict';
        const published = {name:'Reading list', publisher:{name:'Alice',fingerprint:'abc'}, offerGrants:[]};
        assert.ok(matchesLibrary(published, 'alice reading', 'published'));
        assert.ok(!matchesLibrary(published, '', 'local'));
        assert.ok(matchesLibrary({...published,publisher:null}, 'reading', 'local'));
        const failed = {...published,notice:'Could not read Alice: offline'};
        const compromised = {...published,notice:"The publisher's channel is compromised"};
        const withdrawn = {...published,notice:'Withdrawn by the publisher: broken'};
        const grants = {...published,offer:'release@1.0.0',offerGrants:[{}]};
        assert.deepEqual(updateOffers([failed,compromised,withdrawn,grants]), [compromised,withdrawn,grants]);
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

#[test]
fn ac1_s81_library_marks_only_main_and_unset_clears_it() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    install(&app, &["reading-list", "desk"]);
    invoke(&app, "main_set", json!({"local":"reading-list"}));
    app.eval("main", "window.dispatchEvent(new Event('focus')); null");
    until(
        &app,
        "main",
        "document.querySelector('[data-tile=reading-list] .main-mark')?.textContent === 'Opens at start' && document.querySelectorAll('.main-mark').length === 1",
    );
    app.eval("main", "document.querySelector('[data-tile=reading-list]').click(); document.querySelector('[data-local=reading-list] .main-set').click(); null");
    until(
        &app,
        "main",
        "document.querySelectorAll('.main-mark').length === 0",
    );
    assert!(app.quit().success());
}

fn dom_test(script: &str, case: &str) {
    let output = std::process::Command::new("node")
        .args([script, case])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn ac1_s81_library_main_marker_dom() {
    dom_test("tests/support/manager_dom.mjs", "main");
}

#[test]
fn ac2_s81_install_main_explanation_dom() {
    dom_test("tests/support/consent_dom.mjs", "main");
}
#[test]
fn ac2_s81_install_consent_explains_only_new_main() {
    for existing in [false, true] {
        let home = tempfile::tempdir().unwrap();
        let app = App::start(home.path(), &[]);
        install(&app, &["reading-list"]);
        if existing {
            invoke(&app, "main_set", json!({"local":"reading-list"}));
        }
        begin_install(&app, &[fixture("desk")]);
        assert_eq!(app.eval("consent", "Array.from(document.querySelectorAll('main p')).some(p => p.textContent === 'Desk opens when watane starts, in place of the manager.')"), !existing);
        approve(&app);
        installed(&app, "desk");
        assert_eq!(
            std::fs::read_to_string(home.path().join("app/main")).unwrap(),
            if existing { "reading-list\n" } else { "desk\n" }
        );
        assert!(app.quit().success());
    }
}

#[test]
fn ac3_s81_connection_moves_focus_dom() {
    dom_test("tests/support/manager_dom.mjs", "focus");
}
#[test]
fn ac3_s81_used_by_link_opens_details_and_moves_focus() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    install(&app, &["reading-list", "recommendations"]);
    // Panels appear before the install action finishes selecting its new mod.
    until(
        &app,
        "main",
        "!document.querySelector('#installed-note').hidden",
    );
    app.eval("main", "document.querySelector('[data-tile=reading-list]').click(); document.querySelector('[data-local=reading-list] .connections .chip').click(); null");
    until(
        &app,
        "main",
        "!document.querySelector('[data-local=recommendations]').hidden && document.activeElement.closest('[data-local]')?.dataset.local === 'recommendations'",
    );
    assert!(app.quit().success());
}

#[test]
fn ac4_s81_disable_remove_warning_distinguishes_connections_dom() {
    dom_test("tests/support/manager_dom.mjs", "warning");
}
#[test]
fn ac4_s81_disable_remove_warning_distinguishes_connections() {
    for (withhold_kind, sentences) in [
        (Some("call"), "These mods extend it: Recommendations."),
        (Some("subscribe"), "These mods use it: Recommendations."),
        (
            None,
            "These mods use it: Recommendations. These mods extend it: Recommendations.",
        ),
    ] {
        let home = tempfile::tempdir().unwrap();
        let app = App::start(home.path(), &[]);
        install(&app, &["reading-list", "recommendations"]);
        if let Some(kind) = withhold_kind {
            let mods = invoke(&app, "mods_list", json!({}));
            let consumer = mods
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["local"] == "recommendations")
                .unwrap();
            let key = consumer["grants"]
                .as_array()
                .unwrap()
                .iter()
                .find(|g| g["kind"] == kind)
                .unwrap()["key"]
                .clone();
            invoke(
                &app,
                "grant_withdraw",
                json!({"local":"recommendations", "key":key}),
            );
            app.eval("main", "window.dispatchEvent(new Event('focus')); null");
            until(
                &app,
                "main",
                &format!(
                    "!document.querySelector('[data-local=recommendations] .connections').textContent.includes({})",
                    json!(if kind == "call" { "Uses" } else { "Adds to" })
                ),
            );
        }
        for verb in ["Disable", "Remove"] {
            app.eval(
                "main",
                &format!(
                    "document.querySelector('[data-local=reading-list] .{}').click(); null",
                    verb.to_lowercase()
                ),
            );
            until(
                &app,
                "main",
                "!document.querySelector('#confirm-panel').hidden",
            );
            assert_eq!(
                app.eval(
                    "main",
                    "document.querySelector('#confirm-title').textContent + ' ' + Array.from(document.querySelector('#dependency-warning .warning-lines').children,n=>n.textContent).join(' ')"
                ),
                format!("{verb} Reading list? {sentences}")
            );
            app.eval(
                "main",
                "document.querySelector('#confirm-cancel').click(); null",
            );
        }
        assert!(app.quit().success());
    }
}

#[test]
fn ac5_s81_words_and_focus_are_documented_at_their_facts() {
    let technical = include_str!("../TECHNICAL.md");
    let functional = include_str!("../FUNCTIONAL.md");
    let library = technical
        .split("### 5.3 Main window")
        .nth(1)
        .unwrap()
        .split("### 5.4")
        .next()
        .unwrap();
    for text in [
        "Opens at start",
        "focus",
        "These mods use it",
        "These mods extend it",
    ] {
        assert!(library.contains(text), "5.3 lacks {text}");
    }
    let consent = technical
        .split("### 6.3 The consent page")
        .nth(1)
        .unwrap()
        .split("## 7.")
        .next()
        .unwrap();
    assert!(consent.contains("<name> opens when watane starts, in place of the manager."));
    for (id, texts) in [
        (
            "63",
            vec![
                "Opens at start",
                "opens when watane starts, in place of the manager.",
            ],
        ),
        ("66", vec!["focus"]),
        ("67", vec!["These mods use it", "These mods extend it"]),
    ] {
        let line = functional
            .lines()
            .find(|l| l.starts_with(&format!("- **APP-F-{id}**")))
            .unwrap();
        for text in texts {
            assert!(line.contains(text), "APP-F-{id} lacks {text}");
        }
    }
}

#[test]
#[ignore = "slow: proves watane-dev install adoption through a running app and restart"]
fn ac2_s51_dev_install_adopts_main_and_update_keeps_it_unset() {
    let dev = support::dev::workspace_dev();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    // A copy: watane-dev writes wit/deps into the folder it installs.
    let desk = tempfile::tempdir().unwrap();
    support::journey::copy_dir(&fixture("desk"), desk.path());
    let mut command = support::dev::dev_command(&dev, &app, desk.path(), &["install"]);
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    assert!(app.wait("consent", TIMEOUT));
    until(&app, "consent", "document.body.dataset.ready === 'true'");
    assert_eq!(app.eval("consent", "document.body.textContent.includes('Desk opens when watane starts, in place of the manager.')"), true);
    approve(&app);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let path = home.path().join("app/main");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "desk\n");
    assert!(app.quit().success());
    let app = App::start(home.path(), &[]);
    assert!(app.wait("mod-desk", TIMEOUT));
    assert_eq!(
        app.windows()
            .iter()
            .map(|w| w["label"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["mod-desk"]
    );
    app.menu("mods");
    assert!(app.wait("main", TIMEOUT));
    invoke(&app, "main_set", json!({"local":null}));
    let changed = tempfile::tempdir().unwrap();
    support::journey::copy_dir(&fixture("desk"), changed.path());
    std::fs::write(
        changed.path().join("ui/index.html"),
        "<!doctype html><h1>Updated Desk</h1>",
    )
    .unwrap();
    support::dev::dev_run(&dev, &app, changed.path(), &["install"]);
    assert!(!path.exists());
    assert!(app.quit().success());
}

#[test]
fn ac5_s39_w37_menu_items_and_separators() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    assert_eq!(
        app.menu_items(),
        json!([
            {"menu":"watane", "items":["About watane", "-", "Mods…", "Settings…", "-", "Quit watane"]},
            {"menu":"Edit", "items":["Undo", "Redo", "-", "Cut", "Copy", "Paste", "Select All"]},
            {"menu":"Window", "items":["Minimize", "Close Window"]}
        ])
    );
    assert!(app.quit().success());
}

#[test]
fn ac7_s39_w37_quiet_theme_changes_prefers_color_scheme() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    for (theme, dark) in [("dark", true), ("light", false)] {
        app.theme(Some(theme));
        until(
            &app,
            "main",
            &format!("matchMedia('(prefers-color-scheme: dark)').matches === {dark}"),
        );
    }
    app.theme(None);
    assert!(app.quit().success());
}
