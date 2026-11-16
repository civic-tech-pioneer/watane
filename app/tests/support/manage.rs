use super::{driver::App, journey::*, publish::*};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const DESK: &str = "mod-manager";
pub const MANAGE: &str = "Manage your mods: see every mod and its grants, turn mods off, withdraw grants, and ask you to add, turn on or remove mods";
pub const ASKED: &str = "Asked by Desk (local), which manages your mods";
pub const CALL_KEY: &str = "call mods:reading-list/books@0.1.0 from local/reading-list";
pub fn call_grant() -> Value {
    json!({"call":{"interface":"mods:reading-list/books@0.1.0", "provider":"local/reading-list"}})
}
pub fn host_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../host/tests/fixtures/mods")
        .join(name)
}
pub struct Fixture {
    pub desk: PathBuf,
    pub weather: PathBuf,
    pub notes: PathBuf,
    pub ivy: watane_dist::Dist,
    pub link: String,
    pub release: String,
    _site: Site,
    _ivy_home: tempfile::TempDir,
    _folders: tempfile::TempDir,
}
impl Fixture {
    pub fn new() -> Self {
        let folders = tempfile::tempdir().unwrap();
        let desk = folders.path().join("manager");
        copy_dir(&host_fixture("manager"), &desk);
        copy_dir(&fixture("desk-ui/ui"), &desk.join("ui"));
        let meta = desk.join("mod.toml");
        let mut text = std::fs::read_to_string(&meta).unwrap();
        text.push_str("\nmain = true\n");
        std::fs::write(meta, text).unwrap();
        let weather = folders.path().join("reading-list");
        copy_dir(&fixture("reading-list"), &weather);
        rename(&weather, "Reading list", "Weather");
        let notes = folders.path().join("recommendations");
        copy_dir(&fixture("recommendations"), &notes);
        rename(&notes, "Recommendations", "Notes");
        let ivy_home = tempfile::tempdir().unwrap();
        let ivy = dist(ivy_home.path());
        ivy.set_name("Ivy").unwrap();
        let site = Site::serve(ivy.site_dir());
        ivy.set_site_url(&site.base).unwrap();
        let published = ivy.publish(book_package("0.1.0")).unwrap();
        Self {
            desk,
            weather,
            notes,
            link: published.link.unwrap().to_string(),
            release: published.info.reference.to_string(),
            ivy,
            _site: site,
            _ivy_home: ivy_home,
            _folders: folders,
        }
    }
    pub fn install(&self, app: &App) {
        begin_install(
            app,
            &[self.desk.clone(), self.weather.clone(), self.notes.clone()],
        );
        approve(app);
        for local in ["manager", "reading-list", "recommendations"] {
            installed(app, local);
        }
        open(app, "manager");
        until(app, DESK, "!!document.querySelector('#request-install')");
    }
    pub fn choose(&self, app: &App) {
        choose(app, &self.ivy.own_link(None).unwrap().to_string());
        app.eval("main", "document.querySelector('#search-text').value = 'Book club'; document.querySelector('#search').click(); null");
        until(
            app,
            "main",
            &format!(
                "Array.from(document.querySelectorAll('#results [data-release]')).some(e => e.dataset.release === {})",
                json!(self.release)
            ),
        );
    }
    pub fn ask_install(&self, app: &App) {
        click(
            app,
            "request-install",
            json!({"release":self.release,"link":self.link}),
        );
    }
}
fn rename(folder: &Path, from: &str, to: &str) {
    let path = folder.join("mod.toml");
    std::fs::write(
        &path,
        std::fs::read_to_string(&path).unwrap().replace(from, to),
    )
    .unwrap();
}
pub fn book_package(version: &str) -> Vec<u8> {
    let folder = tempfile::tempdir().unwrap();
    copy_dir(&host_fixture("counter"), folder.path());
    rename(folder.path(), "Counter", "Book club");
    let wit = folder.path().join("wit/world.wit");
    std::fs::write(
        &wit,
        std::fs::read_to_string(&wit)
            .unwrap()
            .replace("mods:counter@0.1.0", &format!("mods:counter@{version}")),
    )
    .unwrap();
    let wasm = folder.path().join("logic.wasm");
    let mut bytes = std::fs::read(&wasm).unwrap();
    for interface in ["api", "changes"] {
        let old = format!("mods:counter/{interface}@0.1.0");
        let new = format!("mods:counter/{interface}@{version}");
        assert_eq!(old.len(), new.len());
        let positions: Vec<_> = bytes
            .windows(old.len())
            .enumerate()
            .filter_map(|(i, b)| (b == old.as_bytes()).then_some(i))
            .collect();
        for i in positions {
            bytes[i..i + old.len()].copy_from_slice(new.as_bytes());
        }
    }
    std::fs::write(wasm, bytes).unwrap();
    watane_host::watane_pkg::pack(folder.path(), false)
        .unwrap()
        .into_bytes()
}
pub fn click(app: &App, function: &str, fields: Value) {
    until(app, DESK, "!!document.querySelector('#result')");
    for (key, value) in fields.as_object().unwrap() {
        let property = if key == "delete-data" {
            "checked"
        } else {
            "value"
        };
        app.eval(
            DESK,
            &format!(
                "document.getElementById({}).{property} = {}; null",
                json!(key),
                value
            ),
        );
    }
    app.eval(
        DESK,
        &format!("document.getElementById({}).click(); null", json!(function)),
    );
}
pub fn outcome(app: &App, expected: &str) {
    until(
        app,
        DESK,
        &format!(
            "document.querySelector('#result').textContent.includes({})",
            json!(expected)
        ),
    );
}
pub fn consent_text(app: &App) -> String {
    consent(app);
    app.eval("consent", "document.body.textContent")
        .as_str()
        .unwrap()
        .to_owned()
}
pub fn mods_on_disk(app: &App) -> Vec<std::ffi::OsString> {
    let mut names: Vec<_> = std::fs::read_dir(app.home.join("mods"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    names
}
pub fn no_window(app: &App, label: &str) {
    let end = std::time::Instant::now() + TIMEOUT;
    while app.windows().iter().any(|w| w["label"] == label) {
        assert!(
            std::time::Instant::now() < end,
            "window stayed open: {label}"
        );
    }
}
pub fn refresh(app: &App) {
    app.eval("main", "window.dispatchEvent(new Event('focus')); null");
}
