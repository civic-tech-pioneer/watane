//! Step 6 (TECHNICAL 10.4, 15.4): `publish --push` and the fork of `new --from`.
//! Decisions run dev in-process on a scripted git and gh (`share::with_runner`);
//! the `_e2e_` tests run the real program, git and `fake-gh`.
#![cfg(feature = "fake-host")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::{Value, json};
use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::rc::Rc;

struct Host(Child);
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Fixture {
    _root: tempfile::TempDir,
    root: PathBuf,
    dir: PathBuf,
    home: PathBuf,
    _host: Host,
}
impl Fixture {
    /// A page-only mod `demo` and a fake home host; no git or gh yet.
    fn new() -> Self {
        let temp = tempfile::Builder::new()
            .prefix("Alice's share ")
            .tempdir()
            .unwrap();
        let root = temp.path().canonicalize().unwrap();
        let dir = root.join("demo");
        let home = root.join("home");
        for p in [dir.join("wit"), dir.join("ui"), home.clone()] {
            fs::create_dir_all(p).unwrap();
        }
        fs::write(
            dir.join("mod.toml"),
            "name = \"Demo\"\nsummary = \"a page\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("wit/world.wit"),
            "package mods:demo@0.1.0; world demo {}",
        )
        .unwrap();
        fs::write(dir.join("ui/index.html"), "<title>Demo</title>").unwrap();
        fs::write(dir.join("private.txt"), "must never be committed").unwrap();
        fs::write(home.join("fake.json"), r#"{"link":null}"#).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_fake-watane"))
            .arg("--home")
            .arg(&home)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        Self {
            _root: temp,
            root,
            dir,
            home,
            _host: Host(child),
        }
    }
    fn unchanged(&self, old: &str) {
        assert_eq!(fs::read_to_string(self.dir.join("mod.toml")).unwrap(), old);
        assert!(!self.home.join("requests").exists());
    }
    fn publish_args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = ["watane-dev", "publish", "--push", "--json", "--dir"]
            .map(OsString::from)
            .into();
        args.extend([
            self.dir.clone().into(),
            "--home".into(),
            self.home.clone().into(),
        ]);
        args
    }
}

// ---- In-process runs on a scripted git and gh ----

