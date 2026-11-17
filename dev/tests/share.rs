#![cfg(feature = "fake-host")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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
fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\"'\"'"))
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
        format!(
            "Ask the person to set the site URL in the watane app to https://raw.githubusercontent.com/alice/watane-channel/main/, then run: watane-dev publish --json --dir {} --home {} --push",
            shell_quote(&f.dir.canonicalize().unwrap()),
            shell_quote(&f.home.canonicalize().unwrap())
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

const UPSTREAM: &str = "https://github.com/alice/reading-list";
fn parent_tag() -> String {
    format!("{}/reading-list@0.1.0", "y".repeat(52))
}
impl Fixture {
    fn released(&self) -> String {
        fs::write(
            self.dir.join("mod.toml"),
            format!("name = \"Reading List\"\nsummary = \"page\"\nrepository = \"{UPSTREAM}\"\n"),
        )
        .unwrap();
        fs::write(
            self.dir.join("wit/world.wit"),
            "package mods:reading-list@0.1.0; world reading-list {}",
        )
        .unwrap();
        fs::create_dir_all(self.dir.join("logic/src")).unwrap();
        fs::write(self.dir.join("logic/src/lib.rs"), "// source data").unwrap();
        let package = watane_pkg::pack(&self.dir, true).unwrap();
        let path = self.root.join("source.pkg");
        fs::write(&path, package.as_bytes()).unwrap();
        fs::write(
            self.home.join("fake.json"),
            json!({"fetch":{parent_tag():{"release":parent_tag(),"package":path}}}).to_string(),
        )
        .unwrap();
        self.git(&self.dir, &["init", "-q", "-b", "main"]);
        fs::write(self.dir.join("README.md"), "publisher readme").unwrap();
        fs::create_dir(self.dir.join(".github")).unwrap();
        fs::write(self.dir.join(".github/x.yml"), "publisher workflow").unwrap();
        self.git(&self.dir, &["add", "."]);
        self.git(&self.dir, &["commit", "-qm", "parent"]);
        self.git(&self.dir, &["tag", &parent_tag()]);
        fs::create_dir_all(self.remote("reading-list")).unwrap();
        self.git(
            &self.remote("reading-list"),
            &["init", "-q", "--bare", "-b", "main"],
        );
        self.git(&self.dir, &["push", UPSTREAM, "main", "--tags"]);
        self.git(&self.dir, &["rev-parse", "HEAD"])
    }
    fn derive(&self) -> (i32, Value) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        self.env(&mut c);
        let o = c
            .args(["new", "ratings", "--from", &parent_tag(), "--json", "--dir"])
            .arg(self.root.join("ratings"))
            .arg("--home")
            .arg(&self.home)
            .env("WATANE_DEV_CACHE", self.home.join("cache"))
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice(&o.stdout).unwrap_or_else(|_| panic!("{o:?}")),
        )
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
#[test]
fn ac6_new_from_clones_the_tag_and_keeps_only_package_files() {
    let f = Fixture::new();
    let commit = f.released();
    let (code, v) = f.derive();
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["result"]["fork"],
        json!({"repository":UPSTREAM,"tag":parent_tag()})
    );
    let dir = f.root.join("ratings");
    assert!(dir.join(".git").is_dir());
    assert_eq!(f.git(&dir, &["rev-parse", "HEAD"]), commit);
    assert_eq!(f.git(&dir, &["branch", "--show-current"]), "main");
    assert_eq!(f.git(&dir, &["remote"]), "upstream");
    assert_eq!(
        f.git(&dir, &["config", "--get", "remote.upstream.url"]),
        UPSTREAM
    );
    f.assert_derived_files(&v);
}

#[test]
fn ac7_tampered_tag_falls_back_to_the_package() {
    let f = Fixture::new();
    f.released();
    fs::write(f.dir.join("ui/index.html"), "tampered").unwrap();
    f.git(&f.dir, &["add", "ui/index.html"]);
    f.git(&f.dir, &["commit", "-qm", "tampered"]);
    f.git(&f.dir, &["tag", "-f", &parent_tag()]);
    f.git(
        &f.dir,
        &[
            "push",
            "-f",
            UPSTREAM,
            &format!("refs/tags/{}", parent_tag()),
        ],
    );
    let (code, v) = f.derive();
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["warnings"][0]["code"], "new.fork_differs");
    assert_eq!(
        v["warnings"][0]["message"],
        format!(
            "{UPSTREAM} at {} differs from the verified package (ui/index.html: different); the mod was made from the verified package",
            parent_tag()
        )
    );
    assert_eq!(
        v["warnings"][0]["fix"],
        format!(
            "Tell the person: the repository of reading-list does not hold the published source of {}",
            parent_tag()
        )
    );
    assert!(v["result"]["fork"].is_null());
    assert!(!f.root.join("ratings/.git").exists());
    f.assert_derived_files(&v);
}
#[test]
fn ac7_unreachable_repository_falls_back_to_the_package() {
    let f = Fixture::new();
    f.released();
    fs::remove_dir_all(f.remote("reading-list")).unwrap();
    let (code, v) = f.derive();
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["warnings"][0]["code"], "new.fork_unavailable");
    let message = v["warnings"][0]["message"].as_str().unwrap();
    assert!(
        message.starts_with(&format!("could not clone {UPSTREAM} at {} (", parent_tag())),
        "{message}"
    );
    assert!(message.ends_with("the mod was made from the verified package, with no git history"));
    assert_eq!(
        v["warnings"][0]["fix"],
        format!(
            "To keep the publisher's history, check that git can reach {UPSTREAM}, delete {}, then run new --from again",
            f.root.join("ratings").display()
        )
    );
    assert!(v["result"]["fork"].is_null());
    assert!(!f.root.join("ratings/.git").exists());
    f.assert_derived_files(&v);
}

