#![cfg(feature = "fake-host")]
#![allow(
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "tests may panic, catch panics, and share lock-only mutexes that guard no data"
)]

use crate::{in_process, support};
use serde_json::{Value, json};
use std::ffi::OsStr;
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
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap()["mode"],
        "home"
    );
    Host(child)
}
fn key() -> String {
    "y".repeat(52)
}
fn release(slug: &str) -> String {
    format!("{}/{slug}@0.1.0", key())
}
fn link(slug: &str) -> String {
    format!("watane:{}@https://publisher.example/mods/#{slug}", key())
}
fn page(dir: &Path, slug: &str, source: bool) -> watane_pkg::Package {
    fs::create_dir_all(dir.join("wit")).unwrap();
    fs::create_dir_all(dir.join("ui")).unwrap();
    fs::write(
        dir.join("mod.toml"),
        "name = \"Page\"\nsummary = \"Example\"\n",
    )
    .unwrap();
    // The source file is data; this page package has no executable logic.
    fs::write(
        dir.join("wit/world.wit"),
        format!("package mods:{slug}@0.1.0;\nworld {slug} {{}}\n"),
    )
    .unwrap();
    fs::write(dir.join("ui/index.html"), "<p>Example</p>").unwrap();
    if source {
        fs::create_dir_all(dir.join("logic/src")).unwrap();
        fs::write(
            dir.join("logic/src/lib.rs"),
            "// reading_list\n// reading-list-extra is not a mention\n",
        )
        .unwrap();
    }
    watane_pkg::pack(dir, source).unwrap()
}
struct Fixture {
    _guard: std::sync::MutexGuard<'static, ()>,
    root: tempfile::TempDir,
    home: PathBuf,
    cache: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        fs::create_dir(&home).unwrap();
        let cache = root.path().join("cache");
        Self {
            _guard: guard,
            root,
            home,
            cache,
        }
    }
    fn run(&self, args: &[&str]) -> (i32, Value) {
        let mut all: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        all.extend([
            OsStr::new("--json"),
            OsStr::new("--home"),
            self.home.as_os_str(),
        ]);
        in_process::dev(
            self.root.path(),
            &all,
            &[
                ("WATANE_DEV_CACHE", Some(self.cache.as_os_str())),
                (
                    "WATANE_BIN",
                    Some(OsStr::new(env!("CARGO_BIN_EXE_fake-watane"))),
                ),
            ],
        )
    }
    /// `run` as a process: `new --from` without a slug or `--dir` writes to
    /// the process's folder (TECHNICAL 2.5).
    fn run_process(&self, args: &[&str]) -> (i32, Value) {
        let out = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(self.root.path())
            .env("WATANE_DEV_CACHE", &self.cache)
            .env("WATANE_BIN", env!("CARGO_BIN_EXE_fake-watane"))
            .args(args)
            .arg("--json")
            .arg("--home")
            .arg(&self.home)
            .output()
            .unwrap();
        let v = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| panic!("{out:?}"));
        (out.status.code().unwrap(), v)
    }
    fn config(&self, slug: &str, package: &watane_pkg::Package) {
        let path = self.root.path().join(format!("{slug}.pkg"));
        fs::write(&path, package.as_bytes()).unwrap();
        fs::write(self.home.join("fake.json"), json!({"fetch": {link(slug): {"release":release(slug), "package":path}, release(slug): {"release":release(slug), "package":path}}}).to_string()).unwrap();
    }
}
#[test]
fn ac1_derive_source_guides_origin_mentions_and_default_slug() {
    let f = Fixture::new();
    let package = page(&f.root.path().join("source"), "reading-list", true);
    f.config("reading-list", &package);
    let _host = start(&f.home);
    let (code, v) = f.run(&["new", "ratings", "--from", &link("reading-list")]);
    assert_eq!(code, 0, "{v}");
    let dir = f.root.path().join("ratings");
    let meta: toml::Table =
        toml::from_str(&fs::read_to_string(dir.join("mod.toml")).unwrap()).unwrap();
    assert_eq!(meta["name"].as_str(), Some("Ratings"));
    assert!(
        fs::read_to_string(dir.join("mod.toml"))
            .unwrap()
            .contains(&format!("release = \"{}\"", release("reading-list")))
    );
    assert!(
        fs::read_to_string(dir.join("wit/world.wit"))
            .unwrap()
            .starts_with("package mods:ratings@0.1.0;")
    );
    for name in ["AGENTS.md", "CLAUDE.md", ".gitignore"] {
        assert_eq!(
            fs::read_to_string(dir.join(name)).unwrap(),
            fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("template")
                    .join(name)
            )
            .unwrap()
            .replace("{{slug}}", "ratings")
            .replace("{{crate}}", "ratings")
            .replace("{{title}}", "Ratings")
            .replace(
                "{{crates}}",
                "- serde 1.0.229\n- serde_json 1.0.154\n- wit-bindgen 0.63.0"
            )
        );
    }
    let mut expected_files: Vec<_> = package
        .paths()
        .map(str::to_string)
        .chain(["AGENTS.md".into(), "CLAUDE.md".into(), ".gitignore".into()])
        .collect();
    expected_files.sort();
    assert_eq!(v["result"]["files"], json!(expected_files));
    assert_eq!(v["result"]["origin"], release("reading-list"));
    assert_eq!(v["result"]["from_slug"], "reading-list");
    assert_eq!(v["result"]["providers"], json!([]));
    assert_eq!(
        v["result"]["mentions"],
        json!([{"file":"logic/src/lib.rs","line":1},{"file":"wit/world.wit","line":2}])
    );
    assert_eq!(
        v["next"],
        format!(
            "Replace reading-list with ratings (and reading_list with ratings) at each result.mentions entry, then run: watane-dev test --json --dir {}",
            v["dir"].as_str().unwrap()
        )
    );
    let (code, v) = f.run_process(&["new", "--from", &link("reading-list")]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["result"]["slug"], "reading-list");
    assert_eq!(v["result"]["mentions"], json!([]));
    assert_eq!(
        v["next"],
        format!(
            "Edit wit/world.wit, logic/src/lib.rs and ui/, then run: watane-dev test --json --dir {}",
            v["dir"].as_str().unwrap()
        )
    );
}
#[test]
fn ac2_no_source_writes_nothing() {
    let f = Fixture::new();
    let p = page(&f.root.path().join("source"), "reading-list", false);
    f.config("reading-list", &p);
    let _h = start(&f.home);
    let (code, v) = f.run(&["new", "ratings", "--from", &link("reading-list")]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["errors"][0]["code"], "usage.no_source");
    assert_eq!(
        v["errors"][0]["message"],
        format!(
            "the release {} holds no source (no logic/Cargo.toml, logic/src/ or tests/)",
            release("reading-list")
        )
    );
    assert_eq!(
        v["errors"][0]["fix"],
        format!(
            "Build on a release that includes its source, or make a new mod that depends on this one: watane-dev new ratings --dir {dest} && watane-dev add --release '{}' --dir {dest} --home {}",
            link("reading-list"),
            f.home.canonicalize().unwrap().display(),
            dest =
                crate::support::shell_path(f.root.path().canonicalize().unwrap().join("ratings")),
        )
    );
    assert!(!f.root.path().join("ratings").exists());
}
#[test]
fn ac3_exists_precedes_fetch_and_absent_app_writes_nothing() {
    let f = Fixture::new();
    fs::create_dir(f.root.path().join("ratings")).unwrap();
    {
        let _h = start(&f.home);
        let (code, v) = f.run(&["new", "ratings", "--from", &link("reading-list")]);
        assert_eq!(code, 2, "{v}");
        assert_eq!(v["errors"][0]["code"], "usage.exists");
        assert!(!f.home.join("requests").exists())
    };
    let (code, v) = f.run(&["new", "other", "--from", &link("reading-list")]);
    assert_eq!(code, 3, "{v}");
    assert_eq!(v["errors"][0]["code"], "env.host_not_running");
    assert!(!f.root.path().join("other").exists());
}
#[test]
fn ac4_add_release_records_publisher_wit_link_and_rejects_bad_target() {
    let f = Fixture::new();
    let p = watane_pkg::pack(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ratings"),
        false,
    )
    .unwrap();
    f.config("ratings", &p);
    let _h = start(&f.home);
    let dir = f.root.path().join("stats");
    watane_dev::template::new_mod(&dir, "stats").unwrap();
    for target in [link("ratings"), release("ratings")] {
        let (code, v) = f.run(&["add", "--release", &target, "--dir", dir.to_str().unwrap()]);
        assert_eq!(code, 0, "{v}");
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(dir.join("deps.json")).unwrap()).unwrap(),
            json!({"mods:ratings":key()})
        );
        assert_eq!(
            fs::read(dir.join("wit/deps/ratings/world.wit")).unwrap(),
            p.file("wit/world.wit").unwrap()
        );
        let cfg: Value =
            serde_json::from_slice(&fs::read(dir.join(".watane-dev.json")).unwrap()).unwrap();
        let expected_link = if target.starts_with("watane:") {
            json!(target)
        } else {
            Value::Null
        };
        assert_eq!(
            cfg["release_providers"]["ratings"],
            json!({"release":release("ratings"),"package":p.id(),"link":expected_link})
        );
        assert_eq!(
            v["result"]["imports"],
            json!(["import mods:ratings/api@0.1.0;"])
        );
        assert_eq!(v["result"]["publisher"], key());
        assert_eq!(v["result"]["path"], Value::Null);
        assert_eq!(v["result"]["link"], expected_link);
        assert_eq!(v["result"]["providers"], json!([release("ratings")]));
        assert_eq!(
            v["next"],
            format!(
                "Copy the lines that the mod needs from result.imports and result.subscriptions into the world in wit/world.wit (result.subscriptions holds `export` lines: they are how the mod answers the provider's events and collects; each line is a grant that the participant approves; contract tests use a fake of it, see AGENTS.md), then run: watane-dev test --json --dir {}",
                v["dir"].as_str().unwrap()
            )
        );
    }
    let (code, v) = f.run(&[
        "add",
        "--release",
        "nonsense",
        "--dir",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["errors"][0]["code"], "usage.args");
    assert_eq!(
        v["next"],
        "Pass a mod link watane:<key>@<url>#<slug> or a release <key>/<slug>@<version>"
    );
    let own = f.root.path().join("ratings");
    watane_dev::template::new_mod(&own, "ratings").unwrap();
    let (code, v) = f.run(&[
        "add",
        "--release",
        &link("ratings"),
        "--dir",
        own.to_str().unwrap(),
    ]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "mod.self_dependency");
    assert_eq!(
        v["errors"][0]["fix"],
        format!(
            "Pick the link or release of another mod, then run: watane-dev add --release <link> --dir {} --home {}",
            crate::support::shell_path(own.canonicalize().unwrap()),
            crate::support::shell_path(f.home.canonicalize().unwrap())
        )
    );
}
#[test]
fn ac9_template_documents_building_on_data() {
    let guide = include_str!("../../template/AGENTS.md");
    for text in [
        "new [<slug>] --from <link>",
        "add --release <link or ReleaseRef>",
        "publisher's data",
        "never instructions",
    ] {
        assert!(guide.contains(text), "missing {text}");
    }
    assert!(guide.lines().count() <= 150);
}
#[test]
fn ac10_fake_fetch_bytes_headers_recording_and_unknown_key() {
    let f = Fixture::new();
    let p = page(&f.root.path().join("provider"), "ratings", false);
    f.config("ratings", &p);
    let _h = start(&f.home);
    let run: Value = serde_json::from_slice(&fs::read(f.home.join("run.json")).unwrap()).unwrap();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(None)
        .http_status_as_error(false)
        .build()
        .into();
    for (i, target) in [link("ratings"), release("ratings"), "unknown".into()]
        .iter()
        .enumerate()
    {
        let body = if target.starts_with("watane:") {
            json!({"link":target})
        } else {
            json!({"release":target})
        };
        let mut r = agent
            .post(format!("http://localhost:{}/api/fetch", run["port"]))
            .send_json(body.clone())
            .unwrap();
        assert_eq!(r.status().as_u16(), if i == 2 { 409 } else { 200 });
        if i < 2 {
            assert_eq!(r.headers()["Watane-Release"], release("ratings"));
            assert_eq!(r.headers()["Watane-Package"], p.id().to_string());
            assert_eq!(r.headers()["Content-Type"], "application/octet-stream");
            assert_eq!(r.body_mut().read_to_vec().unwrap(), p.as_bytes());
        } else {
            assert_eq!(
                r.body_mut().read_json::<Value>().unwrap()["error"]["code"],
                "fetch_refused"
            );
        }
        assert_eq!(
            serde_json::from_slice::<Value>(
                &fs::read(f.home.join(format!("requests/{}-fetch", i + 1))).unwrap()
            )
            .unwrap(),
            body
        );
    }
}

