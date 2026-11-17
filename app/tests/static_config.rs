#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::{Value, json};
use std::path::Path;
fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap()
}
#[test]
fn ac8_static_security_configuration() {
    let main = [
        "status",
        "mods_list",
        "mod_open",
        "mod_disable",
        "mod_enable",
        "mod_remove",
        "grant_withdraw",
        "grant_give",
        "install_folders",
        "install_paths",
        "main_set",
        "updates_done",
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
        "machines",
        "machines_on",
        "pair_code",
        "machine_join",
        "machine_remove",
        "machine_role",
        "machines_expiry",
        "relay_add",
        "relay_remove",
        "dht_set",
        "own_hints",
        "identity_confirm_hint",
    ];
    let consent = ["consent_get", "consent_answer"];
    let mut files: Vec<_> = std::fs::read_dir(root().join("capabilities"))
        .unwrap()
        .map(|p| p.unwrap().file_name())
        .collect();
    files.sort();
    assert_eq!(files, ["consent.json", "main.json"]);
    for (label, commands) in [("main", main.as_slice()), ("consent", consent.as_slice())] {
        let cap: Value =
            serde_json::from_str(&read(&format!("capabilities/{label}.json"))).unwrap();
        assert_eq!(
            cap,
            json!({"identifier":label,"windows":[label],"permissions":commands.iter().map(|c| format!("allow-{}", c.replace('_',"-"))).collect::<Vec<_>>()})
        );
    }
    let config: Value = serde_json::from_str(&read("tauri.conf.json")).unwrap();
    assert_eq!(config["build"], json!({"frontendDist":"ui"}));
    assert_eq!(
        config["app"],
        json!({"withGlobalTauri":false,"windows":[],"security":{"csp":"default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src ipc: http://ipc.localhost; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'","freezePrototype":true}})
    );
    let build = read("build.rs");
    let commands = build
        .split(".commands(&[")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_matches('"'))
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        [
            "status",
            "mods_list",
            "mod_open",
            "mod_disable",
            "mod_enable",
            "mod_remove",
            "grant_withdraw",
            "grant_give",
            "install_folders",
            "install_paths",
            "main_set",
            "updates_done",
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
            "machines",
            "machines_on",
            "pair_code",
            "machine_join",
            "machine_remove",
            "machine_role",
            "machines_expiry",
            "relay_add",
            "relay_remove",
            "dht_set",
            "own_hints",
            "identity_confirm_hint",
            "consent_get",
            "consent_answer"
        ]
    );
    for file in std::fs::read_dir(root().join("src")).unwrap() {
        let file = file.unwrap().path();
        if file.extension().is_some_and(|s| s == "rs")
            && !["consent.rs", "commands.rs", "views.rs"]
                .contains(&file.file_name().unwrap().to_str().unwrap())
        {
            assert!(!std::fs::read_to_string(file).unwrap().contains(".answer("));
        }
    }
    assert!(read("Info.plist").contains("NSAllowsLocalNetworking</key><true/>"));
    // ATS refuses http://<local>.localhost without this exception (NOTES S10-4).
    let plist = read("Info.plist");
    assert!(plist.contains("<key>localhost</key><dict><key>NSIncludesSubdomains</key><true/><key>NSExceptionAllowsInsecureHTTPLoads</key><true/></dict>"));
    assert!(!plist.contains("NSAllowsArbitraryLoads"));
    for file in ["ui/app.js", "ui/consent.js"] {
        assert!(!read(file).contains("innerHTML"));
    }
}
#[test]
fn ac9_release_refuses_driver() {
    let source = read("src/lib.rs");
    assert!(source.contains("#[cfg(all(feature = \"driver\", not(debug_assertions)))]"));
    assert!(
        source.contains("compile_error!(\"the driver feature is only for debug test builds\");")
    );
}

#[test]
fn ac1_home_and_port_default_and_port_is_validated() {
    use watane_app::Config;
    // SAFETY: this is the only test in this binary that reads or writes these variables.
    unsafe {
        std::env::remove_var("WATANE_PORT");
        std::env::set_var("WATANE_HOME", "from-env")
    };
    let config = Config::from_args([].into_iter()).unwrap();
    assert_eq!(config.home, Path::new("from-env"));
    assert_eq!(config.port, watane_host::DEFAULT_PORT);
    assert!(
        Config::from_args(
            ["--port", "65536", "--home", "x"]
                .map(Into::into)
                .into_iter()
        )
        .err()
        .unwrap()
        .to_string()
        .contains("bad argument")
    );
    // SAFETY: as above.
    unsafe { std::env::set_var("WATANE_PORT", "4242") };
    let config = Config::from_args(["--home", "example"].map(Into::into).into_iter()).unwrap();
    assert_eq!(config.home, Path::new("example"));
    assert_eq!(config.port, 4242);
    let config = Config::from_args(["--port", "0"].map(Into::into).into_iter()).unwrap();
    assert_eq!(config.port, 0);
    // SAFETY: as above.
    unsafe { std::env::set_var("WATANE_PORT", "nope") };
    Config::from_args([].into_iter()).unwrap_err();
    // SAFETY: as above.
    unsafe {
        std::env::remove_var("WATANE_PORT");
        std::env::remove_var("WATANE_HOME");
    }
}

#[test]
fn ac7_specs_and_notes_record_s44_behavior() {
    let technical = include_str!("../TECHNICAL.md");
    let functional = include_str!("../FUNCTIONAL.md");
    for text in [
        "you",
        "No mods found",
        "Could not read <index>: <cause>",
        "give link or release",
        "method_not_allowed",
        "listed by you",
        "Installed",
    ] {
        assert!(technical.contains(text), "TECHNICAL missing {text}");
        assert!(functional.contains(text), "FUNCTIONAL missing {text}");
    }
    assert!(include_str!("../NOTES.md").contains("S44"));
}

#[test]
fn ac8_specs_record_toolset_removal_and_readable_causes() {
    let technical = read("TECHNICAL.md");
    assert!(technical.contains("| `toolset_remove` | main | `name` | `null` |"));
    assert!(technical.contains("tools: [string]"));
    assert!(technical.contains("give link or release: <serde cause>"));
    assert!(read("FUNCTIONAL.md").contains("remove a saved toolset"));
    assert!(
        !std::fs::read_to_string(root().join("../host/TECHNICAL.md"))
            .unwrap()
            .contains("`remove_toolset`, `toolsets`, and `tokens` wait")
    );
    assert!(read("NOTES.md").contains("S58"));
}