impl Fixture {
    fn ratings(&mut self) -> String {
        let commit = self.released();
        let (code, v) = self.derive();
        assert_eq!(code, 0, "{v}");
        self.dir = self.root.join("ratings");
        self.git(
            &self.dir,
            &["remote", "set-url", "upstream", &format!("{UPSTREAM}.git")],
        );
        fs::remove_dir_all(self.dir.join("logic")).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ratings");
        fs::copy(fixture.join("logic.wasm"), self.dir.join("logic.wasm")).unwrap();
        fs::copy(
            fixture.join("wit/world.wit"),
            self.dir.join("wit/world.wit"),
        )
        .unwrap();
        fs::create_dir_all(self.dir.join("wit/deps/watane-host")).unwrap();
        fs::copy(
            fixture.join("wit/deps/watane-host/host.wit"),
            self.dir.join("wit/deps/watane-host/host.wit"),
        )
        .unwrap();
        fs::create_dir(self.dir.join("tests")).unwrap();
        fs::write(
            self.dir.join("tests/test.json"),
            r#"{"steps":[{"call":"api.get","returns":0}]}"#,
        )
        .unwrap();
        commit
    }
    fn carol(&self) -> (i32, Value) {
        let o = self
            .command()
            .env("FAKE_GH_USER", "carol")
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice(&o.stdout).unwrap_or_else(|_| panic!("{o:?}")),
        )
    }
    fn carol_remote(&self) -> PathBuf {
        self.root.join("remotes/carol/ratings")
    }
    fn assert_fork_push(&self, commit: &str, v: &Value) {
        let remote = self.carol_remote();
        assert_eq!(
            fs::read_to_string(remote.join("fake-gh-parent"))
                .unwrap()
                .trim(),
            "alice/reading-list"
        );
        assert_eq!(
            self.git(&remote, &["symbolic-ref", "HEAD"]),
            "refs/heads/watane"
        );
        assert_eq!(self.git(&remote, &["merge-base", "watane", commit]), commit);
        assert_eq!(
            self.git(&remote, &["rev-parse", "main"]),
            self.git(&self.remote("reading-list"), &["rev-parse", "main"])
        );
        assert_eq!(
            self.git(
                &remote,
                &["rev-parse", &format!("{}/ratings@0.1.0", "y".repeat(52))]
            ),
            self.git(&remote, &["rev-parse", "watane"])
        );
        assert_eq!(v["warnings"], json!([]));
        assert!(
            fs::read_to_string(self.dir.join("mod.toml"))
                .unwrap()
                .contains("https://github.com/carol/ratings")
        );
    }
}
#[test]
fn ac8_derived_mod_forks_on_first_push() {
    let mut f = Fixture::new();
    let commit = f.ratings();
    let (code, v) = f.carol();
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["result"]["push"]["mod"]["forked"], true);
    assert_eq!(v["result"]["push"]["mod"]["created"], true);
    f.assert_fork_push(&commit, &v);
}
#[test]
fn ac8_rerun_after_decline_reuses_fork_and_sets_default_branch() {
    let mut f = Fixture::new();
    let commit = f.ratings();
    fs::write(f.home.join("fake.json"), r#"{"answer":"decline"}"#).unwrap();
    let (code, v) = f.carol();
    assert_eq!(code, 4, "{v}");
    assert_eq!(
        f.git(&f.carol_remote(), &["symbolic-ref", "HEAD"]),
        "refs/heads/main"
    );
    assert_eq!(
        fs::read_to_string(f.carol_remote().join("fake-gh-parent"))
            .unwrap()
            .trim(),
        "alice/reading-list"
    );
    assert!(
        fs::read_to_string(f.dir.join("mod.toml"))
            .unwrap()
            .contains("https://github.com/carol/ratings")
    );
    fs::write(f.home.join("fake.json"), "{}").unwrap();
    let (code, v) = f.carol();
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["result"]["push"]["mod"]["created"], false);
    assert_eq!(v["result"]["push"]["mod"]["forked"], false);
    f.assert_fork_push(&commit, &v);
    let mut names = fs::read_dir(f.root.join("remotes/carol"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["ratings", "watane-channel"]);
}
#[test]
fn ac8_existing_fork_falls_back_to_a_new_repository() {
    let mut f = Fixture::new();
    let commit = f.ratings();
    let other = f.root.join("remotes/carol/other");
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    let mut cmd = Command::new(&f.git);
    f.env(&mut cmd);
    assert!(
        cmd.args(["clone", "-q", "--bare"])
            .arg(f.remote("reading-list"))
            .arg(&other)
            .status()
            .unwrap()
            .success()
    );
    fs::write(other.join("fake-gh-parent"), "alice/reading-list").unwrap();
    let before = f.git(&other, &["show-ref"]);
    let (code, v) = f.carol();
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["warnings"][0]["code"], "publish.fork_unavailable");
    assert_eq!(
        v["warnings"][0]["message"],
        "could not fork alice/reading-list as carol/ratings (the account already has a fork of it); https://github.com/carol/ratings was created without GitHub's fork link"
    );
    assert!(!f.carol_remote().join("fake-gh-parent").exists());
    assert_eq!(
        f.git(
            &f.carol_remote(),
            &["rev-parse", &format!("{}/ratings@0.1.0", "y".repeat(52))]
        ),
        f.git(&f.carol_remote(), &["rev-parse", "main"])
    );
    assert_eq!(f.git(&other, &["show-ref"]), before);
    assert_eq!(
        f.git(&f.carol_remote(), &["merge-base", "main", &commit]),
        commit
    );
    assert_eq!(
        f.git(&f.carol_remote(), &["symbolic-ref", "HEAD"]),
        "refs/heads/main"
    );
    assert_eq!(v["result"]["push"]["mod"]["forked"], false);
}
#[test]
fn ac8_derived_without_upstream_warns() {
    let mut f = Fixture::new();
    f.released();
    fs::remove_dir_all(f.remote("reading-list")).unwrap();
    let (code, derived) = f.derive();
    assert_eq!(code, 0, "{derived}");
    assert!(derived["result"]["fork"].is_null());
    f.dir = f.root.join("ratings");
    fs::remove_dir_all(f.dir.join("logic")).unwrap();
    let (code, v) = f.carol();
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["warnings"][0]["code"], "publish.fork_unavailable");
    assert_eq!(
        v["warnings"][0]["message"],
        "could not fork no upstream repository as carol/ratings (no upstream repository); https://github.com/carol/ratings was created without GitHub's fork link"
    );
    assert_eq!(v["result"]["push"]["mod"]["forked"], false);
}

