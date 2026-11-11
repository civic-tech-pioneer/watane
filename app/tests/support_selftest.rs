#[path = "support/pages.rs"]
mod pages;

use pages::{Listener, Pages};
use std::{fs, path::Path, time::Duration};

const CSP: &str = "default-src 'self'; script-src 'self'; connect-src 'self'; frame-src 'none'; object-src 'none'";

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

#[test]
fn ac1_pages_serve_each_origin_on_both_loopbacks() {
    let dir = tempfile::tempdir().unwrap();
    for local in ["hostile", "other"] {
        fs::create_dir(dir.path().join(local)).unwrap();
        fs::write(dir.path().join(local).join("index.html"), local).unwrap();
    }
    let pages = Pages::start(dir.path());
    assert_eq!(
        pages.url("hostile"),
        format!("http://hostile.localhost:{}/", pages.port)
    );
    for address in ["127.0.0.1", "[::1]"] {
        for local in ["hostile", "other"] {
            let response = client()
                .get(format!("http://{address}:{}/", pages.port))
                .header("Host", format!("{local}.localhost:{}", pages.port))
                .send()
                .unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(response.text().unwrap(), local);
        }
    }
    let port = pages.port;
    drop(pages);
    for address in ["127.0.0.1", "[::1]"] {
        assert!(std::net::TcpStream::connect(format!("{address}:{port}")).is_err());
    }
}

#[test]
fn ac2_csp_on_success_and_not_found_and_strict_host_routing() {
    let pages = Pages::start(fixtures());
    let url = format!("http://127.0.0.1:{}/", pages.port);
    for (host, status) in [
        (format!("hostile.localhost:{}", pages.port), 200),
        (format!("absent.localhost:{}", pages.port), 404),
        ("hostile.localhost".into(), 404),
        ("hostile.localhost:0".into(), 404),
        (format!("localhost:{}", pages.port), 404),
        (format!("hostile.localhost.evil:{}", pages.port), 404),
        (format!("nested.hostile.localhost:{}", pages.port), 404),
        (format!("../hostile.localhost:{}", pages.port), 404),
    ] {
        let response = client().get(&url).header("Host", &host).send().unwrap();
        assert_eq!(response.status(), status, "{host}");
        assert_eq!(response.headers()["content-security-policy"], CSP);
    }
    let response = client()
        .get(format!("{url}missing.js"))
        .header("Host", format!("hostile.localhost:{}", pages.port))
        .send()
        .unwrap();
    assert_eq!(response.status(), 404);
    assert_eq!(response.headers()["content-security-policy"], CSP);
}