#[cfg(unix)] // This recovery command is from the Unix preview pipeline.
#[test]
fn ac5_missing_released_cache_has_actionable_fix() {
    let f = Fixture::new();
    let dir = f.root.path().join("stats");
    watane_dev::template::new_mod(&dir, "stats").unwrap();
    fs::create_dir_all(dir.join("wit/deps/ratings")).unwrap();
    fs::write(
        dir.join("wit/deps/ratings/world.wit"),
        "package mods:ratings@0.1.0; interface api { get: func() -> u32; }",
    )
    .unwrap();
    fs::write(
        dir.join("deps.json"),
        json!({"mods:ratings":key()}).to_string(),
    )
    .unwrap();
    // S86: serve carries the --dir and --home it was given.
    let home = f.root.path().canonicalize().unwrap();
    for given in [false, true] {
        let mut run = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        run.args(["serve", "--json", "--dir"])
            .arg(&dir)
            .env("WATANE_DEV_CACHE", &f.cache);
        if given {
            run.arg("--home").arg(f.root.path().join("."));
        }
        let out = run.output().unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(1), "{v}");
        assert_eq!(v["errors"][0]["code"], "mod.release_provider_missing");
        let home = if given {
            format!(" --home {}", crate::support::shell_path(&home))
        } else {
            String::new()
        };
        let fix = format!(
            "Start the watane app, then run: watane-dev add --release {} --dir {}{home}",
            release("ratings"),
            crate::support::shell_path(dir.canonicalize().unwrap())
        );
        assert_eq!(v["errors"][0]["fix"], fix);
        assert_eq!(
            v["next"],
            fix.strip_prefix("Start the watane app, then run: ")
                .unwrap()
        );
    }
}

