#[path = "support/pages.rs"]
mod pages;

use pages::{Listener, Pages};
use std::{fs, path::Path, time::Duration};

// Serializes tests that bind loopback ports, so a dropped port is not reused
// by another test before a "stops on drop" check.
static NET: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    let _net = NET.lock().unwrap_or_else(|e| e.into_inner());
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
    let _net = NET.lock().unwrap_or_else(|e| e.into_inner());
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
    let _net = NET.lock().unwrap_or_else(|e| e.into_inner());
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
    let _net = NET.lock().unwrap_or_else(|e| e.into_inner());
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
    assert_eq!(html.matches("<script").count(), 2);
    assert!(html.contains("<script src=\"config.js\"></script>"));
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
    let output = match std::process::Command::new("node")
        .arg("-e")
        .arg(REPORT_HARNESS)
        .arg(script)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("SKIPPED ac4 report harness: node not found on PATH");
            return;
        }
        Err(error) => panic!("run node: {error}"),
    };
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ac5_script_names_all_app_and_plugin_commands() {
    let script = fs::read_to_string(fixtures().join("hostile/ui/hostile.js")).unwrap();
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
        "toolset_remove",
        "tokens_list",
        "token_issue",
        "token_end",
        "install_link",
        "release_install",
        "release_withdraw",
        "identity_set_name",
        "site_set_url",
        "identity_backup",
        "identity_restore",
        "identity_confirm_own",
        "channels_list",
        "channel_follow",
        "channel_unfollow",
        "channels_refresh",
        "indexes_list",
        "index_choose",
        "index_drop",
        "search",
        "install_release",
        "index_list",
        "index_include",
        "index_unlist",
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
  context.fixtureListener = 12345;
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
  assert.equal(report.attacks.length, 120);
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
  if (!missing) assert.equal(calls.filter(Array.isArray).length, 105);
}
(async () => { await run(false, null); await run(true, 'previous'); await run(false, null, true); })().catch(e => { console.error(e); process.exitCode = 1; });
"#;

#[allow(dead_code)]
#[path = "support/driver.rs"]
mod driver;