#[test]
fn ac3_listener_counts_connections_and_stops_on_drop() {
    let listener = Listener::start();
    assert_eq!(listener.count(), 0);
    for count in 1..=2 {
        let response = client()
            .get(format!("http://127.0.0.1:{}/", listener.port))
            .send()
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.text().unwrap(), "");
        assert_eq!(listener.count(), count);
    }
    // Count even a socket that sends no HTTP, and keep accepting behind it.
    let idle =
        std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, listener.port)).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while listener.count() != 3 {
        assert!(
            std::time::Instant::now() < deadline,
            "idle connection not counted"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        client()
            .get(format!("http://127.0.0.1:{}/", listener.port))
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(listener.count(), 4);
    let port = listener.port;
    drop(listener);
    drop(idle);
    assert!(std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_err());
}

#[test]
fn ac4_hostile_external_script_reports_every_attack_before_navigation() {
    let pages = Pages::start(fixtures());
    let get = |path: &str| {
        client()
            .get(format!("http://127.0.0.1:{}/{path}", pages.port))
            .header("Host", format!("hostile.localhost:{}", pages.port))
            .send()
            .unwrap()
    };
    let html = get("").text().unwrap();
    assert!(html.contains("<script src=\"hostile.js\" defer></script>"));
    assert_eq!(html.matches("<script").count(), 1);
    let response = get("hostile.js");
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("javascript")
    );
    let script = response.text().unwrap();
    // Execute the HTTP-served script with controlled API successes, failures and
    // timeouts. Browser isolation itself belongs to gui_isolation (S10-4).
    let output = std::process::Command::new("node")
        .arg("-e")
        .arg(REPORT_HARNESS)
        .arg(script)
        .output()
        .expect("node is required for the fixture report self-test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ac5_script_names_all_app_and_plugin_commands() {
    let script = fs::read_to_string(fixtures().join("hostile/hostile.js")).unwrap();
    for command in [
        "status",
        "mods_list",
        "mod_open",
        "mod_enable",
        "mod_disable",
        "mod_remove",
        "grant_withdraw",
        "grant_give",
        "install_folders",
        "tools_list",
        "toolsets_list",
        "toolset_save",
        "tokens_list",
        "token_issue",
        "token_end",
        "install_link",
        "identity_set_name",
        "site_set_url",
        "identity_backup",
        "identity_restore",
        "identity_confirm_own",
        "channels_list",
        "channel_follow",
        "channel_unfollow",
        "channels_refresh",
        "consent_get",
        "consent_answer",
        "plugin:window|create",
        "plugin:event|listen",
        "plugin:dialog|open",
        "plugin:__TAURI_CHANNEL__|fetch",
    ] {
        assert!(
            script.contains(&format!("\"{command}\"")),
            "missing {command}"
        );
    }
}

#[test]
fn ac6_required_release_driver_build_is_guarded() {
    let source = include_str!("../src/lib.rs");
    assert!(source.contains("#[cfg(all(feature = \"driver\", not(debug_assertions)))]"));
    assert!(
        source.contains("compile_error!(\"the driver feature is only for debug test builds\")")
    );
}

const REPORT_HARNESS: &str = r#"
const vm = require('node:vm');
const assert = require('node:assert/strict');
async function run(allowed, before, missing = false) {
  const calls = [], navigations = [], downloads = [];
  const context = { URL, URLSearchParams, Blob, console, Math, Promise,
    setTimeout: (f, ms) => setTimeout(f, ms / 10), clearTimeout,
    localStorage: { getItem: k => { assert.equal(k, 'hostile'); return before; },
      setItem: (k, v) => { assert.equal(k, 'hostile'); assert.equal(typeof v, 'string'); } },
    fetch: async url => { calls.push(String(url)); if (!allowed) throw Error('fetch blocked'); return {text: async () => 'body'}; },
    XMLHttpRequest: class { open(method, url, async) { assert.equal(async, false); calls.push(url); }
      send() { if (!allowed) throw Error('XHR blocked'); this.responseText = 'body'; } },
    WebSocket: class { constructor(url) { calls.push(url); if (allowed) setTimeout(() => this.onopen?.(), 0); } close() {} },
  };
  context.window = context;
  context.location = { search: '?listener=12345&api=23456', port: '23456',
    set href(url) { assert.equal(context.__report.done, true); navigations.push(url); } };
  context.open = url => { calls.push(url); return allowed ? {} : null; };
  if (!missing) context.__TAURI_INTERNALS__ = { invoke: async (cmd, args) => {
    calls.push([cmd, args]); if (!allowed) throw Error('command denied'); return null;
  }};
  const append = element => { if (element.tag === 'a') return;
    calls.push(element.src || element.data);
    element.contentWindow = {location: {href: allowed ? element.src : 'about:blank'}};
    // Blocked frames may still fire load for about:blank.
    if (allowed || element.tag === 'iframe') setTimeout(() => element.onload?.(), 0);
  };
  context.document = { head: {appendChild: append}, body: {appendChild: append},
    createElement: tag => ({tag, remove() {}, click() { downloads.push(this.href); }}) };
  context.URL = class extends URL { static createObjectURL() { return 'blob:fixture'; } static revokeObjectURL() {} };
  vm.runInNewContext(process.argv[1], context);
  const deadline = Date.now() + 5000;
  while (!context.__report?.navTried && Date.now() < deadline) await new Promise(r => setTimeout(r, 5));
  const report = JSON.parse(JSON.stringify(context.__report));
  assert.equal(report.done, true); assert.equal(report.navTried, true);
  assert.equal(report.storageBefore, before);
  const names = report.attacks.map(a => a.name);
  assert.equal(new Set(names).size, names.length);
  assert.equal(report.attacks.length, 109);
  for (const target of ['tauri', 'api', 'other', 'listener']) {
    for (const method of ['fetch', 'xhr']) assert(names.includes(`${method}:${target}`));
  }
  for (const name of ['websocket', 'frame', 'object', 'script', 'window.open', 'download:blob', 'download:data']) assert(names.includes(name));
  for (let id = 0; id < 64; id++) assert(names.includes(`channel:${id}`));
  for (const attack of report.attacks) {
    assert(['allowed', 'refused'].includes(attack.outcome)); assert.equal(typeof attack.detail, 'string');
    if (attack.name.startsWith('download:')) assert.equal(attack.outcome, 'refused');
    else assert.equal(attack.outcome, allowed ? 'allowed' : 'refused', attack.name);
  }
  assert.equal(navigations.length, 2); assert(navigations.includes('tauri://localhost/index.html'));
  assert(navigations.some(url => url.startsWith('http://127.0.0.1:12345/')));
  assert.equal(downloads.length, 2);
  assert(calls.some(url => typeof url === 'string' && url.includes('localhost:23456/api/')));
  assert(calls.some(url => typeof url === 'string' && url.includes('other.localhost:23456')));
  if (!missing) assert.equal(calls.filter(Array.isArray).length, 94);
}
(async () => { await run(false, null); await run(true, 'previous'); await run(false, null, true); })().catch(e => { console.error(e); process.exitCode = 1; });
"#;