type Reply = std::io::Result<Output>;
fn reply(code: i32, stdout: &str, stderr: &str) -> Reply {
    #[cfg(unix)]
    let status = ExitStatus::from_raw(code << 8);
    #[cfg(windows)]
    let status = ExitStatus::from_raw(u32::try_from(code).unwrap());
    Ok(Output {
        status,
        stdout: stdout.into(),
        stderr: stderr.into(),
    })
}
fn ok(stdout: &str) -> Reply {
    reply(0, stdout, "")
}
fn not_found() -> Reply {
    reply(1, "", "gh: Not Found (HTTP 404)\n")
}
fn missing() -> Reply {
    Err(std::io::ErrorKind::NotFound.into())
}
/// The world without a script: gh logged in as alice, no repository on
/// GitHub, no remote URL set, something staged, branch `main`.
fn default_answer(command: &str) -> Reply {
    if command == "gh api user --jq .login" {
        ok("alice\n")
    } else if command.starts_with("gh api repos/") && command.ends_with(" --jq .parent.full_name") {
        not_found()
    } else if command.contains(" config --get ") || command.ends_with(" diff --cached --quiet") {
        reply(1, "", "")
    } else if command.ends_with(" branch --show-current") {
        ok("main\n")
    } else {
        ok("")
    }
}
fn release(slug: &str) -> String {
    format!("{}/{slug}@0.1.0", "y".repeat(52))
}
struct Run {
    code: u8,
    value: Value,
    /// Each git and gh command: `<mod>`, `<site>` and `<root>` for the folders,
    /// without the `-c http.lowSpeed*` guard (`share::tests` pins it).
    log: Vec<String>,
}
impl Fixture {
    /// Runs `args` in-process; `script` answers a command first, else `default_answer`.
    fn dev(
        &self,
        args: &[OsString],
        mut script: impl FnMut(&str) -> Option<Reply> + 'static,
    ) -> Run {
        let log = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&log);
        let names = [
            (self.home.join("site"), "<site>"),
            (self.dir.clone(), "<mod>"),
            (self.root.clone(), "<root>"),
        ]
        .map(|(path, name)| (path.display().to_string(), name));
        let runner = move |program: &str, args: &[String]| {
            let mut command = format!("{program} {}", args.join(" "))
                .replace(" -c http.lowSpeedLimit=1000 -c http.lowSpeedTime=60", "");
            for (path, name) in &names {
                command = command.replace(path, name);
            }
            #[cfg(windows)]
            let command = command.replace("<root>\\", "<root>/");
            if let ["-C", dir, "init", ..] = args.iter().map(String::as_str).collect::<Vec<_>>()[..]
            {
                fs::create_dir_all(Path::new(dir).join(".git/info")).unwrap();
            }
            seen.borrow_mut().push(command.clone());
            script(&command).unwrap_or_else(|| default_answer(&command))
        };
        let mut out = Vec::new();
        let cache = self.home.join("cache");
        let code = watane_dev::share::with_runner(Box::new(runner), || {
            watane_dev::cli::main_with(
                args,
                &[("WATANE_DEV_CACHE", Some(cache.as_os_str()))],
                None,
                &mut out,
            )
        });
        let value = serde_json::from_slice(&out)
            .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&out)));
        let log = log.borrow().clone();
        Run { code, value, log }
    }
    fn push(&self, args: &[&str], script: impl FnMut(&str) -> Option<Reply> + 'static) -> Run {
        let mut all = self.publish_args();
        all.extend(args.iter().map(OsString::from));
        self.dev(&all, script)
    }
}
fn preflight() -> Vec<String> {
    v(&[
        "git -C <mod> --version",
        "git -C <mod> var GIT_COMMITTER_IDENT",
        "gh api user --jq .login",
    ])
}
fn site_push(owner: &str) -> Vec<String> {
    [
        format!("gh api repos/{owner}/watane-channel --jq .parent.full_name"),
        format!("gh repo create {owner}/watane-channel --public"),
        "git -C <site> init -q -b main".into(),
        format!("git -C <site> remote add origin -- https://github.com/{owner}/watane-channel"),
        "git -C <site> add -A -- head packages".into(),
        "git -C <site> diff --cached --quiet".into(),
        format!("git -C <site> commit -q -m {}", release("demo")),
        "git -C <site> push -q origin HEAD".into(),
    ]
    .into()
}
fn mod_push(paths: &str) -> Vec<String> {
    [
        "git -C <mod> branch --show-current".into(),
        "git -C <mod> rm -r -q --cached --ignore-unmatch .".into(),
        format!("git -C <mod> add -f -- {paths}"),
        "git -C <mod> diff --cached --quiet".into(),
        "git -C <mod> commit -q -m demo 0.1.0".into(),
        format!("git -C <mod> tag {}", release("demo")),
        format!(
            "git -C <mod> push -q origin HEAD refs/tags/{}",
            release("demo")
        ),
    ]
    .into()
}
const PAGE: &str = "mod.toml ui/index.html wit/deps/watane-host/host.wit wit/world.wit";
fn v(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn ac1_first_push_runs_the_commands_of_10_4_1() {
    let f = Fixture::new();
    let run = f.push(&[], |_| None);
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(
        run.log,
        [
            preflight(),
            v(&[
                "gh api repos/alice/demo --jq .parent.full_name",
                "gh repo create alice/demo --public",
                "git -C <mod> ls-remote -- https://github.com/alice/demo",
                "git -C <mod> init -q -b main",
                "git -C <mod> remote add origin -- https://github.com/alice/demo",
            ]),
            site_push("alice"),
            mod_push(PAGE),
        ]
        .concat()
    );
    assert_eq!(
        run.value["result"]["push"],
        json!({"site":{"repository":"https://github.com/alice/watane-channel","raw":"https://raw.githubusercontent.com/alice/watane-channel/main/","pages":"https://alice.github.io/watane-channel/","created":true,"pushed":true},"mod":{"repository":"https://github.com/alice/demo","created":true,"pushed":true,"forked":false,"tag":release("demo")}})
    );
    for dir in [&f.dir, &f.home.join("site")] {
        assert_eq!(
            fs::read_to_string(dir.join(".git/info/exclude")).unwrap(),
            ".*\n"
        );
    }
}

#[test]
fn ac1_nothing_staged_skips_the_commit() {
    let f = Fixture::new();
    let run = f.push(&[], |c| {
        c.ends_with(" diff --cached --quiet").then(|| ok(""))
    });
    assert_eq!(run.code, 0, "{}", run.value);
    assert!(
        !run.log.iter().any(|c| c.contains(" commit ")),
        "{:?}",
        run.log
    );
    assert!(run.log.iter().any(|c| c.starts_with("git -C <mod> tag ")));
}

#[test]
fn ac1_pushed_repository_is_fetched_and_reset_before_the_commit() {
    let f = Fixture::new();
    fs::write(
        f.dir.join("mod.toml"),
        "name = \"Demo\"\nsummary = \"a page\"\nrepository = \"https://github.com/alice/demo\"\n",
    )
    .unwrap();
    let run = f.push(&[], |c| {
        c.ends_with(" ls-remote -- https://github.com/alice/demo")
            .then(|| ok("abc\trefs/heads/main\n"))
    });
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(
        run.log[3..8],
        v(&[
            "git -C <mod> ls-remote -- https://github.com/alice/demo",
            "git -C <mod> init -q -b main",
            "git -C <mod> remote add origin -- https://github.com/alice/demo",
            "git -C <mod> fetch -q origin",
            "git -C <mod> reset -q origin/main",
        ])
    );
    assert_eq!(run.value["result"]["push"]["mod"]["created"], false);
}

#[test]
fn ac3_missing_git_or_gh_stops_before_anything() {
    for (program, code, steps) in [("git", "env.git_missing", 1), ("gh", "env.gh_missing", 3)] {
        let f = Fixture::new();
        let old = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
        let run = f.push(&[], move |c| {
            c.starts_with(&format!("{program} ")).then(missing)
        });
        assert_eq!(run.code, 3, "{}", run.value);
        assert_eq!(run.value["errors"][0]["code"], code);
        assert_eq!(run.log, preflight()[..steps]);
        f.unchanged(&old);
    }
}

#[test]
fn ac3_gh_not_logged_in_or_no_git_identity() {
    let f = Fixture::new();
    let old = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
    let run = f.push(&[], |c| {
        (c == "gh api user --jq .login").then(|| {
            reply(
                4,
                "",
                "To get started with GitHub CLI, please run:  gh auth login\n",
            )
        })
    });
    assert_eq!(run.code, 3, "{}", run.value);
    assert_eq!(run.value["errors"][0]["code"], "env.gh_auth");
    assert!(
        run.value["errors"][0]["fix"]
            .as_str()
            .unwrap()
            .contains("gh auth login")
    );
    assert_eq!(run.log, preflight());
    f.unchanged(&old);

    let run = f.push(&[], |c| {
        c.ends_with(" var GIT_COMMITTER_IDENT").then(|| {
            reply(
                128,
                "",
                "*** Please tell me who you are.\n\nfatal: unable to auto-detect email address (got 'alice@box.(none)')\n",
            )
        })
    });
    assert_eq!(run.code, 3, "{}", run.value);
    assert_eq!(run.value["errors"][0]["code"], "env.git_identity");
    assert_eq!(
        run.value["errors"][0]["message"],
        "git has no name and email for commits: fatal: unable to auto-detect email address"
    );
    assert_eq!(run.log, preflight()[..2]);
    f.unchanged(&old);

    // No network: another gh failure is publish.push_failed.
    let run = f.push(&[], |c| {
        (c == "gh api user --jq .login")
            .then(|| reply(1, "", "error connecting to api.github.com\n"))
    });
    assert_eq!(run.code, 4, "{}", run.value);
    assert_eq!(run.value["errors"][0]["code"], "publish.push_failed");
    f.unchanged(&old);
}

#[test]
fn ac4_refused_push_reports_release_and_finishing_commands() {
    let f = Fixture::new();
    let tags = format!(" push -q origin HEAD refs/tags/{}", release("demo"));
    let run = f.push(&[], move |c| {
        c.ends_with(&tags)
            .then(|| reply(1, "", "remote: remote: error: denied   \nremote: \n\n"))
    });
    assert_eq!(run.code, 4, "{}", run.value);
    let quoted = format!("'{}'", f.dir.display().to_string().replace('\'', "'\"'\"'"));
    let fix = format!(
        "git -C {quoted} push -q origin HEAD refs/tags/{}",
        release("demo")
    );
    let error = &run.value["errors"][0];
    assert_eq!(error["code"], "publish.push_failed");
    assert_eq!(
        error["message"],
        format!("{fix} failed: remote: error: denied")
    );
    assert_eq!(error["fix"], fix.as_str());
    assert_eq!(run.value["errors"].as_array().unwrap().len(), 1);
    assert_eq!(run.value["next"], fix.as_str());
    assert_eq!(run.value["result"]["release"], release("demo"));
    assert_eq!(run.value["result"]["push"]["site"]["pushed"], true);
    assert_eq!(run.value["result"]["push"]["mod"]["pushed"], false);
}

#[test]
fn ac4_failures_in_both_lists_join_their_finishing_commands() {
    let f = Fixture::new();
    let run = f.push(&[], |c| {
        (c.starts_with("git -C <mod> tag ") || c == "git -C <site> push -q origin HEAD")
            .then(|| reply(1, "", "fatal: no\n"))
    });
    assert_eq!(run.code, 4, "{}", run.value);
    let quote = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "'\"'\"'"));
    let site = format!("git -C {} push -q origin HEAD", quote(&f.home.join("site")));
    let tag = format!(
        "git -C {0} tag {1} && git -C {0} push -q origin HEAD refs/tags/{1}",
        quote(&f.dir),
        release("demo")
    );
    assert_eq!(run.value["errors"][0]["fix"], site.as_str());
    assert_eq!(run.value["errors"][1]["fix"], tag.as_str());
    assert_eq!(run.value["next"], format!("{site} && {tag}"));
    assert_eq!(run.value["result"]["push"]["site"]["pushed"], false);
    assert_eq!(run.value["result"]["push"]["mod"]["pushed"], false);
}

