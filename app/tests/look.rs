#[test]
fn ac1_ac2_ac3_ac5_find_and_updates_rendering() {
    let result = std::process::Command::new("node")
        .args(["tests/support/manager_dom.mjs", "find-updates"])
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
fn ac1_search_reads_one_publisher_and_interface() {
    let script = r#"
      import assert from 'node:assert/strict';
      import {searchWords} from './ui/words.js';
      const parties = [{channel:'ivy-key',name:'Ivy',fingerprint:'IVYFP'}, {channel:'alice-key',name:'Alice Smith',fingerprint:'alicefp'}];
      assert.deepEqual(searchWords('book ivy', parties), {text:'book',publisher:'ivy-key',interface:null});
      assert.deepEqual(searchWords('IVYFP book mods:books/list@1.2.3 ivy mods:other/api',parties), {text:'book ivy mods:other/api',publisher:'ivy-key',interface:'mods:books/list@1.2.3'});
      assert.deepEqual(searchWords('Alice Smith book',parties), {text:'book',publisher:'alice-key',interface:null});
      assert.deepEqual(searchWords('book nonsense: /bad@1',parties), {text:'book nonsense: /bad@1',publisher:null,interface:null});
      for (const word of ['mods:books/list@01.2.3','mods:books/list@1.2.3-beta','mods:books/list@18446744073709551616.0.0','mods:book-1/list', 'a'.repeat(120)+':books/list']) assert.deepEqual(searchWords(word,[]),{text:word,publisher:null,interface:null});
      assert.deepEqual(searchWords('mods:books/list',[]), {text:'',publisher:null,interface:'mods:books/list'});
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
#[test]
fn ac3_channel_site_holds_a_real_read_until_released() {
    use support::publish::Site;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("head"), "verified head fixture").unwrap();
    let site = Site::serve(dir.path().to_path_buf());
    let held = site.hold();
    let url = format!("{}head", site.base);
    let (finished, result) = std::sync::mpsc::channel();
    let request = std::thread::spawn(move || {
        let response = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(support::journey::TIMEOUT)
            .build()
            .unwrap()
            .get(url)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .unwrap();
        finished.send(response).unwrap();
    });
    site.wait_held(held);
    assert!(matches!(
        result.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    site.release();
    assert_eq!(
        result.recv_timeout(support::journey::TIMEOUT).unwrap(),
        "verified head fixture"
    );
    request.join().unwrap();
}

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
    fn snapshot(app: &App, script: &str) {
        app.eval("main", &format!("window.__snapshot=null; import('./app.js').then(page=>{{window.__snapshot=(()=>{{{script}}})()}}); null"));
        check(app, "window.__snapshot !== null");
        assert_eq!(app.eval("main", "window.__snapshot"), json!(true));
    }
    struct FindFixture {
        publisher: watane_dist::Dist,
        other: watane_dist::Dist,
        _site: super::support::publish::Site,
        _other_site: super::support::publish::Site,
        _publisher_home: tempfile::TempDir,
        _other_home: tempfile::TempDir,
    }
    impl FindFixture {
        fn new() -> Self {
            use super::support::publish::{Site, dist};
            let publisher_home = tempfile::tempdir().unwrap();
            let publisher = dist(publisher_home.path());
            publisher.set_name("Ivy").unwrap();
            let site = Site::serve(publisher.site_dir());
            publisher.set_site_url(&site.base).unwrap();
            let package = watane_host::watane_pkg::pack(&fixture("reading-list"), false).unwrap();
            publisher.publish(package.into_bytes()).unwrap();
            // Ivy has a book mod and a non-book mod. Another publisher has a
            // book mod without Ivy's interface: each filter must exclude a decoy.
            fn page_package(slug: &str, name: &str, summary: &str) -> Vec<u8> {
                let dir = tempfile::tempdir().unwrap();
                std::fs::create_dir(dir.path().join("wit")).unwrap();
                std::fs::create_dir(dir.path().join("ui")).unwrap();
                std::fs::write(
                    dir.path().join("mod.toml"),
                    format!("name = {name:?}\nsummary = {summary:?}\n"),
                )
                .unwrap();
                std::fs::write(
                    dir.path().join("wit/world.wit"),
                    format!("package mods:{slug}@0.1.0; world {slug} {{}} "),
                )
                .unwrap();
                std::fs::write(
                    dir.path().join("ui/index.html"),
                    "<!doctype html><p>Fixture</p>",
                )
                .unwrap();
                watane_host::watane_pkg::pack(dir.path(), false)
                    .unwrap()
                    .into_bytes()
            }
            publisher
                .publish(page_package("notes", "Notes", "Plain notes"))
                .unwrap();
            let other_home = tempfile::tempdir().unwrap();
            let other = dist(other_home.path());
            other.set_name("Alice").unwrap();
            let other_site = Site::serve(other.site_dir());
            other.set_site_url(&other_site.base).unwrap();
            let other_release = other
                .publish(page_package("bookmarks", "Bookmarks", "Save books"))
                .unwrap();
            publisher
                .list(other_release.link.as_ref().unwrap(), "More books")
                .unwrap();
            Self {
                publisher,
                other,
                _site: site,
                _other_site: other_site,
                _publisher_home: publisher_home,
                _other_home: other_home,
            }
        }
    }
    #[test]
    fn ac1_real_dist_fixture_filters_search_fields() {
        use super::support::publish::dist;
        let fixture = FindFixture::new();
        let home = tempfile::tempdir().unwrap();
        let reader = dist(home.path());
        reader
            .choose_index(&fixture.other.own_link(None).unwrap())
            .unwrap();
        reader
            .choose_index(&fixture.publisher.own_link(None).unwrap())
            .unwrap();
        let mut query = watane_dist::Query::default();
        assert_eq!(reader.search(&query).unwrap().len(), 3);
        query.text = "book".into();
        assert_eq!(reader.search(&query).unwrap().len(), 2);
        query.publisher = fixture.publisher.identity();
        let expected = format!(
            "{}/reading-list@0.1.0",
            fixture.publisher.identity().unwrap()
        );
        let hits = reader.search(&query).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].info.reference.to_string(), expected);
        query = watane_dist::Query {
            interface: Some("mods:reading-list/books".parse().unwrap()),
            ..Default::default()
        };
        let hits = reader.search(&query).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].info.reference.to_string(), expected);
    }
    #[test]
    fn ac1_ac3_ac5_find_words_results_and_light_dark() {
        use super::support::publish::choose;
        let fixture = FindFixture::new();
        let publisher = &fixture.publisher;
        let other = &fixture.other;
        let home = tempfile::tempdir().unwrap();
        let app = App::start(home.path(), &[]);
        // Explicitly choosing the other publisher allows its loopback site
        // under DD-26, just as in tests/find.rs.
        choose(&app, &other.own_link(None).unwrap().to_string());
        choose(&app, &publisher.own_link(None).unwrap().to_string());
        area(&app, "#find");
        app.eval("main", "document.querySelector('#search-text').value=''; document.querySelector('#search').click(); null");
        check(
            &app,
            "document.querySelectorAll('#results article.result').length===3",
        );
        app.eval("main", "document.querySelector('#search-text').value='book'; document.querySelector('#search').click(); null");
        check(
            &app,
            "document.querySelectorAll('#results article.result').length===2",
        );
        app.eval("main", "document.querySelector('#search-text').value='book ivy'; document.querySelector('#search').click(); null");
        let ivy_release = format!("{}/reading-list@0.1.0", publisher.identity().unwrap());
        check(
            &app,
            &format!(
                "document.querySelectorAll('#results article.result').length===1 && document.querySelector('#results article.result').dataset.release==={} && document.querySelector('#search-reading').textContent==='By Ivy'",
                json!(ivy_release)
            ),
        );
        // Start from all three again, so the interface check cannot pass on
        // the result left behind by the publisher search.
        app.eval("main", "document.querySelector('#search-text').value=''; document.querySelector('#search').click(); null");
        check(
            &app,
            "document.querySelectorAll('#results article.result').length===3",
        );
        app.eval("main", "document.querySelector('#search-text').value='mods:reading-list/books'; document.querySelector('#search-text').dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true})); null");
        check(
            &app,
            &format!(
                "document.querySelectorAll('#results article.result').length===1 && document.querySelector('#results article.result').dataset.release==={} && document.querySelector('#search-reading').textContent==='Offering mods:reading-list/books'",
                json!(ivy_release)
            ),
        );
        click(&app, "#add-mod");
        check(
            &app,
            "document.activeElement.id==='install-link' && !document.querySelector('#add-menu').hidden",
        );
        let indexes = invoke(&app, "indexes_list", json!({}));
        let hits = invoke(
            &app,
            "search",
            json!({"text":"","publisher":null,"interface":null}),
        );
        let hit = hits
            .as_array()
            .unwrap()
            .iter()
            .find(|hit| hit["release"] == ivy_release)
            .unwrap();
        let mut compromised = hit.clone();
        compromised["status"] = json!("compromised");
        compromised["verified"] = json!(false);
        let mut installed_hit = hit.clone();
        installed_hit["installed"] = json!("reading-list");
        let mut older_hit = installed_hit.clone();
        older_hit["version"] = json!("0.2.0");
        let mods = json!([{"local":"reading-list","slug":"reading-list","name":"Reading list","version":"0.1.0","enabled":true,"grants":[],"links":[],"page":null}]);
        for theme in ["light", "dark"] {
            app.theme(Some(theme));
            snapshot(
                &app,
                &format!(
                    r#"
              const $=s=>document.querySelector(s);
              page.modsView({mods}); page.renderIndexes({indexes}); page.renderResults({});
              const cards=Array.from(document.querySelectorAll('article.result'));
              return document.querySelectorAll('#search-text').length===1 &&
                !$('#search-publisher') && !$('#search-interface') && !$('#refresh-find') &&
                !!$('.ix[data-channel] .index-dot') && !!$('.ix .drop-index[aria-label]') &&
                $('#result-count').textContent==='4 mods found' &&
                cards[0].querySelector('.install-release').textContent==='Install…' &&
                cards[1].querySelector('a.installed-link').textContent==='Installed' &&
                cards[2].textContent.includes('You have 0.1.0') &&
                cards[3].querySelector('.blocked').textContent==="Can't install: Ivy's channel no longer matches what it said before, so watane can't trust it. Ask Ivy for the link to a new channel." &&
                !cards[3].querySelector('button') && !!$('#follow-link') && !!$('#follow-channel') && !!$('#channels') &&
                $('#find').textContent.includes('Channels you follow') &&
                $('#find').textContent.includes('Install opens the plan in the consent view. Nothing installs from here.');
            "#,
                    json!([hit, installed_hit, older_hit, compromised])
                ),
            );
        }
        app.theme(None);
        assert!(app.quit().success());
    }
    #[test]
    fn ac2_ac3_ac5_updates_cards_and_held_check() {
        use super::support::publish::{Site, dist};
        let publisher_home = tempfile::tempdir().unwrap();
        let publisher = dist(publisher_home.path());
        publisher.set_name("Ivy").unwrap();
        let site = Site::serve(publisher.site_dir());
        publisher.set_site_url(&site.base).unwrap();
        let home = tempfile::tempdir().unwrap();
        let app = App::start(home.path(), &[]);
        check(&app, "document.body.dataset.ready==='true'");
        invoke(
            &app,
            "channel_follow",
            json!({"link":publisher.own_link(None).unwrap().to_string()}),
        );
        app.eval("main", "window.dispatchEvent(new Event('focus')); null");
        check(
            &app,
            &format!(
                "!!document.querySelector('#channels [data-channel=\"{}\"]')",
                publisher.identity().unwrap()
            ),
        );
        area(&app, "#updates");
        for theme in ["light", "dark"] {
            app.theme(Some(theme));
            snapshot(
                &app,
                r#"
              const $=s=>document.querySelector(s), now=Date.now();
              const mod=(local,extra={})=>({local,slug:local,name:local,version:'1.0.0',enabled:true,links:[],offer:null,offerGrants:[],...extra});
              const mods=[mod('Reading list',{offer:'key/reading-list@2.0.0',offerGrants:[{kind:'network',short:'Connect to books.test',text:'Full exact grant'}]}),mod('Tags',{notice:'Withdrawn by the publisher: loses tags',offer:'key/tags@2.0.0',release:'key/tags@1.0.0'}),mod('Notes',{links:[{kind:'call',provider:'Tags',granted:true}]}),mod('Bad',{notice:"The publisher's channel is compromised"})];
              page.renderUpdates(mods,[{local:'old',name:'Old',from:'1',to:'2',at:now-86400000},{local:'new',name:'New',from:'1',to:'2',at:now}], [{channel:'key',name:'Bob',fingerprint:'bobfp',cause:'whole: nested cause',readAt:now-172800000}],[]);
              const labels=Array.from(document.querySelectorAll('#updates h2'),n=>n.textContent).join('|');
              const first=$('#update-offers .card');
              const valid=labels==='Needs you|Channels|Updated by itself' && $('#updates h1').textContent==='Updates' &&
                first.querySelector('.newgrant').textContent==='◎ Connect to books.test' && first.querySelector('.newgrant').title==='Full exact grant' &&
                first.textContent.includes('Not now keeps 1.0.0 and hides this until the offer changes or watane restarts.') &&
                first.querySelector('.offer').textContent==='Review…' && $('#updates-count').textContent==='3' &&
                $('#update-offers').textContent.includes('Notes uses it.') && $('#update-offers').textContent.includes('2.0.0 is the newest version that is not withdrawn and asks for nothing new.') &&
                !$('#update-offers [data-local=Bad]').querySelector('button') &&
                $('#update-channels').textContent.includes("Couldn't read Bob's channel") &&
                $('#update-channels').textContent.includes('Tags comes from it, so its updates wait. watane tries again at the next check. Last read 2 days ago.') &&
                $('#update-channels').textContent.includes('Cause: whole: nested cause') && $('#update-channels button').textContent==='Try again' &&
                $('#update-done .done-row').dataset.local==='new' && $('#update-done').textContent.includes('today, ');
              first.querySelector('.not-now').click();
              window.__dismissCheck=()=>$('#updates-count').textContent==='2' && !$('#update-offers [data-local="Reading list"]');
              return valid;
            "#,
            );
            check(&app, "window.__dismissCheck()");
            // Empty lists show their placeholders and hide the Channels section.
            snapshot(
                &app,
                r#"page.renderUpdates([],[],[],[]); return document.querySelector('#update-offers').textContent==='Nothing needs you.' && document.querySelector('#failed-channels').hidden && document.querySelector('#update-done').textContent==='None since the app started.';"#,
            );
        }
        let held = site.hold();
        click(&app, "#refresh-channels");
        site.wait_held(held);
        check(
            &app,
            "document.querySelector('#refresh-channels').disabled && document.querySelector('#refresh-channels .refresh-label').textContent==='Checking…' && document.querySelector('#refresh-channels .refresh-glyph').classList.contains('spin') && document.querySelector('#checked-at').textContent==='Reading 1 channel…' && document.querySelector('#checked-at').getAttribute('role')==='status'",
        );
        site.release();
        check(
            &app,
            "!document.querySelector('#refresh-channels').disabled && document.querySelector('#refresh-channels .refresh-label').textContent==='Check now' && document.querySelector('#checked-at').textContent==='Checked just now' && !document.querySelector('#refresh-channels .refresh-glyph').classList.contains('spin')",
        );
        app.theme(None);
        assert!(app.quit().success());
    }
    #[test]
    fn ac2_ac5_review_opens_real_consent() {
        use super::support::publish::{Site, consent, dist, update_package};
        let publisher_home = tempfile::tempdir().unwrap();
        let publisher = dist(publisher_home.path());
        let site = Site::serve(publisher.site_dir());
        publisher.set_site_url(&site.base).unwrap();
        let published = publisher.publish(update_package("0.1.0", true)).unwrap();
        let home = tempfile::tempdir().unwrap();
        let app = App::start(home.path(), &[]);
        check(&app, "document.body.dataset.ready==='true'");
        invoke(
            &app,
            "channel_follow",
            json!({"link":publisher.own_link(None).unwrap().to_string()}),
        );
        area(&app, "#updates");
        snapshot(
            &app,
            &format!(
                r#"
          page.renderUpdates([{{local:'reading-list',slug:'reading-list',name:'Reading list',version:'0.0.0',links:[],offer:{},offerGrants:[{{kind:'network',text:'Connect to example.org'}}]}}],[],[],[]);
          return document.querySelector('#update-offers .offer').textContent==='Review…';
        "#,
                json!(published.info.reference.to_string())
            ),
        );
        click(&app, "#update-offers .offer");
        consent(&app);
        assert!(
            app.eval(
                "consent",
                "document.body.textContent.includes('Connect to example.org')"
            )
            .as_bool()
            .unwrap()
        );
        super::support::journey::deny(&app);
        assert!(app.quit().success());
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
        app.eval("main", "document.body.classList.add('dropping'); null");
        check(
            &app,
            "getComputedStyle(document.querySelector('#drop-target')).display === 'flex' && document.querySelector('#drop-target').textContent.includes('Drop to install') && document.querySelector('#drop-target').textContent.includes('watane reads the mod in this folder and asks before it gets anything.')",
        );
        app.eval("main", "document.body.classList.remove('dropping'); null");
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
        assert!(
            invoke(&app, "mods_list", json!({}))
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["enabled"] == true)
        );
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
            "document.querySelector('#confirm-panel').hidden && document.querySelector('[data-tile=recommendations] .badge')?.textContent === 'Off'",
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
            "Array.from(document.querySelectorAll('.tile')).every(tile => tile.querySelectorAll('.badge').length === 1) && document.querySelector('[data-tile=desk] .badge').textContent === 'Problem' && document.querySelector('[data-tile=desk] .main-mark').textContent === 'Opens at start' && document.querySelector('[data-tile=reading-list] .badge').textContent === 'Problem' && document.querySelector('[data-tile=recommendations] .badge')?.textContent === 'Off'",
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
