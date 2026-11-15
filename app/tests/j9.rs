#![cfg(feature = "driver")]
mod support;
use redb::ReadableDatabase;
use serde_json::json;
use std::path::{Path, PathBuf};
use support::{driver::App, journey::*};
use watane_host::watane_pkg::PackageId;

fn host_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../host/tests/fixtures/mods")
        .join(name)
}

#[test]
#[ignore = "slow: journey J9"]
fn ac2_journey_j9() {
    let folders = tempfile::tempdir().unwrap();
    let menu = folders.path().join("menu");
    copy_dir(&host_fixture("menu"), &menu);
    copy_dir(&fixture("menu-ui/ui"), &menu.join("ui"));
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    begin_install(
        &app,
        &[
            menu.clone(),
            host_fixture("item-a"),
            host_fixture("item-b"),
            host_fixture("item-bad"),
        ],
    );
    for local in ["item-a", "item-b", "item-bad"] {
        assert_eq!(
            app.eval("consent", &format!(
                "Array.from(document.querySelectorAll('main section')).find(s => s.querySelector('h2').textContent === {}).textContent.includes('Receive the mods:menu/menu@0.1.0 events of menu (local) and answer its requests')",
                json!(local)
            )),
            true,
            "{local}'s subscription must explain both events and requests"
        );
    }
    approve(&app);
    for local in ["menu", "item-a", "item-b", "item-bad"] {
        installed(&app, local);
    }
    let mods = invoke(&app, "mods_list", json!({}));
    for local in ["item-a", "item-b", "item-bad"] {
        let item = mods
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["local"] == local)
            .unwrap();
        assert!(
            item["grants"]
                .as_array()
                .unwrap()
                .iter()
                .any(|g| g["kind"] == "subscribe" && g["granted"] == true)
        );
    }
    open(&app, "menu");
    until(
        &app,
        "mod-menu",
        "document.querySelector('#items').textContent === '[\"a\",\"b\"]'",
    );
    assert_eq!(
        app.eval(
            "mod-menu",
            "Array.from(document.querySelectorAll('#commands button'), b => b.textContent)"
        ),
        json!(["a", "b"])
    );
    app.eval(
        "mod-menu",
        "document.querySelector('#commands button').click(); null",
    );
    until(
        &app,
        "mod-menu",
        "document.querySelector('#clicked').textContent === 'a'",
    );

    let item_b = mods
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["local"] == "item-b")
        .unwrap();
    let grant = item_b["grants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["kind"] == "subscribe")
        .unwrap();
    invoke(
        &app,
        "grant_withdraw",
        json!({"local":"item-b", "key":grant["key"]}),
    );
    app.eval(
        "mod-menu",
        "document.querySelector('#refresh').click(); null",
    );
    until(
        &app,
        "mod-menu",
        "document.querySelector('#items').textContent === '[\"a\"]'",
    );
    assert_eq!(
        app.eval(
            "mod-menu",
            "Array.from(document.querySelectorAll('#commands button'), b => b.textContent)"
        ),
        json!(["a"])
    );
    assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
    assert!(app.quit().success());

    // The app has closed the host/database. Inspect only item-a's committed state.
    let db = redb::Database::open(home.path().join("state/state.redb")).unwrap();
    // PackageId::of uses BLAKE3, as host's state table names do.
    let id = PackageId::of(b"local/item-a").to_string();
    let name = format!("s/{}", &id[..32]);
    let txn = db.begin_read().unwrap();
    let table = txn
        .open_table(redb::TableDefinition::<&str, &[u8]>::new(&name))
        .unwrap();
    assert_eq!(table.get("clicked").unwrap().unwrap().value(), b"a");

    let fresh_home = tempfile::tempdir().unwrap();
    let app = App::start(fresh_home.path(), &[]);
    until(&app, "main", "document.body.dataset.ready === 'true'");
    app.pick(&[menu, host_fixture("item-back")]);
    app.eval("main", "document.querySelector('#install').click(); null");
    until(
        &app,
        "main",
        "document.querySelector('#error').textContent.includes('cycle')",
    );
    let error = app.eval("main", "document.querySelector('#error').textContent");
    let error = error.as_str().unwrap();
    assert!(
        error.contains("menu") && error.contains("item-back"),
        "{error}"
    );
    assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
    assert_eq!(invoke(&app, "mods_list", json!({})), json!([]));
    assert!(app.quit().success());
}