#[test]
fn ac4_idle_time_parses_ioreg_nanoseconds() {
    assert_eq!(
        driver::idle_time(r#"    | |   "HIDIdleTime" = 30123456789"#),
        Some(Duration::from_nanos(30_123_456_789))
    );
    assert_eq!(
        driver::idle_time(r#"    "HIDIdleTime" = 30123456789"#),
        Some(Duration::from_nanos(30_123_456_789))
    );
    assert_eq!(driver::idle_time("+-o IOHIDSystem"), None);
    assert_eq!(driver::idle_time(r#""HIDIdleTime" = nope"#), None);
    assert_eq!(
        driver::idle_time(r#""OtherHIDIdleTime" = 40000000000"#),
        None
    );
}

#[test]
fn ac1_screen_locked_parses_ioreg_sessions() {
    assert_eq!(
        driver::screen_locked(
            r#""IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=Yes,"CGSSessionScreenIsLocked"=Yes})"#
        ),
        Some(true)
    );
    assert_eq!(
        driver::screen_locked(r#""IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=Yes})"#),
        Some(false)
    );
    assert_eq!(driver::screen_locked("+-o Root"), None);
}

#[test]
fn ac2_front_skips_locked_and_rejects_unknown_ioreg() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    if std::env::var_os("S52_IOREG_CHILD").is_some() {
        let home = tempfile::tempdir().unwrap();
        assert!(driver::App::front(home.path(), &[]).is_none());
        return;
    }
    for scenario in ["locked", "poll", "unknown"] {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("ioreg");
        fs::write(
            &script,
            r#"#!/bin/sh
if [ "$1" = "-c" ]; then
    echo '"HIDIdleTime" = 0'
elif [ "$S52_IOREG_SCENARIO" = unknown ]; then
    echo '+-o Root'
elif [ "$S52_IOREG_SCENARIO" = poll ] && [ ! -f "$S52_IOREG_COUNT" ]; then
    touch "$S52_IOREG_COUNT"
    echo '"IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=Yes})'
else
    echo '"IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=Yes,"CGSSessionScreenIsLocked"=Yes})'
fi
"#,
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!(
            "{}:{}",
            dir.path().display(),
            std::env::var("PATH").unwrap()
        );
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ac2_front_skips_locked_and_rejects_unknown_ioreg",
            ])
            // Keep panic diagnostics on stderr; successful skips must bypass capture.
            .args((scenario == "unknown").then_some("--nocapture"))
            .env("PATH", path)
            .env("S52_IOREG_CHILD", "1")
            .env("S52_IOREG_SCENARIO", scenario)
            .env("S52_IOREG_COUNT", dir.path().join("count"))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if scenario == "unknown" {
            assert!(!output.status.success());
            assert!(
                stderr.contains("ioreg output has no console session"),
                "{stderr}"
            );
        } else {
            assert!(output.status.success(), "{stderr}");
            assert_eq!(
                stderr
                    .matches("front test skipped: the screen is locked (CGSSessionScreenIsLocked)")
                    .count(),
                1,
                "{stderr}"
            );
            if scenario == "locked" {
                assert!(!stderr.contains("waiting for you"), "{stderr}");
            }
        }
    }
}

#[test]
fn ac3_screen_assertions_require_front_constructor_and_ignore() {
    for file in ["gui_shell.rs", "gui_isolation.rs"] {
        let source = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join(file),
        )
        .unwrap();
        for test in source.split("#[test]").skip(1) {
            let visible =
                test.contains("[\"visible\"], true") || test.contains("[\"visible\"] == true");
            let background_probe = test.contains("fn ac2_background_screen_");
            if background_probe {
                assert!(test.contains("App::background("));
            }
            let needs_front = test.contains("document.hasFocus()")
                || test.contains("[\"focused\"] == true")
                || test.contains("[\"focused\"], true")
                || test.contains("fn ac4_approve_delay")
                || (visible && !background_probe);
            if needs_front || test.contains("App::front(") {
                assert!(
                    test.contains("App::front("),
                    "{file}: focus test must call App::front"
                );
                assert!(
                    test.contains("#[ignore = \"front: takes the screen\"]"),
                    "{file}: focus test must be ignored"
                );
            }
        }
    }
}

#[test]
fn ac5_all_window_builders_and_focus_calls_share_screen_helpers() {
    let views =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views.rs")).unwrap();
    assert_eq!(views.matches("screen_builder(app, builder)").count(), 3);
    assert_eq!(views.matches(".set_focus()").count(), 1);
    assert_eq!(views.matches(".request_user_attention(").count(), 1);
    assert!(views.contains("focus(&w, false)?"));
    assert!(views.contains("focus(&w, true)"));
}

#[allow(dead_code)]
#[path = "support/journey.rs"]
mod journey;
#[allow(dead_code)]
#[path = "support/publish.rs"]
mod publish;
#[test]
fn ac8_prebuilt_j4_versions_and_grants_pack_without_building() {
    for (version, network) in [("0.1.0", false), ("0.2.0", false), ("0.3.0", true)] {
        assert!(!publish::update_package(version, network).is_empty());
    }
}

#[allow(dead_code)]
#[path = "support/github.rs"]
mod github;

#[test]
fn ac1_real_github_redacts_token() {
    let token = "made-up-secret-token";
    let result = github::redact(token, &format!("{token}: stdout {token}; stderr {token}"));
    assert!(!result.contains(token));
    assert_eq!(result, "[token]: stdout [token]; stderr [token]");
}

#[cfg(feature = "real-github")]
#[allow(dead_code)]
#[path = "support/dev.rs"]
mod dev;

#[test]
fn ac4_real_github_requires_explicit_feature() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo = fs::read_to_string(app.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("real-github = [\"driver\"]"));
    for journey in ["j3", "j5"] {
        let source = fs::read_to_string(app.join(format!("tests/{journey}.rs"))).unwrap();
        assert!(source.contains(&format!(
            "#[cfg(feature = \"real-github\")]\n#[test]\n#[ignore = \"real: GitHub minmax-fund\"]\nfn ac3_{journey}_real_github()"
        )));
    }
}

#[cfg(feature = "real-github")]
#[test]
fn ac1_real_github_child_environment_and_site_origin_without_network() {
    use std::{ffi::OsStr, process::Command};
    let token = "made-up-github-harness-secret";
    let github = github::RealGithub::with_token(token.into());
    let mut command = Command::new("git");
    github.env(&mut command);
    let env: std::collections::BTreeMap<_, _> = command.get_envs().collect();
    let get = |key: &str| env[OsStr::new(key)].map(|v| v.to_str().unwrap().to_owned());
    assert!(get("GH_TOKEN").as_deref() == Some(token));
    assert_eq!(get("CLAUDE_GH_TOKEN"), None);
    assert_eq!(get("GH_PROMPT_DISABLED").as_deref(), Some("1"));
    assert_eq!(get("GIT_CONFIG_NOSYSTEM").as_deref(), Some("1"));
    assert_eq!(get("GIT_TERMINAL_PROMPT").as_deref(), Some("0"));
    assert!(Path::new(&get("GH_CONFIG_DIR").unwrap()).is_dir());
    let config = fs::read_to_string(get("GIT_CONFIG_GLOBAL").unwrap()).unwrap();
    assert!(!config.contains(token));
    assert!(config.contains("[user]"));
    assert!(config.contains("helper = \n helper = !gh auth git-credential"));
    let home = tempfile::tempdir().unwrap();
    github.site_origin(home.path(), "minmax-fund/watane-j-selftest00");
    let site = home.path().join("dist/site");
    let site = site.to_str().unwrap();
    assert_eq!(
        fs::read_to_string(home.path().join("dist/site/.git/info/exclude")).unwrap(),
        ".*\n"
    );
    assert_eq!(
        github.git(&["-C", site, "branch", "--show-current"]).trim(),
        "main"
    );
    assert_eq!(
        github
            .git(&["-C", site, "config", "remote.origin.url"])
            .trim(),
        "https://github.com/minmax-fund/watane-j-selftest00"
    );
}

// Harness self-tests live here, not in support/github.rs, so they run in one
// test binary rather than in every binary that includes `support`.
#[cfg(feature = "real-github")]
mod github_harness {
    use super::github::{PREFIX, RealGithub};
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        process::Command,
        time::{Duration, Instant},
    };

    #[test]
    fn ac2_pages_wait_status_requires_https_and_redacts_deadline_without_network() {
        let github = RealGithub::with_token("unused-secret".into());
        let panic = std::panic::catch_unwind(|| {
            github.wait_status("http://127.0.0.1/head", 404, Duration::ZERO);
        })
        .unwrap_err();
        let message = panic
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
            .unwrap();
        assert!(message.contains("HTTPS"), "{message}");
        let panic = std::panic::catch_unwind(|| {
            github.wait_status("https://unused-secret.invalid/head", 404, Duration::ZERO);
        })
        .unwrap_err();
        let message = panic.downcast_ref::<String>().unwrap();
        for text in [
            "Pages did not serve https://[token].invalid/head",
            "within",
            "last status: None",
            "last error: None",
            "waited",
        ] {
            assert!(message.contains(text), "{message}");
        }
        assert!(!message.contains("unused-secret"), "{message}");
    }

    #[test]
    fn ac2_pages_wait_requires_https_and_reports_deadline() {
        let github = RealGithub::with_token("unused-secret".into());
        let panic = std::panic::catch_unwind(|| {
            github.wait_served("http://127.0.0.1/head", b"head", Duration::ZERO);
        })
        .unwrap_err();
        assert!(
            panic
                .downcast_ref::<String>()
                .is_some_and(|s| s.contains("HTTPS"))
                || panic
                    .downcast_ref::<&str>()
                    .is_some_and(|s| s.contains("HTTPS"))
        );
        let started = Instant::now();
        let panic = std::panic::catch_unwind(|| {
            github.wait_served(
                "https://127.0.0.1:0/head",
                b"head",
                Duration::from_millis(20),
            );
        })
        .unwrap_err();
        let message = panic.downcast_ref::<String>().unwrap();
        for text in [
            "Pages did not serve https://127.0.0.1:0/head",
            "within",
            "last status",
            "waited",
        ] {
            assert!(message.contains(text), "{message}");
        }
        assert!(started.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn ac2_pages_wait_reports_client_error_and_attempts_tls() {
        use std::io::Write;
        let _net = super::NET.lock().unwrap_or_else(|e| e.into_inner());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!(
            "https://{}/unused-secret/head",
            listener.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Plain HTTP on an HTTPS connection must cause a TLS error.
                        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
                        return true;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return false;
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("local listener: {error}"),
                }
            }
        });
        let github = RealGithub::with_token("unused-secret".into());
        let started = Instant::now();
        let panic = std::panic::catch_unwind(|| {
            github.wait_served(&url, b"head", Duration::from_millis(100));
        })
        .unwrap_err();
        let connected = server.join().unwrap();
        let message = panic.downcast_ref::<String>().unwrap();
        assert!(
            message.contains("last status: None; last error: Some("),
            "{message}"
        );
        assert!(!message.contains("unused-secret"), "{message}");
        assert!(message.contains("[token]"), "{message}");
        assert!(connected, "HTTPS client never connected: {message}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    // Run the harness in a child with its own PATH; never change the test
    // process's environment or invoke the installed gh.
    #[test]
    fn ac2_reserve_create_and_drop_without_network() {
        run_scenarios(&[
            "unused", "forked", "create", "exists", "auth", "not-http", "guard",
        ]);
    }

    #[test]
    fn ac1_pages_setup_and_guards_without_network() {
        run_scenarios(&["pages", "pages-http", "pages-guard", "pages-unreserved"]);
    }

    fn run_scenarios(scenarios: &[&str]) {
        for &scenario in scenarios {
            let root = tempfile::tempdir().unwrap();
            let gh = root.path().join("gh");
            fs::write(
                &gh,
                r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$WATANE_GH_STUB_ROOT/calls"
case "$*" in
  'api --include repos/'*)
case "$WATANE_GH_SCENARIO" in
  exists) printf 'HTTP/2.0 200 OK\n'; exit 0 ;;
  auth) printf 'HTTP/2.0 401 Unauthorized\n'; exit 1 ;;
  not-http) printf '404 is not an HTTP response\n'; exit 1 ;;
esac
printf 'HTTP/2.0 404 Not Found\n'; exit 1 ;;
  'api --method POST repos/'*)
if test "$WATANE_GH_SCENARIO" = pages-http; then
  printf '{"html_url":"http://minmax-fund.github.io/watane-j-stub/"}'
else
  printf '{"html_url":"https://minmax-fund.github.io/watane-j-stub///"}'
fi ;;
  'repo create '*) touch "$WATANE_GH_STUB_ROOT/created" ;;
  'api --include --method DELETE repos/'*)