#[test]
fn ac5_no_source_pushes_only_the_site() {
    let f = Fixture::new();
    let old = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
    let run = f.push(&["--no-source"], |_| None);
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(run.log, [preflight(), site_push("alice")].concat());
    assert!(run.value["result"]["push"]["mod"].is_null());
    assert_eq!(run.value["result"]["push"]["site"]["created"], true);
    assert_eq!(fs::read_to_string(f.dir.join("mod.toml")).unwrap(), old);
}

#[test]
fn ac5_origin_differs_and_foreign_repository_stop_before_anything() {
    for case in ["missing", "different", "nonempty"] {
        let f = Fixture::new();
        if case != "nonempty" {
            fs::create_dir(f.dir.join(".git")).unwrap();
        }
        if case == "different" {
            fs::write(f.dir.join("mod.toml"),"name = \"Demo\"\nsummary = \"a page\"\nrepository = \"https://github.com/alice/demo\"\n").unwrap();
        }
        let old = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
        let run = f.push(&[], |c| {
            if c.ends_with(" config --get remote.origin.url") {
                Some(ok("https://github.com/bob/other\n"))
            } else if c == "gh api repos/alice/demo --jq .parent.full_name" {
                Some(ok("null\n"))
            } else if c.contains(" ls-remote ") {
                Some(ok("abc\trefs/heads/main\n"))
            } else {
                None
            }
        });
        assert_eq!(run.code, 1, "{}", run.value);
        let (code, last) = if case == "nonempty" {
            (
                "publish.repository_exists",
                "git -C <mod> ls-remote -- https://github.com/alice/demo",
            )
        } else {
            (
                "publish.origin_differs",
                "git -C <mod> config --get remote.origin.url",
            )
        };
        assert_eq!(run.value["errors"][0]["code"], code);
        assert_eq!(run.log.last().unwrap(), last, "{:?}", run.log);
        assert!(!run.log.iter().any(|c| c.contains(" create ")));
        f.unchanged(&old);
    }
}

#[test]
fn ac5_named_repository_is_used_on_any_host() {
    let f = Fixture::new();
    fs::write(
        f.dir.join("mod.toml"),
        "name = \"Demo\"\nsummary = \"a page\"\nrepository = \"https://git.example.test/a/demo.git\"\n",
    )
    .unwrap();
    let run = f.push(&[], |_| None);
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(
        run.log[3..6],
        v(&[
            "git -C <mod> ls-remote -- https://git.example.test/a/demo.git",
            "git -C <mod> init -q -b main",
            "git -C <mod> remote add origin -- https://git.example.test/a/demo.git",
        ])
    );
    assert_eq!(
        run.value["result"]["push"]["mod"]["repository"],
        "https://git.example.test/a/demo.git"
    );
}

#[test]
fn ac6_prebuilt_mod_keeps_logic_wasm_in_commit() {
    let f = Fixture::new();
    prebuilt(&f.dir);
    let run = f.push(&[], |_| None);
    assert_eq!(run.code, 0, "{}", run.value);
    let add = run
        .log
        .iter()
        .find(|c| c.starts_with("git -C <mod> add -f -- "))
        .unwrap();
    assert!(add.contains(" logic.wasm "), "{add}");
    assert!(!add.contains("private.txt"), "{add}");
}
/// The ratings fixture's prebuilt component in the `demo` folder.
fn prebuilt(dir: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ratings");
    fs::create_dir_all(dir.join("wit/deps/watane-host")).unwrap();
    for name in [
        "logic.wasm",
        "wit/world.wit",
        "wit/deps/watane-host/host.wit",
    ] {
        fs::copy(source.join(name), dir.join(name)).unwrap();
    }
    fs::create_dir(dir.join("tests")).unwrap();
    fs::write(
        dir.join("tests/test.json"),
        r#"{"steps":[{"call":"api.get","returns":0}]}"#,
    )
    .unwrap();
}

#[test]
fn ac3_missing_app_retry_keeps_push_and_package_options() {
    let f = Fixture::new();
    fs::remove_file(f.home.join("run.json")).unwrap();
    let run = f.push(&["--no-source", "--wait", "7"], |_| None);
    assert_eq!(run.code, 3, "{}", run.value);
    let next = run.value["next"].as_str().unwrap();
    assert!(next.contains("--push"), "{next}");
    assert!(next.contains("--no-source"), "{next}");
    assert!(next.contains("--wait 7"), "{next}");
    assert!(run.log.is_empty());
    assert!(!f.home.join("requests").exists());
}