#[test]
fn ac6_publish_unlisted_warning_provider_links_and_default_list() {
    let f = Fixture::new();
    let p = page(&f.root.path().join("source"), "stats", false);
    let dir = f.root.path().join("stats");
    p.unpack(&dir).unwrap();
    // The app owns listing decisions; warnings must preserve its answer even for a page.
    fs::write(
        f.home.join("fake.json"),
        json!({"providers":[{"provider":format!("{}/ratings",key()),"listed":false}]}).to_string(),
    )
    .unwrap();
    let _h = start(&f.home);
    let (code, v) = f.run(&["publish", "--dir", dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["result"]["providers"],
        json!([{"provider":format!("{}/ratings",key()),"listed":false}])
    );
    assert_eq!(v["warnings"].as_array().unwrap().len(), 1);
    assert_eq!(v["warnings"][0]["code"], "publish.provider_unlisted");
    assert!(
        v["warnings"][0]["message"]
            .as_str()
            .unwrap()
            .contains(&format!("{}/ratings", key()))
    );
}

fn run_mod(f: &Fixture, dir: &Path, command: &str, extra: &[&str], watane: &Path) -> (i32, Value) {
    let mut all = vec![
        OsStr::new(command),
        OsStr::new("--json"),
        OsStr::new("--dir"),
        dir.as_os_str(),
    ];
    all.extend(extra.iter().map(OsStr::new));
    in_process::dev(
        f.root.path(),
        &all,
        &[
            ("WATANE_DEV_CACHE", Some(f.cache.as_os_str())),
            ("WATANE_BIN", Some(watane.as_os_str())),
        ],
    )
}
fn consumer(f: &Fixture) -> PathBuf {
    let package = watane_pkg::pack(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ratings"),
        false,
    )
    .unwrap();
    f.config("ratings", &package);
    let _host = start(&f.home);
    let dir = f.root.path().join("stats");
    watane_dev::template::new_mod(&dir, "stats").unwrap();
    let (code, v) = f.run(&[
        "add",
        "--release",
        &link("ratings"),
        "--dir",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{v}");
    let path = dir.join("wit/world.wit");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        path,
        text.replace(
            "world stats {",
            "world stats {\n    import mods:ratings/api@0.1.0;",
        ),
    )
    .unwrap();
    let path = dir.join("logic/src/lib.rs");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        path,
        text.replace(
            "fn get() -> u32 {\n        decode(state::get(\"count\").as_deref())\n    }",
            "fn get() -> u32 { mods::ratings::api::get() }",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("tests/counter.json"),
        json!({"fakes":{"mods:ratings/api":{"get":41}},"steps":[{"call":"api.get","returns":41}]})
            .to_string(),
    )
    .unwrap();
    dir
}
#[test]
fn ac5_offline_contract_and_fake_preview_provider_order() {
    let mut f = Fixture::new();
    f.cache = support::cache().to_path_buf();
    let dir = consumer(&f);
    let fake = Path::new(env!("CARGO_BIN_EXE_fake-watane"));
    // consumer() dropped the app; only the cached provider and test fake remain.
    let (code, v) = run_mod(&f, &dir, "test", &[], fake);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["result"]["contract"]["passed"], 1);
    #[cfg(unix)] // This part starts and stops a preview using Unix process groups.
    {
        let (code, v) = run_mod(&f, &dir, "serve", &[], fake);
        assert_eq!(code, 0, "{v}");
        let home = PathBuf::from(v["result"]["home"].as_str().unwrap());
        let request: Value =
            serde_json::from_slice(&fs::read(home.join("requests/1-install")).unwrap()).unwrap();
        assert_eq!(run_mod(&f, &dir, "serve", &["--stop"], fake).0, 0);
        let folders = request["folders"].as_array().unwrap();
        assert_eq!(folders.len(), 2);
        assert!(folders[0].as_str().unwrap().contains("/providers/"));
        assert_eq!(folders[1], json!(dir.canonicalize().unwrap()));
        assert_eq!(v["result"]["pages"][0]["slug"], "ratings");
        assert_eq!(v["result"]["pages"][1]["slug"], "stats")
    };
}
#[cfg(unix)] // The real preview lifecycle uses Unix process-group controls.
#[test]
#[ignore = "slow: real preview installs the built consumer and a cached released provider"]
fn ac5_real_preview_installs_released_provider_before_consumer() {
    let mut f = Fixture::new();
    f.cache = support::cache().to_path_buf();
    let dir = consumer(&f);
    let real = PathBuf::from(std::env::var_os("WATANE_BIN").expect("WATANE_BIN"));
    let (code, v) = run_mod(&f, &dir, "serve", &[], &real);
    assert_eq!(code, 0, "{v}");
    assert_eq!(run_mod(&f, &dir, "serve", &["--stop"], &real).0, 0);
    assert_eq!(v["result"]["pages"][0]["slug"], "ratings");
    assert_eq!(v["result"]["pages"][1]["slug"], "stats");
}
#[test]
fn ac6_publish_passes_recorded_provider_link() {
    let mut f = Fixture::new();
    f.cache = support::cache().to_path_buf();
    let dir = consumer(&f);
    fs::write(
        f.home.join("fake.json"),
        json!({"providers":[{"provider":format!("{}/ratings",key()),"listed":false}]}).to_string(),
    )
    .unwrap();
    let _host = start(&f.home);
    let (code, v) = f.run(&["publish", "--dir", dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        fs::read_to_string(f.home.join("requests/1-provider-links")).unwrap(),
        link("ratings")
    );
    assert_eq!(v["warnings"][0]["code"], "publish.provider_unlisted");
}
#[test]
fn ac4_transitive_releases_are_fetched_by_reference_and_bounded() {
    let mut f = Fixture::new();
    f.cache = support::cache().to_path_buf();
    let stats = consumer(&f);
    assert_eq!(
        run_mod(
            &f,
            &stats,
            "test",
            &[],
            Path::new(env!("CARGO_BIN_EXE_fake-watane"))
        )
        .0,
        0
    );
    let p = watane_pkg::pack(&stats, false).unwrap();
    let ratings = watane_pkg::pack(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ratings"),
        false,
    )
    .unwrap();
    let outer = f.root.path().join("stats.pkg");
    fs::write(&outer, p.as_bytes()).unwrap();
    let inner = f.root.path().join("ratings.pkg");
    fs::write(&inner, ratings.as_bytes()).unwrap();
    // Start with a different cache to prove the recursive fetch rather than cache reuse.
    f.cache = f.root.path().join("fresh-cache");
    fs::write(f.home.join("fake.json"),json!({"fetch":{link("stats"):{"release":release("stats"),"package":outer},release("ratings"):{"release":release("ratings"),"package":inner}}}).to_string()).unwrap();
    let _h = start(&f.home);
    let dir = f.root.path().join("dashboard");
    watane_dev::template::new_mod(&dir, "dashboard").unwrap();
    let (code, v) = f.run(&[
        "add",
        "--release",
        &link("stats"),
        "--dir",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["result"]["providers"],
        json!([release("stats"), release("ratings")])
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(f.home.join("requests/2-fetch")).unwrap())
            .unwrap(),
        json!({"release":release("ratings")})
    );
    let config: Value =
        serde_json::from_slice(&fs::read(dir.join(".watane-dev.json")).unwrap()).unwrap();
    assert_eq!(
        config["release_providers"]["ratings"]["release"],
        release("ratings")
    );
}

#[test]
fn ac12_stage9_scenarios_layout_and_public_interface_are_documented() {
    let spec = include_str!("../../TECHNICAL.md");
    for scenario in 35..=40 {
        assert!(spec.contains(&format!("| DEV-S-{scenario} | `build_on::ac")));
    }
    for text in [
        "src/build_on.rs",
        "New { slug: Option<String>, from: Option<String>, home: Option<PathBuf> }",
        "Add { provider: Option<PathBuf>, release: Option<String>, home: Option<PathBuf> }",
        "pub fn fetch(",
        "pub fn new_from(",
        "pub fn add_release(",
        "| 9 (step 5) |",
    ] {
        assert!(spec.contains(text), "missing {text}");
    }
}

#[test]
fn ac1_derive_keeps_only_logic_wasm_with_explicit_and_default_slug() {
    let f = Fixture::new();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ratings");
    let expected = fs::read(fixture.join("logic.wasm")).unwrap();
    let source = f.root.path().join("source");
    watane_pkg::pack(&fixture, false)
        .unwrap()
        .unpack(&source)
        .unwrap();
    // Contract source makes this a source package without a rebuildable logic crate.
    fs::create_dir(source.join("tests")).unwrap();
    fs::write(
        source.join("tests/ratings.json"),
        r#"{"steps":[{"call":"api.get","returns":0}]}"#,
    )
    .unwrap();
    let package = watane_pkg::pack(&source, true).unwrap();
    assert!(package.has_source());
    assert!(!package.paths().any(|p| p == "logic/Cargo.toml"));
    f.config("ratings", &package);
    let _host = start(&f.home);
    for (slug, mut args) in [
        ("my-ratings", vec!["new", "my-ratings", "--from"]),
        ("ratings", vec!["new", "ratings", "--from"]),
        (
            "default-ratings",
            vec!["new", "--dir", "default-ratings", "--from"],
        ),
    ] {
        let target = link("ratings");
        args.push(&target);
        let (code, v) = f.run(&args);
        assert_eq!(code, 0, "{v}");
        if slug != "my-ratings" {
            assert_eq!(v["warnings"], json!([]));
        }
        let dir = f.root.path().join(slug);
        assert_eq!(fs::read(dir.join("logic.wasm")).unwrap(), expected);
        let dir_arg = dir.canonicalize().unwrap();
        let step = if slug == "my-ratings" {
            "Replace ratings with my-ratings (and ratings with my_ratings) at each result.mentions entry, write the logic source under logic/ (logic/Cargo.toml and logic/src) for my-ratings"
        } else {
            "Edit wit/world.wit and ui/ (logic.wasm is prebuilt and keeps its exports)"
        };
        assert_eq!(
            v["next"],
            format!(
                "{step}, then run: watane-dev test --json --dir {}",
                crate::support::shell_path(dir_arg)
            )
        );
        assert!(
            v["result"]["files"]
                .as_array()
                .unwrap()
                .contains(&json!("logic.wasm"))
        );
    }
}

#[test]
fn ac3_derive_discards_publisher_wasm_with_explicit_and_default_slug() {
    let f = Fixture::new();
    let source = f.root.path().join("source");
    fs::create_dir(&source).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ratings");
    // Unpack the committed component, then add source data without a cold build.
    watane_pkg::pack(&fixture, false)
        .unwrap()
        .unpack(&source.join("mod"))
        .unwrap();
    let source = source.join("mod");
    fs::create_dir_all(source.join("logic/src")).unwrap();
    fs::write(source.join("logic/src/lib.rs"), "// source data").unwrap();
    fs::write(
        source.join("logic/Cargo.toml"),
        "[package]\nname = \"ratings\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        source.join("mod.toml"),
        r#"name = "Publisher title"
summary = "Example"
"#,
    )
    .unwrap();
    let package = watane_pkg::pack(&source, true).unwrap();
    assert!(package.paths().any(|p| p == "logic.wasm"));
    f.config("ratings", &package);
    let _host = start(&f.home);
    for (slug, title, args) in [
        (
            "my-ratings",
            "My ratings",
            vec!["new", "my-ratings", "--from"],
        ),
        ("ratings", "Ratings", vec!["new", "--from"]),
    ] {
        let mut args = args;
        let target = link("ratings");
        args.push(&target);
        let (code, v) = f.run_process(&args);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["warnings"], json!([]));
        let dir = f.root.path().join(slug);
        let meta: toml::Table =
            toml::from_str(&fs::read_to_string(dir.join("mod.toml")).unwrap()).unwrap();
        assert_eq!(meta["name"].as_str(), Some(title));
        assert!(!dir.join("logic.wasm").exists());
        assert!(
            !v["result"]["files"]
                .as_array()
                .unwrap()
                .contains(&json!("logic.wasm"))
        );
    }
}

#[test]
fn ac4_fake_fetch_bad_package_file_refuses_and_keeps_serving() {
    let f = Fixture::new();
    let _host = start(&f.home);
    let run: Value = serde_json::from_slice(&fs::read(f.home.join("run.json")).unwrap()).unwrap();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(None)
        .http_status_as_error(false)
        .build()
        .into();
    // A directory is deterministically unreadable as bytes, even for privileged users.
    for path in [f.root.path().join("missing.pkg"), f.home.clone()] {
        fs::write(
            f.home.join("fake.json"),
            json!({"fetch":{"bad":{"release":release("ratings"),"package":path}}}).to_string(),
        )
        .unwrap();
        let mut response = agent
            .post(format!("http://127.0.0.1:{}/api/fetch", run["port"]))
            .send_json(json!({"link":"bad"}))
            .expect("fake must answer the bad fixture");
        assert_eq!(response.status().as_u16(), 409);
        let body: Value = response.body_mut().read_json().unwrap();
        assert_eq!(body["error"]["code"], "fetch_refused");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains(path.to_str().unwrap())
        );
        assert_eq!(
            agent
                .get(format!("http://127.0.0.1:{}/api/status", run["port"]))
                .call()
                .unwrap()
                .status()
                .as_u16(),
            200
        );
    }
}

