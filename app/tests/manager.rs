#![cfg(feature = "driver")]
mod support;
use serde_json::json;
use support::{
    driver::App,
    journey::{TIMEOUT, approve, begin_install, fixture, installed, invoke, until},
};

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
            vec!["Uses: reading-list", "Adds to: reading-list"],
        ),
        (
            "reading-list",
            vec!["Used by: recommendations", "Extended by: recommendations"],
        ),
        (
            "desk",
            vec!["Works on its own: it uses no mod and no mod uses it."],
        ),
    ] {
        for text in texts {
            until(
                &app,
                "main",
                &format!(
                    "document.querySelector('[data-local={local}] details').textContent.includes({})",
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
        "!document.querySelector('[data-local=recommendations] .connections').textContent.includes('Uses:') && !document.querySelector('[data-local=reading-list] .connections').textContent.includes('Used by:')",
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
            "document.querySelector('#dependency-dialog').open",
        );
        assert_eq!(app.eval("main", "document.querySelector('#dependency-dialog').textContent.includes('recommendations')"), true);
        assert_eq!(
            app.eval("main", "document.activeElement.id"),
            "dependency-cancel"
        );
        app.eval(
            "main",
            "document.querySelector('#dependency-cancel').click(); null",
        );
        let mods = invoke(&app, "mods_list", json!({}));
        assert!(
            mods.as_array()
                .unwrap()
                .iter()
                .all(|m| m["enabled"] == true)
        );
        assert_eq!(
            app.eval("main", "document.querySelector('#remove-dialog').open"),
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
        assert.deepEqual(dependents(source,mods), ['recommendations']);
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