#[test]
fn ac2_owner_creates_and_pushes_both_repositories() {
    let f = Fixture::new();
    let run = f.push(&["--owner", "acme"], |_| None);
    assert_eq!(run.code, 0, "{}", run.value);
    for (part, name) in [("mod", "demo"), ("site", "watane-channel")] {
        assert_eq!(
            run.value["result"]["push"][part]["repository"],
            format!("https://github.com/acme/{name}")
        );
        assert_eq!(run.value["result"]["push"][part]["created"], true);
        assert_eq!(run.value["result"]["push"][part]["pushed"], true);
    }
    assert!(
        run.log
            .contains(&"gh repo create acme/demo --public".into())
    );
    assert!(
        run.log.iter().all(|c| !c.contains("alice/")),
        "{:?}",
        run.log
    );
    assert!(
        fs::read_to_string(f.dir.join("mod.toml"))
            .unwrap()
            .contains("repository = \"https://github.com/acme/demo\"")
    );
    assert!(
        run.value["next"]
            .as_str()
            .unwrap()
            .ends_with(" --push --owner acme")
    );
}

#[test]
fn ac1_owner_preserved_in_recovery_commands() {
    let f = Fixture::new();
    // Missing home: the app-start instruction carries the selected account.
    let mut args = f.publish_args();
    *args.last_mut().unwrap() = f.root.join("absent").into();
    args.extend(["--owner", "acme"].map(OsString::from));
    let run = f.dev(&args, |_| None);
    assert_eq!(run.code, 3, "{}", run.value);
    assert!(
        run.value["next"]
            .as_str()
            .unwrap()
            .contains("--push --owner acme")
    );
    // Preflight fixes retain the same account.
    let run = f.push(&["--owner", "acme"], |c| {
        (c == "gh api user --jq .login").then(|| reply(4, "", "HTTP 401\n"))
    });
    assert_eq!(run.value["errors"][0]["code"], "env.gh_auth");
    assert!(
        run.value["errors"][0]["fix"]
            .as_str()
            .unwrap()
            .contains("--push --owner acme")
    );
    fs::write(f.dir.join("deps.json"), r#"{"mods:provider":null}"#).unwrap();
    fs::write(
        f.dir.join(".watane-dev.json"),
        r#"{"local_providers":{"provider":"../provider"}}"#,
    )
    .unwrap();
    let run = f.push(&["--owner", "acme"], |_| None);
    assert_eq!(run.code, 1, "{}", run.value);
    assert_eq!(run.value["errors"][0]["code"], "publish.local_dependency");
    assert!(
        run.value["next"]
            .as_str()
            .unwrap()
            .ends_with("--push --owner acme")
    );
}

// ---- Derived mods: the fork of `new --from` (10.4.2) and its first push ----

const UPSTREAM: &str = "https://github.com/alice/reading-list";
fn parent_tag() -> String {
    format!("{}/reading-list@0.1.0", "y".repeat(52))
}
impl Fixture {
    /// Alice's published reading-list: its folder in `<root>/checkout` (the
    /// tagged tree plus a README and a workflow) and its package in the fake.
    fn published(&self, repository: Option<&str>) -> PathBuf {
        let source = self.root.join("source");
        for p in ["wit", "ui", "logic/src"] {
            fs::create_dir_all(source.join(p)).unwrap();
        }
        let repository = repository.map_or(String::new(), |r| format!("repository = \"{r}\"\n"));
        fs::write(
            source.join("mod.toml"),
            format!("name = \"Reading List\"\nsummary = \"page\"\n{repository}"),
        )
        .unwrap();
        fs::write(
            source.join("wit/world.wit"),
            "package mods:reading-list@0.1.0; world reading-list {}",
        )
        .unwrap();
        fs::write(source.join("ui/index.html"), "<title>Demo</title>").unwrap();
        fs::write(source.join("logic/src/lib.rs"), "// source data").unwrap();
        let package = watane_pkg::pack(&source, true).unwrap();
        let path = self.root.join("source.pkg");
        fs::write(&path, package.as_bytes()).unwrap();
        fs::write(
            self.home.join("fake.json"),
            json!({"fetch":{parent_tag():{"release":parent_tag(),"package":path}}}).to_string(),
        )
        .unwrap();
        fs::write(source.join("README.md"), "publisher readme").unwrap();
        fs::create_dir(source.join(".github")).unwrap();
        fs::write(source.join(".github/x.yml"), "publisher workflow").unwrap();
        source
    }
    fn derive_args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = ["watane-dev", "new", "ratings", "--from"]
            .map(OsString::from)
            .into();
        args.extend([
            parent_tag().into(),
            "--json".into(),
            "--dir".into(),
            self.root.join("ratings").into(),
            "--home".into(),
            self.home.clone().into(),
        ]);
        args
    }
    /// `new --from` in-process; a successful clone copies `checkout`.
    fn derive(&self, checkout: Option<PathBuf>, clone: Option<Reply>) -> Run {
        let mut clone = clone;
        self.dev(&self.derive_args(), move |c| {
            if !c.starts_with("git -c advice.detachedHead=false") {
                return None;
            }
            if let Some(answer) = clone.take() {
                return Some(answer);
            }
            let checkout = checkout.as_ref().unwrap();
            assert!(c.ends_with(" <root>/ratings"), "{c}");
            let clone = checkout.parent().unwrap().join("ratings");
            copy(checkout, &clone);
            fs::create_dir(clone.join(".git")).unwrap();
            Some(ok(""))
        })
    }
    fn assert_derived_files(&self, v: &Value) {
        assert_eq!(
            v["result"]["files"],
            json!([
                ".gitignore",
                "AGENTS.md",
                "CLAUDE.md",
                "logic/src/lib.rs",
                "mod.toml",
                "ui/index.html",
                "wit/world.wit"
            ])
        );
        let dir = self.root.join("ratings");
        assert_eq!(
            fs::read_to_string(dir.join("ui/index.html")).unwrap(),
            "<title>Demo</title>"
        );
        assert_eq!(
            fs::read_to_string(dir.join("logic/src/lib.rs")).unwrap(),
            "// source data"
        );
        assert_eq!(
            fs::read_to_string(dir.join("wit/world.wit")).unwrap(),
            "package mods:ratings@0.1.0; world reading-list {}"
        );
        assert!(!dir.join("README.md").exists());
        assert!(!dir.join(".github").exists());
        let meta: toml::Table = fs::read_to_string(dir.join("mod.toml"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            meta["origin"]["release"].as_str(),
            Some(parent_tag().as_str())
        );
        assert!(!meta.contains_key("repository"));
        for name in ["AGENTS.md", "CLAUDE.md", ".gitignore"] {
            let expected = fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("template")
                    .join(name),
            )
            .unwrap()
            .replace("{{slug}}", "ratings")
            .replace("{{crate}}", "ratings")
            .replace("{{title}}", "Ratings")
            .replace(
                "{{crates}}",
                "- serde 1.0.229\n- serde_json 1.0.154\n- wit-bindgen 0.63.0",
            );
            assert_eq!(fs::read_to_string(dir.join(name)).unwrap(), expected);
        }
    }
}
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
fn clone_command() -> String {
    format!(
        "git -c advice.detachedHead=false -c core.symlinks=false clone -q --branch {} -- {UPSTREAM} <root>/ratings",
        parent_tag()
    )
}