#[test]
fn regress_s47_renamed_prebuilt_mod_warns() {
    // Bob derives a mod under a new slug from a package with logic.wasm and no
    // logic source; warn before Bob tries the incompatible contract tests.
    let f = Fixture::new();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ratings");
    let source = f.root.path().join("source");
    watane_pkg::pack(&fixture, false)
        .unwrap()
        .unpack(&source)
        .unwrap();
    fs::create_dir(source.join("tests")).unwrap();
    fs::write(
        source.join("tests/ratings.json"),
        r#"{"steps":[{"call":"api.get","returns":0}]}"#,
    )
    .unwrap();
    let package = watane_pkg::pack(&source, true).unwrap();
    f.config("ratings", &package);
    let _host = start(&f.home);
    let (code, v) = f.run(&["new", "my-ratings", "--from", &link("ratings")]);
    assert_eq!(code, 0, "{v}");
    let dir = f.root.path().join("my-ratings");
    assert!(dir.join("logic.wasm").is_file());
    assert_eq!(v["warnings"].as_array().unwrap().len(), 1);
    assert_eq!(v["warnings"][0]["code"], "new.prebuilt_renamed");
    assert_eq!(
        v["warnings"][0]["message"],
        "logic.wasm was built for ratings, not my-ratings: it will fail the contract tests until the mod has logic source"
    );
    assert_eq!(
        v["warnings"][0]["fix"],
        "Write the logic source under logic/ (logic/Cargo.toml and logic/src) for my-ratings, or run new --from again with the slug ratings"
    );
    assert_eq!(
        fs::read(dir.join("logic.wasm")).unwrap(),
        fs::read(fixture.join("logic.wasm")).unwrap()
    );
}
/// S93 acceptance: the `usage.no_source` fix is a command to paste. Pasted as written, its
/// `add --release ... --dir <destination>` names the folder that `new --from` did not create,
/// so it fails with `usage.not_a_mod` instead of adding the release as a provider.
#[test]
fn regress_s93_no_source_fix_runs_as_written() {
    let f = Fixture::new();
    let p = page(&f.root.path().join("source"), "reading-list", false);
    f.config("reading-list", &p);
    let _h = start(&f.home);
    let (code, v) = f.run(&["new", "ratings", "--from", &link("reading-list")]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["errors"][0]["code"], "usage.no_source");
    let dest = f.root.path().canonicalize().unwrap().join("ratings");
    let shell_dest = crate::support::shell_path(&dest);
    let dest = dest.to_str().unwrap();
    let fix = v["errors"][0]["fix"].as_str().unwrap();
    assert!(fix.contains(&format!("--dir {shell_dest}")), "{fix}");
    // The fix's commands, argument by argument: `new <slug> --dir <dest> && add --release ...`
    // (foreman, S93-fix5: the fix makes the mod first, since new --from writes nothing).
    assert!(
        fix.contains(&format!(
            "watane-dev new ratings --dir {shell_dest} && watane-dev add --release"
        )),
        "{fix}"
    );
    let (code, v) = in_process::dev(
        f.root.path(),
        &[
            OsStr::new("new"),
            OsStr::new("ratings"),
            OsStr::new("--dir"),
            OsStr::new(dest),
            OsStr::new("--json"),
        ],
        &[("WATANE_DEV_CACHE", Some(f.cache.as_os_str()))],
    );
    assert_eq!(code, 0, "{v}");
    let (code, v) = f.run(&["add", "--release", &link("reading-list"), "--dir", dest]);
    assert_ne!(v["errors"][0]["code"], "usage.not_a_mod", "{v}");
    assert_eq!(code, 0, "{v}");
}
#[cfg(unix)] // Executes the recovery command with POSIX shell paths.
#[test]
fn regress_s93_no_source_fix_without_slug_runs_as_written() {
    // `new --from <link>` with no slug: the fix names the release's own slug, so the new mod
    // and the release share it, and the fix's `add --release` refuses it as a self-dependency.
    let f = Fixture::new();
    let p = page(&f.root.path().join("source"), "reading-list", false);
    f.config("reading-list", &p);
    let _h = start(&f.home);
    let (code, v) = f.run_process(&["new", "--from", &link("reading-list")]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["errors"][0]["code"], "usage.no_source");
    let fix = v["errors"][0]["fix"].as_str().unwrap();
    let commands = fix.split_once(": ").unwrap().1;
    let bin = Path::new(env!("CARGO_BIN_EXE_watane-dev"))
        .parent()
        .unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let out = Command::new("sh")
        .arg("-c")
        .arg(commands)
        .current_dir(f.root.path())
        .env("PATH", path)
        .env("WATANE_DEV_CACHE", &f.cache)
        .env("WATANE_BIN", env!("CARGO_BIN_EXE_fake-watane"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{commands}\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