if test -e "$WATANE_GH_STUB_ROOT/created"; then
  rm "$WATANE_GH_STUB_ROOT/created"
  printf 'HTTP/2.0 204 No Content\n'
else
  printf 'HTTP/2.0 404 Not Found\n'; exit 1
fi ;;
  *) printf 'unexpected gh command\n' >&2; exit 2 ;;
esac
"#,
            )
            .unwrap();
            fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
            let path = std::env::join_paths(
                std::iter::once(root.path().to_path_buf())
                    .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
            )
            .unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["ac2_reservation_scenario", "--nocapture"])
                .env("PATH", path)
                .env("WATANE_GH_STUB_ROOT", root.path())
                .env("WATANE_GH_SCENARIO", scenario)
                .env_remove("CLAUDE_GH_TOKEN")
                .env_remove("GH_TOKEN")
                .output()
                .unwrap();
            assert!(output.status.success(), "{scenario}: {output:?}");
            let calls = fs::read_to_string(root.path().join("calls")).unwrap_or_default();
            if ["pages", "pages-http"].contains(&scenario) {
                let post = calls
                    .lines()
                    .find(|line| line.contains("--method POST"))
                    .unwrap();
                assert!(
                    post.starts_with("api --method POST repos/minmax-fund/watane-j-"),
                    "{post}"
                );
                assert!(
                    post.ends_with("/pages -f source[branch]=main -f source[path]=/"),
                    "{post}"
                );
            }
            if ["unused", "forked", "create", "pages", "pages-http"].contains(&scenario) {
                if scenario != "pages-http" {
                    assert!(output.stderr.is_empty(), "{scenario}: {output:?}");
                }
                assert_eq!(
                    calls.matches("--method DELETE").count(),
                    if ["create", "pages", "pages-http"].contains(&scenario) {
                        1
                    } else {
                        2
                    }
                );
                assert_eq!(
                    calls.matches("repo create").count(),
                    usize::from(scenario == "create")
                );
                assert!(!root.path().join("created").exists());
            } else {
                assert!(!calls.contains("DELETE"), "{calls}");
                assert!(!calls.contains("repo create"), "{calls}");
                assert!(!calls.contains("--method POST"), "{calls}");
            }
        }
    }

    #[test]
    fn ac2_reservation_scenario() {
        let Ok(scenario) = std::env::var("WATANE_GH_SCENARIO") else {
            return;
        };
        let mut github = RealGithub::with_token("dummy-harness-token".into());
        if ["pages-guard", "pages-unreserved"].contains(&scenario.as_str()) {
            let name = if scenario == "pages-guard" {
                "minmax-fund/do-not-touch"
            } else {
                "minmax-fund/watane-j-unreserved"
            };
            assert!(std::panic::catch_unwind(|| github.pages(name)).is_err());
        } else if scenario == "guard" {
            github.repos.push("minmax-fund/do-not-touch".into());
            assert!(std::panic::catch_unwind(|| drop(github)).is_err());
        } else if ["exists", "auth", "not-http"].contains(&scenario.as_str()) {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| github.reserve()))
                    .is_err()
            );
            assert!(github.repos.is_empty());
        } else {
            let name = if scenario == "create" {
                github.create()
            } else {
                github.reserve()
            };
            if scenario.starts_with("pages") {
                let result = std::panic::catch_unwind(|| github.pages(&name));
                if scenario == "pages-http" {
                    assert!(result.is_err());
                } else {
                    assert_eq!(
                        result.unwrap(),
                        "https://minmax-fund.github.io/watane-j-stub/"
                    );
                }
                drop(github);
                return;
            }
            let suffix = name.strip_prefix(PREFIX).unwrap();
            assert_eq!(suffix.len(), 10);
            assert!(suffix.as_bytes()[0].is_ascii_lowercase());
            assert!(
                suffix
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            );
            assert_eq!(github.repos.as_slice(), std::slice::from_ref(&name));
            if scenario != "create" {
                let other = github.reserve();
                assert_ne!(name, other);
            }
            let created =
                std::path::PathBuf::from(std::env::var_os("WATANE_GH_STUB_ROOT").unwrap())
                    .join("created");
            assert_eq!(created.exists(), scenario == "create");
            if scenario == "forked" {
                fs::write(created, "forked by publish").unwrap();
            }
            drop(github);
        }
    }
}