#[test]
fn ac6_new_from_clones_the_tag_and_keeps_only_package_files() {
    let f = Fixture::new();
    let checkout = f.published(Some(UPSTREAM));
    let run = f.derive(Some(checkout), None);
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(
        run.log,
        [
            clone_command(),
            "git -C <root>/ratings checkout -q -B main".into(),
            "git -C <root>/ratings remote rename origin upstream".into(),
        ]
    );
    assert_eq!(
        run.value["result"]["fork"],
        json!({"repository":UPSTREAM,"tag":parent_tag()})
    );
    assert!(f.root.join("ratings/.git").is_dir());
    f.assert_derived_files(&run.value);
}

#[test]
fn ac7_tampered_tag_falls_back_to_the_package() {
    let f = Fixture::new();
    let checkout = f.published(Some(UPSTREAM));
    fs::write(checkout.join("ui/index.html"), "tampered").unwrap();
    let run = f.derive(Some(checkout), None);
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(run.log, [clone_command()]);
    assert_eq!(run.value["warnings"][0]["code"], "new.fork_differs");
    assert_eq!(
        run.value["warnings"][0]["message"],
        format!(
            "{UPSTREAM} at {} differs from the verified package (ui/index.html: different); the mod was made from the verified package",
            parent_tag()
        )
    );
    assert_eq!(
        run.value["warnings"][0]["fix"],
        format!(
            "Tell the person: the repository of reading-list does not hold the published source of {}",
            parent_tag()
        )
    );
    assert!(run.value["result"]["fork"].is_null());
    assert!(!f.root.join("ratings/.git").exists());
    f.assert_derived_files(&run.value);
}

#[test]
fn ac7_unreachable_repository_falls_back_to_the_package() {
    for (answer, reason) in [
        (
            reply(128, "", "Cloning...\nfatal: repository not found\n"),
            "fatal: repository not found",
        ),
        (missing(), "git not found"),
    ] {
        let f = Fixture::new();
        f.published(Some(UPSTREAM));
        let run = f.derive(None, Some(answer));
        assert_eq!(run.code, 0, "{}", run.value);
        assert_eq!(run.log, [clone_command()]);
        assert_eq!(run.value["warnings"][0]["code"], "new.fork_unavailable");
        assert_eq!(
            run.value["warnings"][0]["message"],
            format!(
                "could not clone {UPSTREAM} at {} ({reason}); the mod was made from the verified package, with no git history",
                parent_tag()
            )
        );
        assert_eq!(
            run.value["warnings"][0]["fix"],
            format!(
                "To keep the publisher's history, check that git can reach {UPSTREAM}, delete {}, then run new --from again",
                f.root.join("ratings").display()
            )
        );
        assert!(run.value["result"]["fork"].is_null());
        assert!(!f.root.join("ratings/.git").exists());
        f.assert_derived_files(&run.value);
    }
}

#[test]
fn ac3_package_without_repository_has_no_fork_warning() {
    let f = Fixture::new();
    f.published(None);
    let run = f.derive(None, None);
    assert_eq!(run.code, 0, "{}", run.value);
    assert!(run.log.is_empty(), "{:?}", run.log);
    assert_eq!(run.value["warnings"], json!([]));
    assert!(run.value["result"]["fork"].is_null());
    f.assert_derived_files(&run.value);
}

/// Carol's first push of a mod derived from Alice's `reading-list`: the
/// `demo` folder with `[origin]`, a clone's `.git` and `upstream` remote.
struct Carol {
    /// `gh api repos/<owner>/demo` answers before and after the fork POST.
    before: Reply,
    after: Reply,
    fork: Reply,
    create: Reply,
    upstream: bool,
}
impl Default for Carol {
    fn default() -> Self {
        Self {
            before: not_found(),
            after: ok("alice/reading-list\n"),
            fork: ok("carol/demo\n"),
            create: ok(""),
            upstream: true,
        }
    }
}
impl Fixture {
    fn carol(&self, args: &[&str], carol: Carol) -> Run {
        fs::create_dir_all(self.dir.join(".git")).unwrap();
        fs::write(
            self.dir.join("mod.toml"),
            format!(
                "name = \"Demo\"\nsummary = \"a page\"\n[origin]\nrelease = \"{}\"\n",
                parent_tag()
            ),
        )
        .unwrap();
        let Carol {
            before,
            after,
            fork,
            create,
            upstream,
        } = carol;
        let mut lookup = Some(before);
        let mut later = Some(after);
        let (mut fork, mut create) = (Some(fork), Some(create));
        let mut branch = "main";
        self.push(args, move |c| {
            if c == "gh api user --jq .login" {
                Some(ok("carol\n"))
            } else if c.ends_with(" config --get remote.upstream.url") && upstream {
                Some(ok(&format!("{UPSTREAM}.git\n")))
            } else if c.starts_with("gh api repos/") && c.contains("/demo --jq ") {
                Some(
                    lookup
                        .take()
                        .or_else(|| later.take())
                        .unwrap_or_else(|| ok("alice/reading-list\n")),
                )
            } else if c.contains("/forks ") {
                fork.take()
            } else if c.starts_with("gh repo create ") && c.contains("/demo ") {
                create.take()
            } else if c == "git -C <mod> branch -m watane" {
                branch = "watane";
                None
            } else if c == "git -C <mod> branch --show-current" {
                Some(ok(&format!("{branch}\n")))
            } else {
                None
            }
        })
    }
}
fn fork_post(organization: Option<&str>) -> String {
    let organization = organization.map_or(String::new(), |o| format!(" -f organization={o}"));
    format!(
        "gh api -X POST repos/alice/reading-list/forks -f name=demo{organization} -F default_branch_only=true --jq .full_name"
    )
}

