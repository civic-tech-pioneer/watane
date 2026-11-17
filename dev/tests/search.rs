#![cfg(feature = "fake-host")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

#[test]
fn ac3_search_help_describes_arguments_without_dir() {
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .args(["search", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(!help.contains("--dir"), "{help}");
    for text in [
        "published mods",
        "[TEXT]...",
        "Words",
        "--publisher",
        "Publisher",
        "--interface",
        "Interface",
        "--home",
        "Home",
    ] {
        assert!(help.contains(text), "missing {text}: {help}");
    }
}

#[test]
fn ac2_missing_app_repeats_quoted_search_with_absolute_home() {
    let root = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(root.path())
        .args([
            "search",
            "--json",
            "Alice's",
            "reading",
            "--publisher",
            "bad key",
            "--interface",
            "mods:books/api",
            "--home",
            "missing home",
        ])
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(3), "{value}");
    assert_eq!(value["command"], "search");
    assert_eq!(value["dir"], Value::Null);
    assert_eq!(value["errors"][0]["code"], "env.host_not_running");
    assert_eq!(
        value["next"],
        format!(
            "watane-dev search --json 'Alice'\"'\"'s' reading --publisher 'bad key' --interface 'mods:books/api' --home '{}/missing home'",
            root.path().canonicalize().unwrap().display()
        )
    );
}

struct Host(Child);
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn start(home: &Path) -> Host {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fake-watane"))
        .arg("--home")
        .arg(home)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let ready: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(ready["mode"], "home");
    Host(child)
}
fn search(home: &Path, args: &[&str]) -> (i32, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(home)
        .args(["search", "--json", "--home", "."])
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}

#[test]
fn s33_results_pass_through() {
    let home = tempfile::tempdir().unwrap();
    let answer = json!({"indexes":2,"results":[
        {"release":"key/reading@0.1.0", "publisher":{"key":"key","name":"Alice"},
         "via":[{"kind":"index","note":"books"}],"future":{"kept":true}},
        {"release":"another/reading@0.2.0","offers":["mods:reading/books"]}
    ]});
    let _host = start(home.path());
    for count in 1..=2 {
        let results = &answer["results"].as_array().unwrap()[..count];
        fs::write(
            home.path().join("fake.json"),
            json!({"search":{"indexes":2,"results":results}}).to_string(),
        )
        .unwrap();
        let (code, value) = search(home.path(), &["reading"]);
        assert_eq!(code, 0, "{value}");
        assert_eq!(value["command"], "search");
        assert_eq!(value["dir"], Value::Null);
        assert_eq!(
            value["result"],
            json!({"home":home.path().canonicalize().unwrap(),"indexes":2,"results":results})
        );
        assert_eq!(value["next"], Value::Null);
        assert_eq!(
            serde_json::from_slice::<Value>(
                &fs::read(home.path().join(format!("requests/{count}-search"))).unwrap()
            )
            .unwrap(),
            json!({"text":"reading","publisher":null,"interface":null})
        );
        assert_eq!(
            fs::read_dir(home.path().join("requests")).unwrap().count(),
            count
        );
    }
}

#[test]
fn s34_no_index_next() {
    let home = tempfile::tempdir().unwrap();
    let _host = start(home.path());
    for indexes in [0, 1, 3] {
        if indexes != 0 {
            fs::write(
                home.path().join("fake.json"),
                json!({"search":{"indexes":indexes,"results":[]}}).to_string(),
            )
            .unwrap();
        }
        let (code, value) = search(home.path(), &["reading"]);
        assert_eq!(code, 0, "{value}");
        assert_eq!(value["result"]["indexes"], indexes);
        assert_eq!(value["result"]["results"], json!([]));
        assert_eq!(
            value["next"],
            if indexes == 0 {
                format!(
                    "Ask the person to add an index in the watane app, then run: watane-dev search --json reading --home {}",
                    home.path().canonicalize().unwrap().display()
                )
            } else {
                "Try other words, or search with --interface <name>: watane-dev search --json <other words>".to_string()
            }
        );
    }
}

#[test]
fn ac1_words_empty_query_filters_and_environment_home() {
    let home = tempfile::tempdir().unwrap();
    let _host = start(home.path());
    for (n, args, text, publisher, interface) in [
        (1, vec![], "", Value::Null, Value::Null),
        (
            2,
            vec![
                "reading",
                "list",
                "--publisher",
                "invalid key",
                "--interface",
                "mods:reading/books",
            ],
            "reading list",
            json!("invalid key"),
            json!("mods:reading/books"),
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(home.path())
            .env("WATANE_HOME", home.path())
            .args(["search", "--json", "--dir", "missing-mod"])
            .args(args)
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(output.status.success(), "{value}");
        assert_eq!(value["dir"], Value::Null);
        assert_eq!(
            serde_json::from_slice::<Value>(
                &fs::read(home.path().join(format!("requests/{n}-search"))).unwrap()
            )
            .unwrap(),
            json!({"text":text,"publisher":publisher,"interface":interface})
        );
    }
}

#[test]
fn ac6_fake_returns_any_configured_search_body() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("fake.json"),
        r#"{"answer":"decline","search":{"unknown":[3,2,1]}}"#,
    )
    .unwrap();
    let _host = start(home.path());
    let run: Value =
        serde_json::from_slice(&fs::read(home.path().join("run.json")).unwrap()).unwrap();
    let mut response = ureq::post(format!("http://localhost:{}/api/search", run["port"]))
        .send_json(json!({"text":""}))
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response.body_mut().read_json::<Value>().unwrap(),
        json!({"unknown":[3,2,1]})
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(home.path().join("requests/1-search")).unwrap())
            .unwrap(),
        json!({"text":""})
    );
}