#[test]
fn ac5_missing_base_reports_http_not_found_after_install_fetch() {
    let _net = NET.lock().unwrap_or_else(|e| e.into_inner());
    struct Approve;
    impl watane_dist::Confirm for Approve {
        fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
            watane_dist::Decision::Approve
        }
    }
    let alice_home = tempfile::tempdir().unwrap();
    let bob_home = tempfile::tempdir().unwrap();
    let open = |home: &Path| {
        watane_dist::Dist::open(home, std::sync::Arc::new(Approve), Default::default()).unwrap()
    };
    let alice = open(alice_home.path());
    let site = Pages::start(&alice_home.path().join("dist"));
    let base = site.url("site");
    alice.set_site_url(&base).unwrap();
    let package = watane_host::watane_pkg::pack(&fixtures().join("reading-list"), false).unwrap();
    let link = alice
        .publish(package.as_bytes().to_vec())
        .unwrap()
        .link
        .unwrap();
    let bob = open(bob_home.path());
    bob.open_link(&link).unwrap();
    // A followed channel uses its stored base rather than the new link.
    bob.unfollow(&link.channel()).unwrap();
    let missing_base = format!("{base}missing-unpublished/");
    let missing = watane_dist::Link::new(link.channel(), &missing_base, link.slug()).unwrap();
    let started = std::time::Instant::now();
    let error = bob.open_link(&missing).expect_err("missing base must fail");
    assert!(
        matches!(&error, watane_dist::DistError::NotFound { url } if url == &format!("{missing_base}head")),
        "{error:?}"
    );
    let error = watane_app::CmdError::from(error);
    assert_eq!(error.code, "dist");
    assert_eq!(error.message, format!("not found: {missing_base}head"));
    assert!(started.elapsed() < Duration::from_secs(60));
}