#[test]
fn ac8_derived_mod_forks_on_first_push() {
    let f = Fixture::new();
    let run = f.carol(&[], Carol::default());
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(run.value["result"]["push"]["mod"]["forked"], true);
    assert_eq!(run.value["result"]["push"]["mod"]["created"], true);
    assert_eq!(run.value["warnings"], json!([]));
    assert_eq!(
        run.log[..12],
        v(&[
            "git -C <mod> --version",
            "git -C <mod> var GIT_COMMITTER_IDENT",
            "gh api user --jq .login",
            "git -C <mod> config --get remote.origin.url",
            "git -C <mod> config --get remote.upstream.url",
            "gh api repos/carol/demo --jq .parent.full_name",
            &fork_post(None),
            "gh api repos/carol/demo --jq .parent.full_name",
            "git -C <mod> branch --show-current",
            "git -C <mod> branch -m watane",
            "git -C <mod> config --get remote.origin.url",
            "git -C <mod> remote add origin -- https://github.com/carol/demo",
        ])
    );
    assert_eq!(
        run.log[run.log.len() - 2..],
        v(&[
            &format!(
                "git -C <mod> push -q origin HEAD refs/tags/{}",
                release("demo")
            ),
            "gh repo edit carol/demo --default-branch watane",
        ])
    );
    assert!(
        fs::read_to_string(f.dir.join("mod.toml"))
            .unwrap()
            .contains("repository = \"https://github.com/carol/demo\"")
    );
}

