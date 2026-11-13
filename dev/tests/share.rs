#![cfg(feature = "fake-host")]
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};
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
    git: PathBuf,
    _host: Host,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::Builder::new()
            .prefix("Alice's share ")
            .tempdir()
            .unwrap();
        let root = temp.path().canonicalize().unwrap();
        let dir = root.join("demo");
        let home = root.join("home");
        for p in [
            dir.join("wit"),
            dir.join("ui"),
            home.clone(),
            root.join("remotes"),
            root.join("bin"),
        ] {
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
        let git = PathBuf::from(
            String::from_utf8(Command::new("which").arg("git").output().unwrap().stdout)
                .unwrap()
                .trim(),
        );
        std::os::unix::fs::symlink(&git, root.join("bin/git")).unwrap();
        // The fake is built by Cargo before integration tests.
        let fake = Path::new(env!("CARGO_BIN_EXE_fake-gh"));
        std::os::unix::fs::symlink(fake, root.join("bin/gh")).unwrap();
        fs::write(root.join("gitconfig"), format!("[user]\n name = Alice\n email = alice@example.test\n[url \"file://{}/remotes/\"]\n insteadOf = https://github.com/\n", root.display())).unwrap();
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
            git,
            _host: Host(child),
        }
    }
    fn env(&self, cmd: &mut Command) {
        cmd.env("PATH", self.root.join("bin"))
            .env("GIT_CONFIG_GLOBAL", self.root.join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("FAKE_GH_ROOT", self.root.join("remotes"))
            .env("FAKE_GH_USER", "alice")
            .env_remove("GIT_AUTHOR_NAME")
            .env_remove("GIT_AUTHOR_EMAIL")
            .env_remove("GIT_COMMITTER_NAME")
            .env_remove("GIT_COMMITTER_EMAIL");
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        self.env(&mut c);
        c.args(["publish", "--push", "--json", "--dir"])
            .arg(&self.dir)
            .arg("--home")
            .arg(&self.home)
            .env("WATANE_DEV_CACHE", self.home.join("cache"));
        c
    }
    fn run(&self, args: &[&str]) -> (i32, Value) {
        let o = self.command().args(args).output().unwrap();
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
    fn remote(&self, name: &str) -> PathBuf {
        self.root.join("remotes/alice").join(name)
    }
    fn unchanged(&self, old: &str) {
        assert_eq!(fs::read_to_string(self.dir.join("mod.toml")).unwrap(), old);
        assert!(!self.home.join("requests").exists());
    }
}
#[test]
fn ac1_first_push_creates_both_repositories_and_names_the_mod_repository() {
    let f = Fixture::new();
    let (code, v) = f.run(&[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["result"]["push"],
        json!({"site":{"repository":"https://github.com/alice/watane-channel","raw":"https://raw.githubusercontent.com/alice/watane-channel/main/","created":true,"pushed":true},"mod":{"repository":"https://github.com/alice/demo","created":true,"pushed":true,"forked":false,"tag":format!("{}/demo@0.1.0","y".repeat(52))}})
    );
    assert_eq!(
        v["next"],
        "Ask the person to set the site URL in the watane app to https://raw.githubusercontent.com/alice/watane-channel/main/, then run publish again to get the link"
    );
    assert!(v["result"]["link"].is_null());
    let manifest = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
    assert!(manifest.contains("repository = \"https://github.com/alice/demo\""));
    let package =
        watane_pkg::Package::from_bytes(fs::read(f.home.join("requests/1-publish")).unwrap())
            .unwrap();
    assert_eq!(package.file("mod.toml").unwrap(), manifest.as_bytes());
    assert_eq!(
        f.git(&f.remote("demo"), &["ls-tree", "-r", "--name-only", "main"]),
        "mod.toml\nui/index.html\nwit/deps/watane-host/host.wit\nwit/world.wit"
    );
    assert_eq!(
        f.git(&f.remote("demo"), &["tag"]),
        format!("{}/demo@0.1.0", "y".repeat(52))
    );
    assert_eq!(
        f.git(
            &f.remote("watane-channel"),
            &["ls-tree", "-r", "--name-only", "main"]
        ),
        format!(
            "head\npackages/{}",
            v["result"]["package"].as_str().unwrap()
        )
    );
}
#[test]
fn ac2_second_version_pushes_without_creating() {
    let f = Fixture::new();
    assert_eq!(f.run(&[]).0, 0);
    let old_mod = f.git(&f.remote("demo"), &["rev-parse", "main"]);
    let old_site = f.git(&f.remote("watane-channel"), &["rev-parse", "main"]);
    fs::write(
        f.dir.join("wit/world.wit"),
        "package mods:demo@0.2.0; world demo {}",
    )
    .unwrap();
    let (code, v) = f.run(&[]);
    assert_eq!(code, 0, "{v}");
    for (name, old) in [("demo", old_mod), ("watane-channel", old_site)] {
        assert_eq!(f.git(&f.remote(name), &["rev-parse", "main^"]), old);
    }
    assert_eq!(v["result"]["push"]["site"]["created"], false);
    assert_eq!(v["result"]["push"]["mod"]["created"], false);
    assert_eq!(
        v["result"]["push"]["site"]["raw"],
        "https://raw.githubusercontent.com/alice/watane-channel/main/"
    );
    assert_eq!(
        f.git(&f.remote("demo"), &["tag"]),
        format!("{0}/demo@0.1.0\n{0}/demo@0.2.0", "y".repeat(52))
    );
}
#[test]
fn ac3_missing_git_or_gh_stops_before_anything() {
    for (program, code) in [("git", "env.git_missing"), ("gh", "env.gh_missing")] {
        let f = Fixture::new();
        let old = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
        fs::remove_file(f.root.join("bin").join(program)).unwrap();
        let (exit, v) = f.run(&[]);
        assert_eq!(exit, 3, "{v}");
        assert_eq!(v["errors"][0]["code"], code);
        f.unchanged(&old);
        assert_eq!(fs::read_dir(f.root.join("remotes")).unwrap().count(), 0);
    }
}
#[test]
fn ac3_gh_not_logged_in_or_no_git_identity() {
    for auth in [true, false] {
        let f = Fixture::new();
        let old = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
        let mut c = f.command();
        if auth {
            c.env_remove("FAKE_GH_USER");
        } else {
            fs::write(f.root.join("gitconfig"), "[user]\n useConfigOnly = true\n").unwrap();
        }
        let o = c.output().unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(o.status.code(), Some(3), "{v}");
        assert_eq!(
            v["errors"][0]["code"],
            if auth {
                "env.gh_auth"
            } else {
                "env.git_identity"
            }
        );
        if auth {
            assert!(
                v["errors"][0]["fix"]
                    .as_str()
                    .unwrap()
                    .contains("gh auth login")
            );
        }
        f.unchanged(&old);
        assert_eq!(fs::read_dir(f.root.join("remotes")).unwrap().count(), 0);
    }
}
#[test]
fn ac4_refused_push_reports_release_and_finishing_commands() {
    let f = Fixture::new();
    fs::create_dir_all(f.remote("demo")).unwrap();
    f.git(&f.remote("demo"), &["init", "-q", "--bare", "-b", "main"]);
    let hook = f.remote("demo").join("hooks/pre-receive");
    fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let (code, v) = f.run(&[]);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["errors"][0]["code"], "publish.push_failed");
    assert_eq!(
        v["result"]["release"],
        format!("{}/demo@0.1.0", "y".repeat(52))
    );
    assert_eq!(v["result"]["push"]["site"]["pushed"], true);
    assert_eq!(v["result"]["push"]["mod"]["pushed"], false);
    assert_eq!(v["next"], v["errors"][0]["fix"]);
    assert!(
        v["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("push -q origin HEAD")
    );
    let fix = v["next"].as_str().unwrap();
    assert!(fix.contains("push -q origin HEAD"), "{fix}");
    assert!(fix.contains("'\"'\"'"), "{fix}");
    assert!(fix.starts_with("git -C '"), "{fix}");
    assert!(
        fix.ends_with(&format!(
            " push -q origin HEAD refs/tags/{}/demo@0.1.0",
            "y".repeat(52)
        )),
        "{fix}"
    );
    assert!(!fix.contains(" && "), "{fix}");
    assert_eq!(f.git(&f.remote("demo"), &["tag"]), "");
    fs::remove_file(hook).unwrap();
    let mut c = Command::new("/bin/sh");
    f.env(&mut c);
    assert!(c.args(["-c", fix]).status().unwrap().success());
    assert_eq!(
        f.git(&f.remote("demo"), &["tag"]),
        format!("{}/demo@0.1.0", "y".repeat(52))
    );
}
#[test]
fn ac5_no_source_pushes_only_the_site() {
    let f = Fixture::new();
    let old = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
    let (code, v) = f.run(&["--no-source"]);
    assert_eq!(code, 0, "{v}");
    assert!(v["result"]["push"]["mod"].is_null());
    assert!(!f.remote("demo").exists());
    assert_eq!(v["result"]["push"]["site"]["created"], true);
    assert_eq!(v["result"]["push"]["site"]["pushed"], true);
    assert_eq!(fs::read_to_string(f.dir.join("mod.toml")).unwrap(), old);
}
#[test]
fn ac5_origin_differs_and_foreign_repository_stop_before_anything() {
    for case in ["missing", "different", "nonempty"] {
        let f = Fixture::new();
        if case == "nonempty" {
            fs::create_dir_all(f.remote("demo")).unwrap();
            f.git(&f.remote("demo"), &["init", "-q", "--bare", "-b", "main"]);
            let seed = f.root.join("seed");
            fs::create_dir(&seed).unwrap();
            f.git(&seed, &["init", "-q", "-b", "main"]);
            fs::write(seed.join("foreign"), "foreign").unwrap();
            f.git(&seed, &["add", "foreign"]);
            f.git(&seed, &["commit", "-qm", "foreign"]);
            f.git(&seed, &["push", f.remote("demo").to_str().unwrap(), "HEAD"]);
        } else {
            f.git(&f.dir, &["init", "-q", "-b", "main"]);
            f.git(
                &f.dir,
                &["remote", "add", "origin", "https://github.com/bob/other"],
            );
            if case == "different" {
                fs::write(f.dir.join("mod.toml"),"name = \"Demo\"\nsummary = \"a page\"\nrepository = \"https://github.com/alice/demo\"\n").unwrap();
            }
        }
        let old = fs::read_to_string(f.dir.join("mod.toml")).unwrap();
        let (code, v) = f.run(&[]);
        assert_eq!(code, 1, "{v}");
        assert_eq!(
            v["errors"][0]["code"],
            if case == "nonempty" {
                "publish.repository_exists"
            } else {
                "publish.origin_differs"
            }
        );
        f.unchanged(&old);
        assert!(!f.remote("watane-channel").exists());
        assert_eq!(f.remote("demo").exists(), case == "nonempty");
    }
}
#[test]
fn ac6_prebuilt_mod_keeps_logic_wasm_in_commit() {
    let f = Fixture::new();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ratings");
    fs::create_dir_all(f.dir.join("wit/deps/watane-host")).unwrap();
    fs::copy(
        source.join("wit/deps/watane-host/host.wit"),
        f.dir.join("wit/deps/watane-host/host.wit"),
    )
    .unwrap();
    for name in ["logic.wasm", "wit/world.wit", "mod.toml"] {
        fs::copy(source.join(name), f.dir.join(name)).unwrap();
    }
    fs::create_dir(f.dir.join("tests")).unwrap();
    fs::write(
        f.dir.join("tests/test.json"),
        r#"{"steps":[{"call":"api.get","returns":0}]}"#,
    )
    .unwrap();
    let (code, v) = f.run(&[]);
    assert_eq!(code, 0, "{v}");
    let repo = v["result"]["push"]["mod"]["repository"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap();
    assert!(
        f.git(&f.remote(repo), &["ls-tree", "-r", "--name-only", "main"])
            .lines()
            .any(|p| p == "logic.wasm")
    );
}

#[test]
fn ac3_missing_app_retry_keeps_push_and_package_options() {
    let f = Fixture::new();
    fs::remove_file(f.home.join("run.json")).unwrap();
    let (code, value) = f.run(&["--no-source", "--wait", "7"]);
    assert_eq!(code, 3, "{value}");
    let next = value["next"].as_str().unwrap();
    assert!(next.contains("--push"), "{next}");
    assert!(next.contains("--no-source"), "{next}");
    assert!(next.contains("--wait 7"), "{next}");
    assert!(!f.home.join("requests").exists());
}