/// S54 acceptance, real run 2026-11-16: right after `wait_served` saw Alice's `head`,
/// Pages answered `HTTP 503` (not the CDN's 404) for the never-published
/// `<base>missing-<r>/head`, so Bob saw "HTTP 503 from ..." and the real J3 failed at
/// AC5. A minute later the same URL gave 404. The journey must not assume the CDN
/// answers a miss with 404 the moment `head` is served: it has to see the 404 for the
/// missing URL (with a deadline) before Bob's install is expected to say `not found`.
#[test]
fn regress_s54_real_j3_waits_for_cdn_404_before_missing_install() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(app.join("tests/j3.rs")).unwrap();
    let start = source.find("fn ac3_j3_real_github()").unwrap();
    let journey = &source[start..];
    let install = journey
        .find("begin(&dan, \"install_link\", json!({\"link\":missing")
        .unwrap();
    assert!(
        journey[..install].contains("{missing_base}head"),
        "ac3_j3_real_github installs the missing link without first waiting for Pages to \
         answer 404 for {{missing_base}}head; Pages answered 503 there in a real run"
    );
}

/// A known channel uses recorded bases, including records learned through Ivy.
/// Real J3 therefore checks a missing base from a fresh home with no records.
#[test]
fn regress_s54_missing_base_fresh_home_reports_not_found() {
    let _net = NET.lock().unwrap_or_else(|e| e.into_inner());
    struct Approve;
    impl watane_dist::Confirm for Approve {
        fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
            watane_dist::Decision::Approve
        }
    }
    let alice_home = tempfile::tempdir().unwrap();
    let dan_home = tempfile::tempdir().unwrap();
    let open = |home: &Path| {
        watane_dist::Dist::open(home, std::sync::Arc::new(Approve), Default::default()).unwrap()
    };
    let alice = open(alice_home.path());
    alice.set_name("Alice").unwrap();
    let site = Pages::start(&alice_home.path().join("dist"));
    let base = site.url("site");
    alice.set_site_url(&base).unwrap();
    let package = watane_host::watane_pkg::pack(&fixtures().join("reading-list"), false).unwrap();
    let link = alice
        .publish(package.as_bytes().to_vec())
        .unwrap()
        .link
        .unwrap();
    let dan = open(dan_home.path());
    assert!(dan.followed().unwrap().is_empty());
    assert!(dan.indexes().unwrap().is_empty());
    assert!(!dan_home.path().join("dist/follows.json").exists());
    let missing_base = format!("{base}missing-unpublished/");
    let missing = watane_dist::Link::new(link.channel(), &missing_base, link.slug()).unwrap();
    let error = dan
        .open_link(&missing)
        .expect_err("a fresh home must read the link's missing base");
    assert!(
        matches!(&error, watane_dist::DistError::NotFound { url } if url == &format!("{missing_base}head")),
        "{error:?}"
    );
}