#[test]
fn ac8_rerun_after_decline_reuses_fork_and_sets_default_branch() {
    let f = Fixture::new();
    fs::write(f.home.join("fake.json"), r#"{"answer":"decline"}"#).unwrap();
    let run = f.carol(&[], Carol::default());
    assert_eq!(run.code, 4, "{}", run.value);
    assert!(run.log.contains(&fork_post(None)));
    assert!(!run.log.iter().any(|c| c.contains(" push ")));
    let manifest = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
    assert!(manifest.contains("repository = \"https://github.com/carol/demo\""));

    // The rerun reads the repository from mod.toml and the folder's origin.
    fs::write(f.home.join("fake.json"), "{}").unwrap();
    let run = f.push(&[], |c| {
        if c == "gh api user --jq .login" {
            Some(ok("carol\n"))
        } else if c.ends_with(" config --get remote.origin.url") {
            Some(ok("https://github.com/carol/demo\n"))
        } else if c == "git -C <mod> branch --show-current" {
            Some(ok("watane\n"))
        } else {
            None
        }
    });
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(run.value["result"]["push"]["mod"]["created"], false);
    assert_eq!(run.value["result"]["push"]["mod"]["forked"], false);
    assert!(!run.log.iter().any(|c| c.contains("/forks ")));
    assert_eq!(
        run.log.last().unwrap(),
        "gh repo edit carol/demo --default-branch watane"
    );
}

#[test]
fn ac8_late_or_unfinished_fork_is_used() {
    // An earlier run's fork: it exists and its parent is the upstream.
    let f = Fixture::new();
    let run = f.carol(
        &[],
        Carol {
            before: ok("alice/reading-list\n"),
            ..Carol::default()
        },
    );
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(run.value["result"]["push"]["mod"]["forked"], true);
    assert_eq!(run.value["result"]["push"]["mod"]["created"], false);
    assert!(
        !run.log
            .iter()
            .any(|c| c.contains("/forks ") || c.contains(" create carol/demo "))
    );

    // The fork POST failed, then the create lost the race to a late fork.
    let f = Fixture::new();
    let run = f.carol(
        &[],
        Carol {
            fork: reply(1, "", "HTTP 502\n"),
            create: reply(1, "", "GraphQL: Name already exists on this account\n"),
            ..Carol::default()
        },
    );
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(run.value["result"]["push"]["mod"]["forked"], true);
    assert_eq!(run.value["warnings"], json!([]));
    assert!(run.log.contains(&"git -C <mod> branch -m watane".into()));
}

#[test]
fn ac8_existing_fork_falls_back_to_a_new_repository() {
    let f = Fixture::new();
    let run = f.carol(
        &[],
        Carol {
            fork: ok("carol/other\n"),
            ..Carol::default()
        },
    );
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(run.value["warnings"][0]["code"], "publish.fork_unavailable");
    assert_eq!(
        run.value["warnings"][0]["message"],
        "could not fork alice/reading-list as carol/demo (the account already has a fork of it); https://github.com/carol/demo was created without GitHub's fork link"
    );
    assert_eq!(run.value["result"]["push"]["mod"]["forked"], false);
    assert_eq!(run.value["result"]["push"]["mod"]["created"], true);
    assert_eq!(
        run.log[6..8],
        v(&[&fork_post(None), "gh repo create carol/demo --public"])
    );
    assert!(
        !run.log
            .iter()
            .any(|c| c.contains(" branch -m ") || c.contains(" repo edit "))
    );
}

#[test]
fn ac8_derived_without_upstream_warns() {
    let f = Fixture::new();
    let run = f.carol(
        &[],
        Carol {
            upstream: false,
            ..Carol::default()
        },
    );
    assert_eq!(run.code, 0, "{}", run.value);
    assert_eq!(run.value["warnings"][0]["code"], "publish.fork_unavailable");
    assert_eq!(
        run.value["warnings"][0]["message"],
        "could not fork no upstream repository as carol/demo (no upstream repository); https://github.com/carol/demo was created without GitHub's fork link"
    );
    assert_eq!(run.value["result"]["push"]["mod"]["forked"], false);
    assert!(!run.log.iter().any(|c| c.contains("/forks ")));
}

#[test]
fn ac3_owner_forks_and_existing_fork_fallback() {
    for existing in [false, true] {
        let f = Fixture::new();
        let run = f.carol(
            &["--owner", "acme"],
            Carol {
                fork: ok(if existing {
                    "acme/other\n"
                } else {
                    "acme/demo\n"
                }),
                ..Carol::default()
            },
        );
        assert_eq!(run.code, 0, "{}", run.value);
        assert!(run.log.contains(&fork_post(Some("acme"))), "{:?}", run.log);
        assert_eq!(run.value["result"]["push"]["mod"]["forked"], !existing);
        assert_eq!(
            run.value["result"]["push"]["mod"]["repository"],
            "https://github.com/acme/demo"
        );
        assert_eq!(
            run.log.last().unwrap() == "gh repo edit acme/demo --default-branch watane",
            !existing
        );
        if existing {
            assert!(
                run.value["warnings"][0]["message"]
                    .as_str()
                    .unwrap()
                    .contains("the account already has a fork")
            );
        }
        assert!(
            run.log.iter().all(|c| !c.contains("carol/")),
            "{:?}",
            run.log
        );
    }
}

#[test]
fn ac3_owner_matching_login_case_insensitively_omits_organization() {
    let f = Fixture::new();
    let run = f.carol(&["--owner", "CAROL"], Carol::default());
    assert_eq!(run.code, 0, "{}", run.value);
    assert!(run.log.contains(&fork_post(None)), "{:?}", run.log);
    assert_eq!(run.value["result"]["push"]["mod"]["forked"], true);
}

// ---- End to end: the real program, git and fake-gh ----

#[cfg(unix)] // End-to-end git fixtures use POSIX symlinks and shell quoting.
mod real {
    use super::*;

    struct Real {
        f: Fixture,
        git: PathBuf,
    }
    impl Real {
        fn new() -> Self {
            let f = Fixture::new();
            let root = &f.root;
            for p in ["remotes", "bin"] {
                fs::create_dir_all(root.join(p)).unwrap();
            }
            let git = std::env::split_paths(&std::env::var_os("PATH").unwrap())
                .map(|dir| dir.join("git"))
                .find(|git| git.is_file())
                .unwrap();
            std::os::unix::fs::symlink(&git, root.join("bin/git")).unwrap();
            let fake = Path::new(env!("CARGO_BIN_EXE_fake-gh"));
            std::os::unix::fs::symlink(fake, root.join("bin/gh")).unwrap();
            fs::write(root.join("gitconfig"), format!("[user]\n name = Alice\n email = alice@example.test\n[url \"file://{}/remotes/\"]\n insteadOf = https://github.com/\n", root.display())).unwrap();
            Self { f, git }
        }
        fn env(&self, cmd: &mut Command) {
            let root = &self.f.root;
            cmd.env("PATH", root.join("bin"))
                .env("GIT_CONFIG_GLOBAL", root.join("gitconfig"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("FAKE_GH_ROOT", root.join("remotes"))
                .env("FAKE_GH_USER", "alice")
                .env_remove("GIT_AUTHOR_NAME")
                .env_remove("GIT_AUTHOR_EMAIL")
                .env_remove("GIT_COMMITTER_NAME")
                .env_remove("GIT_COMMITTER_EMAIL");
        }
        fn run(&self, args: &[OsString], user: &str) -> (i32, Value) {
            let mut c = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
            self.env(&mut c);
            let o = c
                .args(&args[1..])
                .env("FAKE_GH_USER", user)
                .env("WATANE_DEV_CACHE", self.f.home.join("cache"))
                .output()
                .unwrap();
            (
                o.status.code().unwrap(),
                serde_json::from_slice(&o.stdout).unwrap_or_else(|_| panic!("{o:?}")),
            )
        }
        fn git(&self, dir: &Path, args: &[&str]) -> String {
            let mut c = Command::new(&self.git);
            self.env(&mut c);
            let o = c.arg("-C").arg(dir).args(args).output().unwrap();
            assert!(o.status.success(), "{o:?}");
            String::from_utf8(o.stdout).unwrap().trim().into()
        }
        fn remote(&self, path: &str) -> PathBuf {
            self.f.root.join("remotes").join(path)
        }
    }
    fn shell_quote(path: &Path) -> String {
        format!("'{}'", path.display().to_string().replace('\'', "'\"'\"'"))
    }

    #[test]
    fn ac1_e2e_first_push_creates_both_repositories_and_names_the_mod_repository() {
        let r = Real::new();
        let f = &r.f;
        let (code, v) = r.run(&f.publish_args(), "alice");
        assert_eq!(code, 0, "{v}");
        assert_eq!(
            v["result"]["push"],
            json!({"site":{"repository":"https://github.com/alice/watane-channel","raw":"https://raw.githubusercontent.com/alice/watane-channel/main/","pages":"https://alice.github.io/watane-channel/","created":true,"pushed":true},"mod":{"repository":"https://github.com/alice/demo","created":true,"pushed":true,"forked":false,"tag":release("demo")}})
        );
        assert_eq!(
            v["next"],
            format!(
                "Ask the person to set the site URL in the watane app to https://raw.githubusercontent.com/alice/watane-channel/main/ (or to https://alice.github.io/watane-channel/ if GitHub Pages serves this repository), then look up the published release link with: watane-dev publish --json --dir {} --home {} --push",
                shell_quote(&f.dir),
                shell_quote(&f.home)
            )
        );
        assert!(v["result"]["link"].is_null());
        let manifest = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
        assert!(manifest.contains("repository = \"https://github.com/alice/demo\""));
        let package =
            watane_pkg::Package::from_bytes(fs::read(f.home.join("requests/1-publish")).unwrap())
                .unwrap();
        assert_eq!(package.file("mod.toml").unwrap(), manifest.as_bytes());
        assert_eq!(
            r.git(
                &r.remote("alice/demo"),
                &["ls-tree", "-r", "--name-only", "main"]
            ),
            "mod.toml\nui/index.html\nwit/deps/watane-host/host.wit\nwit/world.wit"
        );
        assert_eq!(r.git(&r.remote("alice/demo"), &["tag"]), release("demo"));
        assert_eq!(
            r.git(
                &r.remote("alice/watane-channel"),
                &["ls-tree", "-r", "--name-only", "main"]
            ),
            format!(
                "head\npackages/{}",
                v["result"]["package"].as_str().unwrap()
            )
        );
    }

    #[test]
    fn ac2_e2e_second_version_pushes_without_creating() {
        let r = Real::new();
        let f = &r.f;
        assert_eq!(r.run(&f.publish_args(), "alice").0, 0);
        let old_mod = r.git(&r.remote("alice/demo"), &["rev-parse", "main"]);
        let old_site = r.git(&r.remote("alice/watane-channel"), &["rev-parse", "main"]);
        fs::write(
            f.dir.join("wit/world.wit"),
            "package mods:demo@0.2.0; world demo {}",
        )
        .unwrap();
        let (code, v) = r.run(&f.publish_args(), "alice");
        assert_eq!(code, 0, "{v}");
        for (name, old) in [("alice/demo", old_mod), ("alice/watane-channel", old_site)] {
            assert_eq!(r.git(&r.remote(name), &["rev-parse", "main^"]), old);
        }
        assert_eq!(v["result"]["push"]["site"]["created"], false);
        assert_eq!(v["result"]["push"]["mod"]["created"], false);
        // The gitconfig's insteadOf is active: raw proves the stored URL is read.
        assert_eq!(
            v["result"]["push"]["site"]["raw"],
            "https://raw.githubusercontent.com/alice/watane-channel/main/"
        );
        assert_eq!(
            r.git(&r.remote("alice/demo"), &["tag"]),
            format!("{0}/demo@0.1.0\n{0}/demo@0.2.0", "y".repeat(52))
        );
    }

    #[test]
    fn ac8_e2e_new_from_clones_then_first_push_forks() {
        let r = Real::new();
        let f = &r.f;
        // Alice's release: a tagged commit with a README in her repository.
        let checkout = f.published(Some(UPSTREAM));
        r.git(&checkout, &["init", "-q", "-b", "main"]);
        r.git(&checkout, &["add", "."]);
        r.git(&checkout, &["commit", "-qm", "parent"]);
        r.git(&checkout, &["tag", &parent_tag()]);
        fs::create_dir_all(r.remote("alice/reading-list")).unwrap();
        r.git(
            &r.remote("alice/reading-list"),
            &["init", "-q", "--bare", "-b", "main"],
        );
        r.git(&checkout, &["push", "-q", UPSTREAM, "main", "--tags"]);
        let commit = r.git(&checkout, &["rev-parse", "HEAD"]);

        let (code, v) = r.run(&f.derive_args(), "carol");
        assert_eq!(code, 0, "{v}");
        assert_eq!(
            v["result"]["fork"],
            json!({"repository":UPSTREAM,"tag":parent_tag()})
        );
        let dir = f.root.join("ratings");
        assert_eq!(r.git(&dir, &["rev-parse", "HEAD"]), commit);
        assert_eq!(r.git(&dir, &["remote"]), "upstream");
        f.assert_derived_files(&v);

        // Carol's first push forks it; `upstream` ends in `.git`.
        r.git(
            &dir,
            &["remote", "set-url", "upstream", &format!("{UPSTREAM}.git")],
        );
        fs::remove_dir_all(dir.join("logic")).unwrap();
        let mut args = f.publish_args();
        args[5] = dir.clone().into();
        let (code, v) = r.run(&args, "carol");
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["result"]["push"]["mod"]["forked"], true);
        assert_eq!(v["result"]["push"]["mod"]["created"], true);
        assert_eq!(v["warnings"], json!([]));
        let remote = r.remote("carol/ratings");
        assert_eq!(
            fs::read_to_string(remote.join("fake-gh-parent"))
                .unwrap()
                .trim(),
            "alice/reading-list"
        );
        assert_eq!(
            r.git(&remote, &["symbolic-ref", "HEAD"]),
            "refs/heads/watane"
        );
        assert_eq!(r.git(&remote, &["merge-base", "watane", &commit]), commit);
        assert_eq!(
            r.git(&remote, &["rev-parse", &release("ratings")]),
            r.git(&remote, &["rev-parse", "watane"])
        );
    }

    /// fake-gh's fork arms (15.4), which app's journeys also rely on.
    #[test]
    fn ac4_fake_gh_organization_and_login_arms() {
        let r = Real::new();
        let upstream = r.remote("alice/reading-list");
        fs::create_dir_all(&upstream).unwrap();
        r.git(&upstream, &["init", "-q", "--bare", "-b", "main"]);
        let seed = r.f.root.join("seed");
        fs::create_dir(&seed).unwrap();
        r.git(&seed, &["init", "-q", "-b", "main"]);
        r.git(&seed, &["commit", "-q", "--allow-empty", "-m", "seed"]);
        r.git(&seed, &["push", "-q", upstream.to_str().unwrap(), "main"]);
        let fork = |name: &str, organization: Option<&str>| {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_fake-gh"));
            r.env(&mut cmd);
            cmd.args([
                "api",
                "-X",
                "POST",
                "repos/alice/reading-list/forks",
                "-f",
                &format!("name={name}"),
            ]);
            if let Some(org) = organization {
                cmd.args(["-f", &format!("organization={org}")]);
            }
            cmd.args(["-F", "default_branch_only=true", "--jq", ".full_name"])
                .output()
                .unwrap()
        };
        let org = fork("ratings", Some("acme"));
        assert!(org.status.success(), "{org:?}");
        assert_eq!(String::from_utf8(org.stdout).unwrap(), "acme/ratings\n");
        assert_eq!(
            fs::read_to_string(r.remote("acme/ratings/fake-gh-parent")).unwrap(),
            "alice/reading-list"
        );
        let existing = fork("another", Some("acme"));
        assert!(existing.status.success());
        assert_eq!(
            String::from_utf8(existing.stdout).unwrap(),
            "acme/ratings\n"
        );
        assert!(!r.remote("acme/another").exists());
        for login in ["alice", "ALICE"] {
            let refused = fork("refused", Some(login));
            assert_eq!(refused.status.code(), Some(1));
            assert!(
                String::from_utf8(refused.stderr)
                    .unwrap()
                    .contains("organization=")
            );
        }
        let personal = fork("personal", None);
        assert!(personal.status.success(), "{personal:?}");
        assert_eq!(
            String::from_utf8(personal.stdout).unwrap(),
            "alice/personal\n"
        );
        assert!(r.remote("alice/personal/fake-gh-parent").exists());
    }
}
