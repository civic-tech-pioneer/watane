//! Scripted OpenAI CLI isolation and child cleanup (TECHNICAL 19.9).

#![allow(clippy::unwrap_used, reason = "tests may panic")]
use std::fs;
use std::path::Path;
use std::process::Command;

const HOST: &str = env!("CARGO_BIN_EXE_watane");

fn invoke(mode: &str, home: &Path, program: &Path) -> (Option<i32>, serde_json::Value) {
    let output = Command::new(HOST)
        .arg(mode)
        .arg(home)
        .arg("--scripted-openai")
        .arg(program)
        .output()
        .unwrap();
    (
        output.status.code(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}

#[test]
fn scripted_setting_is_rejected_in_a_real_home() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let fake = temp.path().join("missing-program");
    let (status, error) = invoke("--home", &home, &fake);
    assert_eq!(status, Some(2));
    assert_eq!(error["error"]["code"], "usage");
    assert!(!home.exists());

    fs::create_dir(&home).unwrap();
    let marker = r#"{"watane_home":1,"mode":"home"}"#;
    fs::write(home.join("home.json"), marker).unwrap();
    let (status, error) = invoke("--preview", &home, &fake);
    if cfg!(all(watane_dev_profile, debug_assertions)) {
        assert_eq!(status, Some(1));
        assert_eq!(error["error"]["code"], "mode_mismatch");
    } else {
        assert_eq!(status, Some(2));
        assert_eq!(error["error"]["code"], "usage");
    }
    assert_eq!(fs::read_to_string(home.join("home.json")).unwrap(), marker);
    assert!(!home.join("run.json").exists());
}

#[cfg(all(watane_dev_profile, debug_assertions))]
#[test]
fn scripted_child_with_invalid_address_or_ca_is_reaped() {
    use std::os::unix::fs::PermissionsExt;

    for (address, message) in [
        (
            "192.0.2.1:1234",
            "scripted OpenAI requires a nonzero port on 127.0.0.1",
        ),
        (
            "127.0.0.1:0",
            "scripted OpenAI requires a nonzero port on 127.0.0.1",
        ),
        ("127.0.0.1:1234", "invalid scripted OpenAI TLS root"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let fake = temp.path().join("scripted-responses");
        fs::write(
            &fake,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"${{0%/*}}/pid\"\nprintf '%s\\n' '{{\"scripted\":1,\"responses_addr\":\"{address}\",\"ca_pem\":\"invalid\"}}'\nexec /bin/sleep 30\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
        let (status, error) = invoke("--preview", &home, &fake);
        assert_eq!(status, Some(1));
        assert_eq!(error["error"]["code"], "bad_request");
        assert_eq!(
            error["error"]["message"],
            format!("scripted OpenAI: {message}")
        );
        let pid = fs::read_to_string(temp.path().join("pid")).unwrap();
        let status = Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .output()
            .unwrap()
            .status;
        assert!(!status.success());
        assert!(!home.join("run.json").exists());
    }
}

#[test]
fn scripted_judge_shares_the_preview_and_release_guards() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let script = temp.path().join("judge.json");
    fs::write(&script, r#"{"plays":[{"safe":[]}]}"#).unwrap();
    let command = |mode: &str| {
        Command::new(HOST)
            .arg(mode)
            .arg(&home)
            .arg("--scripted-openai")
            .arg(temp.path().join("unexpected-program"))
            .arg("--openai-script")
            .arg(&script)
            .output()
            .unwrap()
    };
    let output = command("--home");
    assert_eq!(output.status.code(), Some(2));
    assert!(!home.exists());
    if !cfg!(all(watane_dev_profile, debug_assertions)) {
        let output = command("--preview");
        assert_eq!(output.status.code(), Some(2));
        assert!(!home.exists());
    }
}