#[test]
fn ac3_package_without_repository_has_no_fork_warning() {
    let f = Fixture::new();
    f.released();
    fs::write(
        f.dir.join("mod.toml"),
        "name = \"Reading List\"\nsummary = \"page\"\n",
    )
    .unwrap();
    let package = watane_pkg::pack(&f.dir, true).unwrap();
    fs::write(f.root.join("source.pkg"), package.as_bytes()).unwrap();
    let (code, v) = f.derive();
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["warnings"], json!([]));
    assert!(v["result"]["fork"].is_null());
    assert!(!f.root.join("ratings/.git").exists());
    f.assert_derived_files(&v);
}

#[test]
fn ac3_missing_git_falls_back_with_actionable_warning() {
    let f = Fixture::new();
    f.released();
    fs::remove_file(f.root.join("bin/git")).unwrap();
    let (code, v) = f.derive();
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["warnings"][0]["code"], "new.fork_unavailable");
    assert_eq!(
        v["warnings"][0]["message"],
        format!(
            "could not clone {UPSTREAM} at {} (git not found); the mod was made from the verified package, with no git history",
            parent_tag()
        )
    );
    assert!(v["result"]["fork"].is_null());
    assert!(!f.root.join("ratings/.git").exists());
    f.assert_derived_files(&v);
}