#[test]
fn ac8_search_text_flattens_results() {
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("fake.json"), r#"{"search":{"indexes":1,"results":[{"name":"Reading","publisher":{"key":"key"},"via":[{"note":"books"}],"link":null}]}}"#).unwrap();
    let _host = start(home.path());
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(home.path())
        .args(["search", "reading", "--home", "."])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for line in [
        "search: ok\n",
        "indexes: 1\n",
        "results.0.name: Reading\n",
        "results.0.publisher.key: key\n",
        "results.0.via.0.note: books\n",
        "results.0.link: missing\n",
    ] {
        assert!(text.contains(line), "missing {line}: {text}");
    }
}

#[test]
fn ac1_search_argument_errors_keep_command_name() {
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .args(["search", "--publisher", "--json"])
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(value["command"], "search");
    assert_eq!(value["dir"], Value::Null);
    assert_eq!(value["errors"][0]["code"], "usage.args");
}

#[test]
fn ac5_search_error_next_is_the_fix() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("fake.json"),
        r#"{"search":{"error":{"code":"unavailable","message":"dist is closed"}}}"#,
    )
    .unwrap();
    let _host = start(home.path());
    let (code, value) = search(home.path(), &["reading"]);
    assert_eq!(code, 4);
    assert_eq!(value["errors"][0]["code"], "host.refused");
    assert_eq!(
        value["errors"][0]["message"],
        "[unavailable] dist is closed"
    );
    assert_eq!(
        value["next"],
        "The watane app could not complete this command; ask the person to check the watane app, then run the command again"
    );
    assert_eq!(value["next"], value["errors"][0]["fix"]);
}

#[test]
fn ac2_preview_is_refused_before_search() {
    let home = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fake-watane"))
        .arg("--preview")
        .arg(home.path())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let _host = Host(child);
    let (code, value) = search(home.path(), &["reading"]);
    assert_eq!(code, 3, "{value}");
    assert_eq!(value["errors"][0]["code"], "env.host_not_running");
    assert_eq!(
        value["errors"][0]["message"],
        format!(
            "the host of {} is a preview",
            home.path().canonicalize().unwrap().display()
        )
    );
    assert_eq!(
        value["next"],
        format!(
            "watane-dev search --json reading --home {}",
            home.path().canonicalize().unwrap().display()
        )
    );
    assert!(!home.path().join("requests").exists());
}

#[test]
fn ac2_search_app_failure_names_app() {
    for status in [503, 500] {
        let home = tempfile::tempdir().unwrap();
        fs::write(
            home.path().join("fake.json"),
            json!({
                "search_status": status,
                "search": {"error":{"code":"unavailable","message":"search unavailable"}}
            })
            .to_string(),
        )
        .unwrap();
        let _host = start(home.path());
        let run: Value =
            serde_json::from_slice(&fs::read(home.path().join("run.json")).unwrap()).unwrap();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(None)
            .http_status_as_error(false)
            .build()
            .into();
        let response = agent
            .post(format!("http://127.0.0.1:{}/api/search", run["port"]))
            .send_json(json!({}))
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
        let (code, value) = search(home.path(), &["books"]);
        assert_eq!(code, if status == 503 { 3 } else { 4 }, "{value}");
        assert_eq!(
            value["errors"][0]["code"],
            if status == 503 {
                "env.host_not_running"
            } else {
                "host.failed"
            }
        );
        assert_eq!(
            value["errors"][0]["message"],
            if status == 503 {
                format!(
                    "the watane app of {} runs but could not open its data: search unavailable",
                    home.path().canonicalize().unwrap().display()
                )
            } else {
                "[unavailable] search unavailable".into()
            }
        );
        assert_eq!(
            value["errors"][0]["fix"],
            if status == 503 {
                "Ask the person to restart the watane app, then run this command again"
            } else {
                "The watane app failed on this request; ask the person to read the watane app's log, then run this command again"
            }
        );
        assert_eq!(value["next"], value["errors"][0]["fix"]);
    }
}

#[test]
fn ac6_search_next_names_home_only_when_given() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("no-app");
    fs::create_dir(&home).unwrap();
    for explicit in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        command
            .args(["search", "--json", "reading"])
            .env("WATANE_HOME", &home);
        if explicit {
            command.arg("--home").arg(&home);
        }
        let output = command.output().unwrap();
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.code(), Some(3), "{value}");
        assert_eq!(
            value["next"],
            format!(
                "watane-dev search --json reading{}",
                if explicit {
                    format!(" --home {}", home.canonicalize().unwrap().display())
                } else {
                    String::new()
                }
            )
        );
    }
}

#[test]
fn ac4_search_with_two_words_and_no_results_tries_the_first_word() {
    let home = tempfile::tempdir().unwrap();
    let _host = start(home.path());
    fs::write(
        home.path().join("fake.json"),
        json!({"search":{"indexes":1,"results":[]}}).to_string(),
    )
    .unwrap();
    let (code, value) = search(home.path(), &["reading", "list"]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(
        value["next"],
        format!(
            "Try fewer words: watane-dev search --json reading --home {}",
            home.path().canonicalize().unwrap().display()
        )
    );
}
