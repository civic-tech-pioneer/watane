#[test]
fn ac3_order_badges_filters_and_monogram() {
    let script = r#"
      import assert from 'node:assert/strict';
      import {libraryOrder, badge, matchesLibrary} from './ui/manager.js';
      import {monogram} from './ui/words.js';
      const mods = [
        {local:'z',name:'Zulu',enabled:true},
        {local:'a',name:'Alpha',enabled:true},
        {local:'u',name:'Update',enabled:true,offer:'x@2'},
        {local:'p',name:'Problem',enabled:true,offer:'x@2',notice:'Withdrawn by the publisher: unsafe'},
        {local:'m',name:'Main',enabled:false,main:true,error:'broken',offer:'x@2'}];
      assert.deepEqual(libraryOrder(mods).map(m=>m.local), ['m','p','u','a','z']);
      assert.equal(badge(mods[4]), 'Off');
      assert.equal(badge(mods[3]), 'Problem');
      assert.equal(badge(mods[2]), 'Update');
      for (const notice of ['Turned off by Desk (local)', 'Grant withdrawn by Desk (local)', 'Turned off by Failed updates (local)']) assert.equal(badge({...mods[3],offer:null,notice}), '');
      for (const notice of ["The publisher's channel is compromised", 'Could not read Alice: offline', 'Update to 2.0.0 failed: offline']) assert.equal(badge({...mods[0],notice}), 'Problem');
      assert.ok(matchesLibrary({name:'Reading list',publisher:{name:'Alice'}},'reading alice','published'));
      assert.ok(!matchesLibrary({name:'Reading list',publisher:null},'reading','published'));
      globalThis.document = {createElement:()=>({setAttribute(k,v){this[k]=v;}})};
      const mono = monogram({name:'Reading list',slug:'reading-list'},44);
      assert.equal(mono.textContent,'R');
      assert.equal(mono['aria-hidden'],'true');
      const sum = 'reading-list'.split('').reduce((n,c)=>n+c.charCodeAt(0),0)%8;
      assert.equal(mono.className,`mono mono-${sum} mono-44`);
      const utf16 = monogram({name:'star',slug:'😀'},24);
      assert.ok(utf16.className.includes(`mono-${(0xD83D+0xDE00)%8}`));
    "#;
    let result = std::process::Command::new("node")
        .args(["--input-type=module", "-e", script])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn ac2_ac3_ac4_ac5_ac6_ac7_page_rendering_and_commands() {
    let result = std::process::Command::new("node")
        .args(["tests/support/manager_dom.mjs", "look"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[cfg(feature = "driver")]
mod support;
#[cfg(feature = "driver")]
mod windows {
    use super::support::{
        driver::App,
        journey::{approve, area, begin_install, fixture, installed, invoke, until},
    };
    use serde_json::json;
    fn install(app: &App, names: &[&str]) {
        begin_install(app, &names.iter().map(|n| fixture(n)).collect::<Vec<_>>());
        approve(app);
        for name in names {
            installed(app, name);
        }
    }
    fn check(app: &App, js: &str) {
        until(app, "main", js);
    }
    fn click(app: &App, selector: &str) {
        app.eval(
            "main",
            &format!("document.querySelector({}).click(); null", json!(selector)),
        );
    }
    #[test]
    fn ac2_ac5_ac7_header_first_start_settings_and_drop() {
        let home = tempfile::tempdir().unwrap();
        let app = App::start(home.path(), &[]);
        check(&app, "document.body.dataset.ready === 'true'");
        check(
            &app,
            "!document.querySelector('#first-start').hidden && document.querySelector('#first-start').textContent.includes('Install your first mod') && document.querySelectorAll('#first-start .card').length === 3",
        );
        check(
            &app,
            "document.querySelector('#filter-control').hidden && document.querySelector('#add-mod').hidden && document.querySelector('[data-tab=updates]').hidden",
        );
        check(
            &app,
            "document.querySelector('.mark').textContent.trim().split(/\\s+/).length === 2 && document.querySelector('.mark').title.includes('Your consent mark.') && !document.querySelector('header a[href=\"#settings\"]')",
        );
        click(&app, "#first-index");
        check(
            &app,
            "location.hash === '#find' && document.activeElement.id === 'index-link'",
        );
        click(&app, "#add-mod");
        check(
            &app,
            "!document.querySelector('#add-menu').hidden && document.activeElement.id === 'install-link'",
        );
        app.eval("main","document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true})); null");
        check(&app, "document.querySelector('#add-menu').hidden");
        area(&app, "#mods");
        app.eval("main", "document.body.classList.add('dropping')");
        check(
            &app,
            "getComputedStyle(document.querySelector('#drop-target')).display === 'flex' && document.querySelector('#drop-target').textContent.includes('Drop to install') && document.querySelector('#drop-target').textContent.includes('watane reads the mod in this folder and asks before it gets anything.')",
        );
        app.eval("main", "document.body.classList.remove('dropping')");
        check(
            &app,
            "getComputedStyle(document.querySelector('#drop-target')).display === 'none'",
        );
        area(&app, "#settings");
        check(
            &app,
            "!document.querySelector('#settings-header').hidden && document.querySelector('#settings-header').textContent.includes('Settings') && document.querySelector('#settings-header a').textContent === 'Back to mods' && !!document.querySelector('#settings #machines-on')",
        );
        area(&app, "#mods");
        app.pick(&[fixture("reading-list")]);
        click(&app, "#first-folder");
        assert!(app.wait("consent", super::support::journey::TIMEOUT));
        until(&app, "consent", "document.body.dataset.ready === 'true'");
        approve(&app);
        installed(&app, "reading-list");
        check(
            &app,
            "document.querySelector('#first-start').hidden && !document.querySelector('#filter-control').hidden && !document.querySelector('#add-mod').hidden",
        );
        assert!(app.quit().success());
    }
    #[test]
    fn ac3_ac4_ac6_ac8_library_details_confirm_install_and_badges() {
        let home = tempfile::tempdir().unwrap();
        let app = App::start(home.path(), &[]);
        install(&app, &["reading-list", "recommendations", "desk"]);
        check(
            &app,
            "document.querySelectorAll('button.tile').length === 3 && document.querySelector('#library-count').textContent === '3 mods' && document.querySelector('button.tile').dataset.tile === 'desk'",
        );
        check(
            &app,
            "!document.querySelector('#installed-note').hidden && document.querySelectorAll('section.mod-details').length === 3 && document.querySelectorAll('section.mod-details:not([hidden])').length === 1",
        );
        click(&app, "[data-tile=desk]");
        check(
            &app,
            "document.querySelector('[data-local=desk] .start-row').textContent.includes('watane opens it when it starts, in place of the manager.')",
        );
        click(&app, "[data-local=desk] .disable");
        check(
            &app,
            "!document.querySelector('#confirm-panel').hidden && document.querySelector('#dependency-warning').textContent.includes('Until you enable it, the manager opens at start.')",
        );
        click(&app, "#confirm-cancel");
        click(&app, "[data-tile=reading-list]");
        check(
            &app,
            "document.querySelector('#installed-note').hidden && !document.querySelector('[data-local=reading-list]').hidden && document.querySelector('[data-tile=reading-list]').getAttribute('aria-pressed') === 'true'",
        );
        check(
            &app,
            "(() => { const hex = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim(); const rgb = 'rgb(' + [1,3,5].map(i => parseInt(hex.slice(i,i+2),16)).join(', ') + ')'; return getComputedStyle(document.querySelector('[data-tile=reading-list]')).borderTopColor === rgb; })()",
        );
        click(
            &app,
            "[data-local=reading-list] [data-rel=used-by] [data-target=recommendations]",
        );
        check(
            &app,
            "document.activeElement.matches('[data-local=recommendations] h2') && !document.querySelector('[data-local=recommendations]').hidden",
        );
        check(
            &app,
            "document.querySelectorAll('[data-local=recommendations] .actions .primary').length === 1 && document.querySelector('[data-local=recommendations] .actions').lastElementChild.matches('.remove.quiet') && !document.querySelector('[data-local=recommendations] details.technical').open",
        );
        check(
            &app,
            "Array.from(document.querySelectorAll('[data-local=recommendations] .technical dt'),n=>n.textContent).join('|') === 'Made from|Page origin|Package|Publisher key'",
        );
        click(&app, "[data-local=recommendations] .main-set");
        check(
            &app,
            "document.querySelector('[data-local=recommendations] .main-set').getAttribute('aria-checked') === 'true'",
        );
        assert_eq!(
            std::fs::read_to_string(home.path().join("app/main")).unwrap(),
            "recommendations\n"
        );
        click(&app, "[data-local=recommendations] .main-set");
        check(
            &app,
            "document.querySelector('[data-local=recommendations] .start-row').textContent.includes('in place of the manager.')",
        );
        assert!(!home.path().join("app/main").exists());
        let mods = invoke(&app, "mods_list", json!({}));
        let grant = mods
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["local"] == "recommendations")
            .unwrap()["grants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["kind"] == "call")
            .unwrap()
            .clone();
        let selector = format!(
            "[data-local=recommendations] [data-key='{}'] .sw",
            grant["key"].as_str().unwrap()
        );
        assert_eq!(
            app.eval(
                "main",
                &format!(
                    "document.querySelector({}).parentElement.querySelector('.grant-text').title",
                    json!(selector)
                )
            ),
            grant["text"]
        );
        let short = grant["short"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| grant["text"].as_str().unwrap());
        assert_eq!(
            app.eval(
                "main",
                &format!(
                    "document.querySelector({}).getAttribute('aria-label')",
                    json!(selector)
                )
            ),
            short
        );
        click(&app, &selector);
        check(
            &app,
            "!!document.querySelector('[data-local=recommendations] .give[role=switch][aria-checked=false]')",
        );
        click(&app, &selector);
        check(
            &app,
            "!document.querySelector('[data-local=recommendations] .give')",
        );
        click(&app, "[data-tile=reading-list]");
        click(&app, "[data-local=reading-list] .disable");
        check(
            &app,
            "!document.querySelector('#confirm-panel').hidden && document.querySelector('#confirm-title').textContent === 'Disable Reading list?' && document.querySelector('#dependency-warning').textContent.includes('These mods use it: Recommendations.') && document.activeElement.id === 'confirm-cancel'",
        );
        app.eval("main","document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true})); null");
        check(&app, "document.querySelector('#confirm-panel').hidden");
        click(&app, "[data-local=reading-list] .remove");
        check(
            &app,
            "document.querySelector('#keep-data').checked && document.activeElement.id === 'confirm-cancel'",
        );
        click(&app, "#delete-data");
        check(
            &app,
            "document.querySelector('#confirm-continue').textContent === 'Remove and delete data'",
        );
        click(&app, "#confirm-cancel");
        click(&app, "[data-tile=recommendations]");
        click(&app, "[data-local=recommendations] .disable");
        check(
            &app,
            "document.querySelector('#confirm-panel').hidden && document.querySelector('[data-tile=recommendations] .badge').textContent === 'Off'",
        );
        app.eval("main","document.querySelector('#library-filter').value='no match'; document.querySelector('#library-filter').dispatchEvent(new Event('input')); null");
        check(
            &app,
            "!document.querySelector('#no-matches').hidden && document.querySelectorAll('.mod-details:not([hidden])').length === 0",
        );
        app.eval("main","document.querySelector('#library-filter').value=''; document.querySelector('#library-filter').dispatchEvent(new Event('input')); null");
        // Dist-only badge states use the production renderer with literal ModView data.
        let mut badges = invoke(&app, "mods_list", json!({}));
        for mod_view in badges.as_array_mut().unwrap() {
            match mod_view["local"].as_str().unwrap() {
                "desk" => {
                    mod_view["main"] = json!(true);
                    mod_view["error"] = json!("A page failed");
                }
                "reading-list" => {
                    mod_view["notice"] = json!("Withdrawn by the publisher: unsafe");
                    mod_view["offer"] = json!("key/reading-list@2.0.0");
                }
                "recommendations" => {
                    mod_view["offer"] = json!("key/recommendations@2.0.0");
                }
                _ => unreachable!(),
            }
        }
        app.eval("main", &format!("window.__rendered=false; import('./app.js').then(page=>{{page.modsView({badges}); window.__rendered=true;}}); null"));
        check(
            &app,
            "window.__rendered && Array.from(document.querySelectorAll('.tile'),n=>n.dataset.tile).join('|') === 'desk|reading-list|recommendations'",
        );
        check(
            &app,
            "Array.from(document.querySelectorAll('.tile')).every(tile => tile.querySelectorAll('.badge').length === 1) && document.querySelector('[data-tile=desk] .badge').textContent === 'Problem' && document.querySelector('[data-tile=desk] .main-mark').textContent === 'Opens at start' && document.querySelector('[data-tile=reading-list] .badge').textContent === 'Problem' && document.querySelector('[data-tile=recommendations] .badge').textContent === 'Off'",
        );
        for mod_view in badges.as_array_mut().unwrap() {
            if mod_view["local"] == "recommendations" {
                mod_view["enabled"] = json!(true);
            }
        }
        app.eval("main", &format!("window.__rendered=false; import('./app.js').then(page=>{{page.modsView({badges}); window.__rendered=true;}}); null"));
        check(
            &app,
            "window.__rendered && document.querySelector('[data-tile=recommendations] .badge').textContent === 'Update'",
        );
        app.theme(None);
        assert!(app.quit().success());
    }
    #[test]
    fn ac8_s50_theme_body_and_tile_colours() {
        let home = tempfile::tempdir().unwrap();
        let app = App::start(home.path(), &[]);
        install(&app, &["reading-list"]);
        for (theme, body, tile) in [
            ("light", "rgb(246, 246, 244)", "rgb(255, 255, 255)"),
            ("dark", "rgb(17, 17, 19)", "rgb(25, 25, 28)"),
        ] {
            app.theme(Some(theme));
            check(
                &app,
                &format!(
                    "getComputedStyle(document.body).backgroundColor === '{}' && getComputedStyle(document.querySelector('.tile')).backgroundColor === '{}'",
                    body, tile
                ),
            );
        }
        app.theme(None);
        assert!(app.quit().success());
    }
    #[test]
    fn ac6_install_from_find_and_external_install_preserves_selection() {
        use super::support::publish::{Site, dist};
        let publisher_home = tempfile::tempdir().unwrap();
        let publisher = dist(publisher_home.path());
        let site = Site::serve(publisher.site_dir());
        publisher.set_site_url(&site.base).unwrap();
        let package = watane_host::watane_pkg::pack(&fixture("reading-list"), false).unwrap();
        publisher.publish(package.into_bytes()).unwrap();
        let home = tempfile::tempdir().unwrap();
        let app = App::start(home.path(), &[]);
        check(&app, "document.body.dataset.ready === 'true'");
        invoke(
            &app,
            "channel_follow",
            json!({"link":publisher.own_link(None).unwrap().to_string()}),
        );
        area(&app, "#find");
        app.eval("main", "document.querySelector('#library-filter').value='hidden'; document.querySelector('#search-text').value='reading'; document.querySelector('#search').click(); null");
        check(
            &app,
            "!!document.querySelector('#results .install-release')",
        );
        click(&app, "#results .install-release");
        assert!(app.wait("consent", super::support::journey::TIMEOUT));
        until(&app, "consent", "document.body.dataset.ready === 'true'");
        approve(&app);
        installed(&app, "reading-list");
        check(
            &app,
            "location.hash === '#mods' && document.querySelector('#library-filter').value === '' && document.querySelector('[data-tile=reading-list]').getAttribute('aria-pressed') === 'true' && !document.querySelector('#installed-note').hidden && document.querySelector('#installed-note').textContent.includes('Reading list is installed.')",
        );
        click(&app, "#installed-note .dismiss");
        check(&app, "document.querySelector('#installed-note').hidden");
        let port = app.port;
        let request = std::thread::spawn(move || {
            reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap()
                .post(format!("http://localhost:{port}/api/install"))
                .json(&json!({"folders":[fixture("desk")]}))
                .send()
                .unwrap()
                .status()
        });
        assert!(app.wait("consent", super::support::journey::TIMEOUT));
        until(&app, "consent", "document.body.dataset.ready === 'true'");
        approve(&app);
        assert!(request.join().unwrap().is_success());
        app.eval("main", "window.dispatchEvent(new Event('focus')); null");
        installed(&app, "desk");
        check(
            &app,
            "document.querySelector('[data-tile=reading-list]').getAttribute('aria-pressed') === 'true' && document.querySelector('#installed-note').hidden",
        );
        click(&app, "[data-tile=desk]");
        check(&app, "!document.querySelector('[data-local=desk]').hidden");
        assert!(app.quit().success());
    }
}