#[test]
fn ac2_owner_creates_and_pushes_both_repositories() {
    let f = Fixture::new();
    let (code, v) = f.run(&["--owner", "acme"]);
    assert_eq!(code, 0, "{v}");
    for (part, name) in [("mod", "demo"), ("site", "watane-channel")] {
        assert_eq!(
            v["result"]["push"][part]["repository"],
            format!("https://github.com/acme/{name}")
        );
        assert_eq!(v["result"]["push"][part]["created"], true);
        assert_eq!(v["result"]["push"][part]["pushed"], true);
        let remote = f.root.join("remotes/acme").join(name);
        assert!(!f.git(&remote, &["show-ref"]).is_empty());
    }
    assert!(!f.root.join("remotes/alice").exists());
    assert!(
        fs::read_to_string(f.dir.join("mod.toml"))
            .unwrap()
            .contains("repository = \"https://github.com/acme/demo\"")
    );
    assert!(
        v["next"]
            .as_str()
            .unwrap()
            .ends_with(" --push --owner acme")
    );
}

#[test]
fn ac4_fake_gh_organization_and_login_arms() {
    let f = Fixture::new();
    f.released();
    let fork = |name: &str, organization: Option<&str>| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_fake-gh"));
        f.env(&mut cmd);
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
        fs::read_to_string(f.root.join("remotes/acme/ratings/fake-gh-parent")).unwrap(),
        "alice/reading-list"
    );
    let existing = fork("another", Some("acme"));
    assert!(existing.status.success());
    assert_eq!(
        String::from_utf8(existing.stdout).unwrap(),
        "acme/ratings\n"
    );
    assert!(!f.root.join("remotes/acme/another").exists());
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
    assert!(f.remote("personal").join("fake-gh-parent").exists());
}

#[test]
fn ac3_owner_forks_and_existing_fork_fallback() {
    for existing in [false, true] {
        let mut f = Fixture::new();
        let commit = f.ratings();
        if existing {
            let other = f.root.join("remotes/acme/other");
            fs::create_dir_all(other.parent().unwrap()).unwrap();
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_fake-gh"));
            f.env(&mut cmd);
            assert!(
                cmd.args([
                    "api",
                    "-X",
                    "POST",
                    "repos/alice/reading-list/forks",
                    "-f",
                    "name=other",
                    "-f",
                    "organization=acme",
                    "-F",
                    "default_branch_only=true",
                    "--jq",
                    ".full_name"
                ])
                .status()
                .unwrap()
                .success()
            );
        }
        let out = f
            .command()
            .env("FAKE_GH_USER", "carol")
            .args(["--owner", "acme"])
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(out.status.success(), "{v}");
        assert_eq!(v["result"]["push"]["mod"]["forked"], !existing);
        assert_eq!(
            v["result"]["push"]["mod"]["repository"],
            "https://github.com/acme/ratings"
        );
        let remote = f.root.join("remotes/acme/ratings");
        let branch = if existing { "main" } else { "watane" };
        assert_eq!(
            f.git(&remote, &["symbolic-ref", "HEAD"]),
            format!("refs/heads/{branch}")
        );
        assert_eq!(f.git(&remote, &["merge-base", branch, &commit]), commit);
        assert!(
            fs::read_to_string(f.dir.join("mod.toml"))
                .unwrap()
                .contains("https://github.com/acme/ratings")
        );
        if existing {
            assert_eq!(v["warnings"][0]["code"], "publish.fork_unavailable");
            assert!(
                v["warnings"][0]["message"]
                    .as_str()
                    .unwrap()
                    .contains("the account already has a fork")
            );
            assert!(!remote.join("fake-gh-parent").exists());
        } else {
            assert_eq!(
                fs::read_to_string(remote.join("fake-gh-parent")).unwrap(),
                "alice/reading-list"
            );
            assert_eq!(v["warnings"], json!([]));
        }
        assert!(!f.root.join("remotes/carol").exists());
    }
}

#[test]
fn ac3_owner_matching_login_case_insensitively_omits_organization() {
    let mut f = Fixture::new();
    f.ratings();
    let out = f
        .command()
        .env("FAKE_GH_USER", "carol")
        .args(["--owner", "CAROL"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(out.status.success(), "{v}");
    assert_eq!(v["result"]["push"]["mod"]["forked"], true, "{v}");
}

#[test]
fn ac1_owner_preserved_in_recovery_commands() {
    let f = Fixture::new();
    // Missing home: the app-start instruction carries the selected account.
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
    f.env(&mut cmd);
    let out = cmd
        .args(["publish", "--push", "--owner", "acme", "--json", "--dir"])
        .arg(&f.dir)
        .arg("--home")
        .arg(f.root.join("absent"))
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(3), "{v}");
    assert!(v["next"].as_str().unwrap().contains("--push --owner acme"));
    // Preflight fixes retain the same account.
    let out = f
        .command()
        .args(["--owner", "acme"])
        .env_remove("FAKE_GH_USER")
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["errors"][0]["code"], "env.gh_auth");
    assert!(
        v["errors"][0]["fix"]
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
    let (code, v) = f.run(&["--owner", "acme"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "publish.local_dependency");
    assert!(v["next"].as_str().unwrap().ends_with("--push --owner acme"));
}
