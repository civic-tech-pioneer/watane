//! Client of the local API of a running watane app (TECHNICAL 9.1). dev opens
//! no connection except HTTP to `localhost`.

use crate::outcome::{Code, Problem};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use watane_host::{InstalledMod, RunInfo};
const FETCH_FIX: &str =
    "Pass a mod link watane:<key>@<url>#<slug> or a release <key>/<slug>@<version>";
/// A fix that ends in the command's rerun: `<step>, then run: <rerun>`.
/// Without a rerun it keeps the older prose (publish does: s86, below).
fn then_run(step: &str, rerun: Option<&str>) -> String {
    match rerun {
        Some(rerun) => format!("{step}, then run: {rerun}"),
        None => format!("{step}, then run this command again"),
    }
}
#[derive(Debug)]
pub struct Client {
    port: u16,
    home: PathBuf,
    /// The command's own rerun (built in cli.rs), which ends the fixes that
    /// ask for a retry; `None` keeps the older prose.
    rerun: Option<String>,
}
impl Client {
    pub fn for_app(home: &Path, rerun: Option<&str>) -> Result<Self, Problem> {
        let missing = |message| {
            Problem::new(Code::EnvHostNotRunning, message)
                .fix(then_run("Start the watane app", rerun))
        };
        let absent = || {
            missing(format!(
                "no watane app runs for the home {}",
                home.display()
            ))
        };
        let run = RunInfo::read(home).ok().flatten().ok_or_else(absent)?;
        let client = Self {
            port: run.port,
            home: home.to_path_buf(),
            rerun: rerun.map(str::to_string),
        };
        let status = client
            .request("status", None, Duration::from_secs(2))
            .map_err(|_| absent())?;
        if run.watane != 1
            || status["watane"] != 1
            || status["home"] != json!(home)
            || status["port"] != run.port
        {
            return Err(absent());
        }
        if status["mode"] == "preview" {
            return Err(missing(format!(
                "the host of {} is a preview",
                home.display()
            )));
        }
        if status["mode"] != "home" || run.mode != "home" {
            return Err(absent());
        }
        if status["app"] != true {
            return Err(missing(format!(
                "the host of {} runs without the app",
                home.display()
            )));
        }
        Ok(client)
    }

    /// Read status for either a home or a preview, without requiring an app.
    pub fn home_status(home: &Path) -> Option<Value> {
        let run = RunInfo::read(home).ok().flatten()?;
        let status = Self {
            port: run.port,
            home: home.to_path_buf(),
            rerun: None,
        }
        .request("status", None, Duration::from_secs(2))
        .ok()?;
        (status["watane"] == 1).then_some(status)
    }

    pub fn for_home(home: &Path) -> Result<Option<Self>, Problem> {
        // An unreadable run.json names no host to talk to (9.4 step 1).
        let Some(run) = RunInfo::read(home).ok().flatten() else {
            return Ok(None);
        };
        if run.watane != 1 || run.mode != "preview" {
            return Ok(None);
        }
        let client = Self {
            port: run.port,
            home: home.to_path_buf(),
            rerun: None,
        };
        let status = match client.request("status", None, Duration::from_secs(2)) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        if status["watane"] == 1
            && status["mode"] == "preview"
            && status["home"] == json!(home)
            && status["port"] == run.port
        {
            Ok(Some(client))
        } else {
            Ok(None)
        }
    }
    fn bad_fetch_answer(&self, detail: impl std::fmt::Display) -> Problem {
        Problem::new(Code::HostRefused, format!("[bad_answer] {detail}")).fix(self.then(
            "The watane app answered with a package that does not match the request; ask the person to update the watane app",
        ))
    }
    fn then(&self, step: &str) -> String {
        then_run(step, self.rerun.as_deref())
    }
    fn app_failure(&self, status: u16, value: &Value) -> Option<Problem> {
        if !(500..600).contains(&status) {
            return None;
        }
        let code = value["error"]["code"].as_str().unwrap_or("unknown");
        let message = value["error"]["message"]
            .as_str()
            .unwrap_or("unknown host error");
        Some(if status == 503 && code == "unavailable" {
            Problem::new(
                Code::EnvHostNotRunning,
                format!(
                    "the watane app of {} runs but could not open its data: {message}",
                    self.home.display()
                ),
            )
            .fix(self.then("Ask the person to restart the watane app"))
        } else {
            Problem::new(Code::HostFailed, format!("[{code}] {message}")).fix(self.then(
                "The watane app failed on this request; ask the person to read the watane app's log",
            ))
        })
    }

    fn request(
        &self,
        path: &str,
        body: Option<(&str, &[u8])>,
        timeout: Duration,
    ) -> Result<Value, Problem> {
        self.request_headers(path, body, timeout, &[])
    }
    fn request_headers(
        &self,
        path: &str,
        body: Option<(&str, &[u8])>,
        timeout: Duration,
        links: &[String],
    ) -> Result<Value, Problem> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(None)
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .max_redirects(0)
            .build()
            .into();
        let url = format!("http://localhost:{}/api/{path}", self.port);
        let response = if let Some((content_type, body)) = body {
            let mut request = agent.post(&url).header("Content-Type", content_type);
            if !links.is_empty() {
                request = request.header("Watane-Provider-Links", links.join(" "));
            }
            request.send(body)
        } else {
            agent.get(&url).call()
        };
        let mut response = response.map_err(|e| {
            Problem::new(Code::HostNoAnswer, e.to_string())
                .fix("Check the preview log and run the command again")
        })?;
        let status = response.status();
        let value: Value = response.body_mut().read_json().map_err(|e| {
            if matches!(e, ureq::Error::Timeout(_)) {
                Problem::new(Code::HostNoAnswer, e.to_string())
                    .fix("Check the preview log and run the command again")
            } else {
                Problem::internal(e)
            }
        })?;
        if let Some(error) = self.app_failure(status.as_u16(), &value) {
            return Err(error);
        }
        if !status.is_success() || value.get("error").is_some() {
            if path == "publish"
                && status.as_u16() == 409
                && value["error"]["code"] == "version_exists"
            {
                return Err(Problem::new(
                    Code::PublishVersionExists,
                    value["error"]["message"]
                        .as_str()
                        .unwrap_or("version already published"),
                )
                .at("wit/world.wit", None, None)
                .fix("This version is already published. To publish changes, raise the version in wit/world.wit, then run publish again"));
            }
            let code =
                if !matches!(path, "releases" | "search") && value["error"]["code"] == "refused" {
                    Code::HostDeclined
                } else {
                    Code::HostRefused
                };
            return Err(Problem::new(
                code,
                if code == Code::HostDeclined {
                    value["error"]["message"]
                        .as_str()
                        .unwrap_or("declined")
                        .to_owned()
                } else {
                    format!(
                        "[{}] {}",
                        value["error"]["code"].as_str().unwrap_or("unknown"),
                        value["error"]["message"]
                            .as_str()
                            .unwrap_or("unknown host error")
                    )
                },
            )
            .fix(if path == "search" && status.as_u16() == 400 {
                "Check --publisher (a 52-character key) and --interface (for example mods:reading-list/books or mods:reading-list/books@0.1.0)"
            } else if code == Code::HostDeclined {
                if path == "publish" {
                    "The person declined in the app. Ask them before you run publish again"
                } else {
                    "The person declined in the app. Ask them before you run install again"
                }
            } else {
                "The watane app could not complete this command; ask the person to check the watane app, then run the command again"
            }));
        }
        Ok(value)
    }
    /// Fetch verified release bytes through the home app; never stores a response.
    pub fn fetch(
        &self,
        target: &str,
    ) -> Result<(watane_pkg::ReleaseRef, watane_pkg::Package), Problem> {
        use watane_pkg::{MAX_PACKAGE_BYTES, Package, PackageId, ReleaseRef};
        let asked = if target.starts_with("watane:") {
            None
        } else {
            Some(target.parse::<ReleaseRef>().map_err(|_| {
                Problem::new(Code::UsageArgs, "expected a mod link or ReleaseRef").fix(FETCH_FIX)
            })?)
        };
        let body = if asked.is_some() {
            json!({"release":target})
        } else {
            json!({"link":target})
        };
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(None)
            .timeout_global(Some(Duration::from_secs(330)))
            .http_status_as_error(false)
            .max_redirects(0)
            .build()
            .into();
        let mut response = agent
            .post(format!("http://localhost:{}/api/fetch", self.port))
            .send_json(body)
            .map_err(|e| {
                Problem::new(Code::HostNoAnswer, e.to_string())
                    .fix(self.then("Check that the watane app is running"))
            })?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let value: Value = response.body_mut().read_json().map_err(Problem::internal)?;
            if !(status == 502 && value["error"]["code"] == "fetch_failed")
                && let Some(error) = self.app_failure(status, &value)
            {
                return Err(error);
            }
            let e = Problem::new(
                Code::HostRefused,
                format!(
                    "[{}] {}",
                    value["error"]["code"].as_str().unwrap_or("unknown"),
                    value["error"]["message"]
                        .as_str()
                        .unwrap_or("unknown host error")
                ),
            );
            return Err(e.fix(if status == 400 {
                FETCH_FIX.into()
            } else if status == 409 && value["error"]["code"] == "fetch_refused" {
                "Check the link or release with the person who shared it: the watane app found no release to fetch for it".into()
            } else if status == 502 && value["error"]["code"] == "fetch_failed" {
                self.then("Check the link and that its publisher's site is reachable")
            } else {
                FETCH_FIX.into()
            }));
        }
        let release: ReleaseRef = response
            .headers()
            .get("Watane-Release")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| self.bad_fetch_answer("missing Watane-Release"))?
            .parse()
            .map_err(|e| self.bad_fetch_answer(e))?;
        let id: PackageId = response
            .headers()
            .get("Watane-Package")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| self.bad_fetch_answer("missing Watane-Package"))?
            .parse()
            .map_err(|e| self.bad_fetch_answer(e))?;
        let matches = if let Some(asked) = asked {
            release == asked
        } else {
            // dist owns link validation; dev reads only these two identity fields.
            target
                .strip_prefix("watane:")
                .and_then(|s| s.split_once('@'))
                .map(|(key, _)| key)
                == Some(release.publisher.to_string().as_str())
                && target.rsplit_once('#').map(|(_, slug)| slug) == Some(release.slug.as_str())
        };
        if !matches {
            return Err(
                self.bad_fetch_answer("Watane-Release does not match the requested release")
            );
        }
        use std::io::Read;
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAX_PACKAGE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(Problem::internal)?;
        if bytes.len() as u64 > MAX_PACKAGE_BYTES {
            return Err(Problem::new(
                Code::HostRefused,
                "[too_large] package exceeds MAX_PACKAGE_BYTES",
            ));
        }
        let package = Package::from_bytes(bytes).map_err(|e| self.bad_fetch_answer(e))?;
        if package.id() != id {
            return Err(self.bad_fetch_answer("Watane-Package does not match the package body"));
        }
        let world = package
            .file("wit/world.wit")
            .ok_or_else(|| self.bad_fetch_answer("package has no wit/world.wit"))?;
        let (_, slug, version) = crate::build_on::declaration(
            std::str::from_utf8(world).map_err(|e| self.bad_fetch_answer(e))?,
        )
        .map_err(|e| self.bad_fetch_answer(e.message))?;
        if slug != release.slug || version != release.version {
            return Err(self.bad_fetch_answer("Watane-Release does not match the package body"));
        }
        Ok((release, package))
    }

    pub fn search(
        &self,
        text: &str,
        publisher: Option<&str>,
        interface: Option<&str>,
    ) -> Result<Value, Problem> {
        self.request(
            "search",
            Some((
                "application/json",
                json!({"text":text,"publisher":publisher,"interface":interface})
                    .to_string()
                    .as_bytes(),
            )),
            Duration::from_secs(30),
        )
    }

    pub fn releases(&self) -> Result<Value, Problem> {
        self.request("releases", None, Duration::from_secs(2))
    }

    pub fn publish_with_links(
        &self,
        package: &watane_pkg::Package,
        wait: Duration,
        links: &[String],
    ) -> Result<Value, Problem> {
        self.request_headers(
            "publish",
            Some(("application/octet-stream", package.as_bytes())),
            wait,
            links,
        )
        .map_err(|e| home_wait_error(e, wait))
    }

    pub fn install_wait(
        &self,
        folders: &[PathBuf],
        wait: u64,
    ) -> Result<Vec<InstalledMod>, Problem> {
        self.install_timeout(folders, Duration::from_secs(wait))
            .map_err(|e| home_wait_error(e, Duration::from_secs(wait)))
    }
    pub fn install(&self, folders: &[PathBuf]) -> Result<Vec<InstalledMod>, Problem> {
        self.install_timeout(folders, Duration::from_secs(60))
    }
    fn install_timeout(
        &self,
        folders: &[PathBuf],
        timeout: Duration,
    ) -> Result<Vec<InstalledMod>, Problem> {
        #[derive(Deserialize)]
        struct Installed {
            installed: Vec<InstalledMod>,
        }
        let value = self
            .request("install", Some(("application/json", json!({"folders":folders}).to_string().as_bytes())), timeout)
            .map_err(|mut e| {
                if e.code == Code::HostRefused && e.message.starts_with("[missing_providers]") {
                    for dir in folders {
                        if let Ok(deps) = watane_pkg::Deps::read(dir) {
                            for (slug, publisher) in deps.iter() {
                                if let Some(publisher) = publisher {
                                    let provider = format!("{publisher}/{slug}");
                                    if e.message.contains(&provider)
                                        && let Ok(release) = crate::build_on::provider_release(dir, slug, publisher) {
                                        let link = crate::deps::read_config(dir).ok()
                                            .and_then(|config| config.release_providers.get(slug.as_str()).and_then(|record| record.link.clone()));
                                        e.fix = Some(match link {
                                            Some(link) => format!("Install {release} in the watane app from its link {link}, then run install again"),
                                            None => format!("Install {release} in the watane app (from its link), then run install again"),
                                        });
                                        return e;
                                    }
                                }
                            }
                        }
                    }
                }
                if e.code == Code::HostDeclined {
                    let name = folders.last().map_or_else(|| "the mod".into(), |dir| crate::deps::label(dir));
                    e.message = format!("The person declined installing {name} in their watane app");
                    e.fix = Some(format!("Ask the person whether they want {name} installed in their watane app, then run install again with --wait <seconds>"));
                }
                e
            })?;
        serde_json::from_value::<Installed>(value)
            .map(|v| v.installed)
            .map_err(Problem::internal)
    }
}

fn home_wait_error(mut e: Problem, wait: Duration) -> Problem {
    if e.code == Code::HostNoAnswer {
        e.message = format!("no answer from the home host in {} s", wait.as_secs());
        e.fix = Some("Run the command again with a longer --wait".into());
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn client(port: u16) -> Client {
        Client {
            port,
            home: PathBuf::from("/home/test"),
            rerun: Some("watane-dev cmd --json".into()),
        }
    }

    #[test]
    fn ac4_app_5xx_names_data_failure_or_app_log_for_every_request() {
        let package = publish_package();
        for path in ["install", "publish", "search", "releases", "fetch"] {
            for (status, app_code, code, message, fix, exit) in [
                (
                    503,
                    "unavailable",
                    "env.host_not_running",
                    "the watane app of /home/test runs but could not open its data: broken data",
                    "Ask the person to restart the watane app, then run: watane-dev cmd --json",
                    3,
                ),
                (
                    500,
                    "internal",
                    "host.failed",
                    "[internal] broken data",
                    "The watane app failed on this request; ask the person to read the watane app's log, then run: watane-dev cmd --json",
                    4,
                ),
                (
                    503,
                    "other",
                    "host.failed",
                    "[other] broken data",
                    "The watane app failed on this request; ask the person to read the watane app's log, then run: watane-dev cmd --json",
                    4,
                ),
            ] {
                let (port, thread) = stub(
                    status,
                    json!({"error":{"code":app_code,"message":"broken data"}}),
                    Duration::ZERO,
                );
                let client = client(port);
                let error = match path {
                    "fetch" => client
                        .fetch(&format!("{}/demo@0.2.3", "y".repeat(52)))
                        .unwrap_err(),
                    "install" => client.install_wait(&[], 1).unwrap_err(),
                    "publish" => client
                        .publish_with_links(&package, Duration::from_secs(1), &[])
                        .unwrap_err(),
                    "search" => client.search("", None, None).unwrap_err(),
                    _ => client.releases().unwrap_err(),
                };
                thread.join().unwrap();
                assert_eq!(error.code.as_str(), code, "{path}");
                assert_eq!(error.code.class() as u8, exit);
                assert_eq!(error.message, message, "{path}");
                assert_eq!(error.fix.as_deref(), Some(fix), "{path}");
            }
        }
    }

    #[test]
    fn ac2_fetch_errors_name_link_and_publisher_recovery() {
        let asked = format!("{}/demo@0.2.3", "y".repeat(52));
        for (status, code, fix) in [
            (
                409,
                "fetch_refused",
                "Check the link or release with the person who shared it: the watane app found no release to fetch for it",
            ),
            (
                502,
                "fetch_failed",
                "Check the link and that its publisher's site is reachable, then run: watane-dev cmd --json",
            ),
            // Only a 502 fetch_failed names the publisher's site.
            (404, "fetch_failed", FETCH_FIX),
        ] {
            let (port, thread) = stub(
                status,
                json!({"error":{"code":code,"message":"cannot fetch"}}),
                Duration::ZERO,
            );
            let error = client(port).fetch(&asked).unwrap_err();
            thread.join().unwrap();
            assert_eq!(error.code, Code::HostRefused);
            assert_eq!(error.message, format!("[{code}] cannot fetch"));
            assert_eq!(error.fix.as_deref(), Some(fix));
        }
    }

    #[test]
    fn ac5_missing_released_provider_includes_stored_link() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("stats");
        crate::template::new_mod(&dir, "stats").unwrap();
        let key = "y".repeat(52);
        std::fs::write(
            dir.join("deps.json"),
            json!({"mods:ratings":key}).to_string(),
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("wit/deps/ratings")).unwrap();
        std::fs::write(
            dir.join("wit/deps/ratings/world.wit"),
            "package mods:ratings@0.1.0; interface api { get: func() -> u32; }",
        )
        .unwrap();
        std::fs::write(dir.join(".watane-dev.json"), json!({"release_providers":{"ratings":{
            "release":format!("{key}/ratings@0.1.0"), "package":watane_pkg::PackageId::of(b"ratings"),
            "link":"watane:ratings-link"
        }}}).to_string()).unwrap();
        let (port, thread) = stub(
            409,
            json!({"error":{"code":"missing_providers","message":format!("{key}/ratings is not installed")}}),
            Duration::ZERO,
        );
        let error = client(port).install_wait(&[dir], 1).unwrap_err();
        thread.join().unwrap();
        assert_eq!(error.code, Code::HostRefused);
        assert_eq!(
            error.fix,
            Some(format!(
                "Install {key}/ratings@0.1.0 in the watane app from its link watane:ratings-link, then run install again"
            ))
        );
    }

    #[test]
    fn ac7_missing_released_provider_names_app_install_fix() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("stats");
        crate::template::new_mod(&dir, "stats").unwrap();
        let key = "y".repeat(52);
        std::fs::write(
            dir.join("deps.json"),
            json!({"mods:ratings":key}).to_string(),
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("wit/deps/ratings")).unwrap();
        std::fs::write(
            dir.join("wit/deps/ratings/world.wit"),
            "package mods:ratings@0.1.0; interface api { get: func() -> u32; }",
        )
        .unwrap();
        let (port, thread) = stub(
            409,
            json!({"error":{"code":"missing_providers","message":format!("{key}/ratings is not installed")}}),
            Duration::ZERO,
        );
        let error = client(port).install_wait(&[dir], 1).unwrap_err();
        thread.join().unwrap();
        assert_eq!(error.code, Code::HostRefused);
        assert_eq!(
            error.fix,
            Some(format!(
                "Install {key}/ratings@0.1.0 in the watane app (from its link), then run install again"
            ))
        );
    }

    #[test]
    fn ac3_fetch_refuses_corrupt_headers_bytes_and_app_errors() {
        fn binary(
            status: u16,
            release: &str,
            id: &str,
            body: Vec<u8>,
        ) -> (u16, std::thread::JoinHandle<()>) {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let release_header = if release.is_empty() {
                String::new()
            } else {
                format!("Watane-Release: {release}\r\n")
            };
            let id_header = if id.is_empty() {
                String::new()
            } else {
                format!("Watane-Package: {id}\r\n")
            };
            let headers = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\n{release_header}{id_header}Connection: close\r\n\r\n",
                body.len()
            );
            let thread = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut b = [0];
                    stream.read_exact(&mut b).unwrap();
                    header.push(b[0]);
                }
                let header = String::from_utf8(header).unwrap();
                assert!(header.starts_with("POST /api/fetch HTTP/1.1"));
                assert!(!header.to_lowercase().contains("origin:"));
                let len: usize = header
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap();
                let mut request = vec![0; len];
                stream.read_exact(&mut request).unwrap();
                let request: Value = serde_json::from_slice(&request).unwrap();
                assert!(request.get("release").is_some() || request.get("link").is_some());
                let _ = stream.write_all(headers.as_bytes());
                let _ = stream.write_all(&body);
            });
            (port, thread)
        }
        let p = publish_package();
        let asked = format!("{}/demo@0.2.3", "y".repeat(52));
        for (release, id, bytes, expected) in [
            (
                asked.clone(),
                "0".repeat(64),
                p.as_bytes().to_vec(),
                Code::HostRefused,
            ),
            (
                "nonsense".into(),
                p.id().to_string(),
                p.as_bytes().to_vec(),
                Code::HostRefused,
            ),
            (
                format!("{}/other@0.2.3", "y".repeat(52)),
                p.id().to_string(),
                p.as_bytes().to_vec(),
                Code::HostRefused,
            ),
            (
                asked.clone(),
                p.id().to_string(),
                b"not a package".to_vec(),
                Code::HostRefused,
            ),
            (
                asked.clone(),
                p.id().to_string(),
                vec![0; watane_pkg::MAX_PACKAGE_BYTES as usize + 1],
                Code::HostRefused,
            ),
            (
                "".into(),
                p.id().to_string(),
                p.as_bytes().to_vec(),
                Code::HostRefused,
            ),
            (
                asked.clone(),
                "".into(),
                p.as_bytes().to_vec(),
                Code::HostRefused,
            ),
            (
                asked.clone(),
                "nonsense".into(),
                p.as_bytes().to_vec(),
                Code::HostRefused,
            ),
            (
                format!("{}/demo@0.2.4", "y".repeat(52)),
                p.id().to_string(),
                p.as_bytes().to_vec(),
                Code::HostRefused,
            ),
            (
                format!("{}/demo@9.0.0", "y".repeat(52)),
                p.id().to_string(),
                p.as_bytes().to_vec(),
                Code::HostRefused,
            ),
        ] {
            let target = if release.ends_with("@9.0.0") || release.contains("/other@") {
                release.as_str()
            } else {
                &asked
            };
            let (port, thread) = binary(200, &release, &id, bytes);
            let error = client(port)
                .fetch(target)
                .expect_err("reject corrupt response");
            thread.join().unwrap();
            assert_eq!(error.code, expected, "{error:?}");
            if !error.message.starts_with("[too_large]") {
                assert!(error.message.starts_with("[bad_answer] "), "{error:?}");
                assert_eq!(
                    error.fix.as_deref(),
                    Some(
                        "The watane app answered with a package that does not match the request; ask the person to update the watane app, then run: watane-dev cmd --json"
                    )
                );
            }
        }
        // A link is matched on its key (after `watane:`) and its slug (after `#`) only.
        for link in [
            format!("watane:{}@https://p.example/mods/#other", "y".repeat(52)),
            format!("watane:{}@https://p.example/mods/#demo", "z".repeat(52)),
        ] {
            let (port, thread) = binary(200, &asked, &p.id().to_string(), p.as_bytes().to_vec());
            let error = client(port).fetch(&link).expect_err("reject other release");
            thread.join().unwrap();
            assert_eq!(error.code, Code::HostRefused, "{error:?}");
        }
        let link = format!("watane:{}@https://p.example/mods/#demo", "y".repeat(52));
        let (port, thread) = binary(200, &asked, &p.id().to_string(), p.as_bytes().to_vec());
        let (release, _) = client(port).fetch(&link).unwrap();
        thread.join().unwrap();
        assert_eq!(release.to_string(), asked);
        for (status, code) in [
            (400, "bad_request"),
            (409, "fetch_refused"),
            (502, "fetch_failed"),
            (503, "unavailable"),
        ] {
            let (port, thread) = stub(
                status,
                json!({"error":{"code":code,"message":"cannot fetch"}}),
                Duration::ZERO,
            );
            let error = client(port).fetch(&asked).err().unwrap();
            thread.join().unwrap();
            assert_eq!(
                error.code,
                if status == 503 {
                    Code::EnvHostNotRunning
                } else {
                    Code::HostRefused
                }
            );
            assert_eq!(
                error.message,
                if status == 503 {
                    "the watane app of /home/test runs but could not open its data: cannot fetch"
                        .into()
                } else {
                    format!("[{code}] cannot fetch")
                }
            );
            if status == 400 {
                assert_eq!(
                    error.fix.as_deref(),
                    Some(
                        "Pass a mod link watane:<key>@<url>#<slug> or a release <key>/<slug>@<version>"
                    )
                );
            }
        }
    }

    #[test]
    fn ac4_released_provider_walk_stops_at_thirty_two_packages() {
        let root = tempfile::tempdir().unwrap();
        let env = crate::env::Env {
            cache: root.path().join("cache"),
            unit_seconds: 60,
        };
        let mut records = serde_json::Map::new();
        let key = "y".repeat(52);
        let consumer = root.path().join("consumer");
        for n in 0..33 {
            if n == 32 {
                // Exactly 32 cached packages are still allowed.
                std::fs::write(
                    consumer.join(".watane-dev.json"),
                    json!({"release_providers":records}).to_string(),
                )
                .unwrap();
                let order = crate::build_on::fetch_providers(&consumer, &client(0), &env).unwrap();
                assert_eq!(order.len(), 32);
            }
            let slug = format!("p{n}");
            let id = watane_pkg::PackageId::of(slug.as_bytes());
            let folder = env.cache.join("providers").join(id.to_string());
            std::fs::create_dir_all(&folder).unwrap();
            records.insert(
                slug.clone(),
                json!({"release":format!("{key}/{slug}@0.1.0"),"package":id,"link":null}),
            );
            let parent = if n == 0 {
                consumer.clone()
            } else {
                env.cache
                    .join("providers")
                    .join(watane_pkg::PackageId::of(format!("p{}", n - 1).as_bytes()).to_string())
            };
            std::fs::create_dir_all(parent.join("wit/deps").join(&slug)).unwrap();
            std::fs::write(
                parent.join("deps.json"),
                json!({format!("mods:{slug}"):key}).to_string(),
            )
            .unwrap();
            std::fs::write(
                parent.join("wit/deps").join(&slug).join("world.wit"),
                format!("package mods:{slug}@0.1.0; interface api {{ get: func(); }}"),
            )
            .unwrap();
        }
        std::fs::write(
            consumer.join(".watane-dev.json"),
            json!({"release_providers":records}).to_string(),
        )
        .unwrap();
        let errors = crate::build_on::fetch_providers(&consumer, &client(0), &env).unwrap_err();
        assert_eq!(errors[0].code, Code::HostRefused);
        assert_eq!(errors[0].message, "too many released providers");
    }

    #[test]
    fn ac4_add_release_requires_a_mod_before_fetching() {
        let root = tempfile::tempdir().unwrap();
        let env = crate::env::Env {
            cache: root.path().join("cache"),
            unit_seconds: 60,
        };
        let errors = crate::build_on::add_release(
            root.path(),
            &format!("{}/ratings@0.1.0", "y".repeat(52)),
            &client(0),
            &env,
            "",
        )
        .unwrap_err();
        assert_eq!(errors[0].code, Code::UsageNotAMod);
        assert!(!env.cache.exists());
    }

    fn stub(status: u16, value: Value, delay: Duration) -> (u16, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0; 8192];
            let _ = stream.read(&mut buf).unwrap();
            std::thread::sleep(delay);
            let body = value.to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            // Drain until the client closes: closing with unread request bytes sends a
            // reset, which the client reports as no answer.
            let _ = stream.shutdown(std::net::Shutdown::Write);
            let _ = std::io::copy(&mut stream, &mut std::io::sink());
        });
        (port, thread)
    }

    #[test]
    fn ac2_search_errors_preserve_app_messages_and_name_app() {
        for (status, code) in [
            (400, "bad_request"),
            (503, "unavailable"),
            (500, "internal"),
            (403, "refused"),
        ] {
            let (port, thread) = stub(
                status,
                json!({"error":{"code":code,"message":"query rejected"}}),
                Duration::ZERO,
            );
            let error = client(port).search("reading", None, None).unwrap_err();
            thread.join().unwrap();
            assert_eq!(
                error.code,
                match status {
                    503 => Code::EnvHostNotRunning,
                    500 => Code::HostFailed,
                    _ => Code::HostRefused,
                }
            );
            assert_eq!(
                error.message,
                if status == 503 {
                    "the watane app of /home/test runs but could not open its data: query rejected"
                        .into()
                } else {
                    format!("[{code}] query rejected")
                }
            );
            assert_eq!(
                error.fix.as_deref(),
                Some(if status == 400 {
                    "Check --publisher (a 52-character key) and --interface (for example mods:reading-list/books or mods:reading-list/books@0.1.0)"
                } else if status == 503 {
                    "Ask the person to restart the watane app, then run: watane-dev cmd --json"
                } else if status == 500 {
                    "The watane app failed on this request; ask the person to read the watane app's log, then run: watane-dev cmd --json"
                } else {
                    "The watane app could not complete this command; ask the person to check the watane app, then run the command again"
                })
            );
        }
    }

    #[test]
    #[ignore = "timing: proves search stops waiting after its fixed 30-second timeout"]
    fn ac5_search_timeout_is_thirty_seconds() {
        let (port, thread) = stub(
            200,
            json!({"indexes":0,"results":[]}),
            Duration::from_millis(30_500),
        );
        let start = std::time::Instant::now();
        let error = client(port).search("", None, None).unwrap_err();
        let elapsed = start.elapsed();
        thread.join().unwrap();
        assert_eq!(error.code, Code::HostNoAnswer);
        assert!(elapsed >= Duration::from_secs(29), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(32), "{elapsed:?}");
        assert!(error.fix.is_some());
    }

    fn publish_package() -> watane_pkg::Package {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("wit")).unwrap();
        std::fs::create_dir(dir.path().join("ui")).unwrap();
        std::fs::write(
            dir.path().join("mod.toml"),
            "name = \"Demo\"\nsummary = \"a page\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("wit/world.wit"),
            "package mods:demo@0.2.3; world demo {}",
        )
        .unwrap();
        std::fs::write(dir.path().join("ui/index.html"), "demo").unwrap();
        watane_pkg::pack(dir.path(), false).unwrap()
    }

    #[test]
    fn ac1_releases_uses_get_without_origin() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            assert!(
                header.starts_with("GET /api/releases HTTP/1.1\r\n"),
                "{header}"
            );
            assert!(!header.to_lowercase().contains("origin:"));
            let body = r#"{"releases":[]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        assert_eq!(
            Client {
                port,
                home: PathBuf::from("/home/test"),
                rerun: None,
            }
            .releases()
            .unwrap(),
            json!({"releases":[]})
        );
        thread.join().unwrap();
    }

    #[test]
    fn ac1_releases_status_timeout_and_refused_errors() {
        let (port, thread) = stub(200, json!({"releases":[]}), Duration::from_millis(2200));
        assert_eq!(
            Client {
                port,
                home: PathBuf::from("/home/test"),
                rerun: None,
            }
            .releases()
            .unwrap_err()
            .code,
            Code::HostNoAnswer
        );
        thread.join().unwrap();
        for status in [200, 403, 500] {
            let (port, thread) = stub(
                status,
                json!({"error":{"code":"refused","message":"unavailable"}}),
                Duration::ZERO,
            );
            let e = client(port).releases().unwrap_err();
            assert_eq!(
                e.code,
                if status == 500 {
                    Code::HostFailed
                } else {
                    Code::HostRefused
                }
            );
            assert_eq!(e.message, "[refused] unavailable");
            assert_eq!(
                e.fix.as_deref(),
                Some(if status == 500 {
                    "The watane app failed on this request; ask the person to read the watane app's log, then run: watane-dev cmd --json"
                } else {
                    "The watane app could not complete this command; ask the person to check the watane app, then run the command again"
                })
            );
            thread.join().unwrap();
        }
    }

    #[test]
    fn ac2_publish_version_race() {
        let (port, thread) = stub(
            409,
            json!({"error":{"code":"version_exists","message":"release key/demo@0.2.3 already exists at seq 7"}}),
            Duration::ZERO,
        );
        let e = client(port)
            .publish_with_links(&publish_package(), Duration::from_secs(2), &[])
            .unwrap_err();
        thread.join().unwrap();
        assert_eq!(e.code.as_str(), "publish.version_exists");
        assert_eq!(e.code.class() as i32, 1);
        assert_eq!(e.message, "release key/demo@0.2.3 already exists at seq 7");
        assert_eq!(e.file.as_deref(), Some("wit/world.wit"));
        assert_eq!(
            e.fix.as_deref(),
            Some(
                "This version is already published. To publish changes, raise the version in wit/world.wit, then run publish again"
            )
        );
    }

    #[test]
    fn ac4_publish_request_and_answer() {
        let package = publish_package();
        let expected = package.as_bytes().to_vec();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            assert!(
                header.starts_with("POST /api/publish HTTP/1.1\r\n"),
                "{header}"
            );
            assert!(
                header
                    .to_lowercase()
                    .contains("content-type: application/octet-stream\r\n"),
                "{header}"
            );
            assert!(!header.to_lowercase().contains("origin:"));
            let mut body = vec![0; expected.len()];
            stream.read_exact(&mut body).unwrap();
            assert_eq!(body, expected);
            let body = r#"{"release":"key/demo@0.2.3","site":"/site","link":null}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let result = client(port)
            .publish_with_links(&package, Duration::from_secs(2), &[])
            .unwrap();
        thread.join().unwrap();
        assert_eq!(
            result,
            json!({"release":"key/demo@0.2.3","site":"/site","link":null})
        );
    }

    #[test]
    fn ac4_publish_timeout_and_host_errors() {
        let package = publish_package();
        for (status, value, delay, expected, message, fix) in [
            (
                403,
                json!({"error":{"code":"refused","message":"declined"}}),
                Duration::ZERO,
                Code::HostDeclined,
                "declined",
                "The person declined in the app. Ask them before you run publish again",
            ),
            (
                409,
                json!({"error":{"code":"conflict","message":"already published"}}),
                Duration::ZERO,
                Code::HostRefused,
                "[conflict] already published",
                "The watane app could not complete this command; ask the person to check the watane app, then run the command again",
            ),
            (
                200,
                json!({}),
                Duration::from_millis(1200),
                Code::HostNoAnswer,
                "no answer from the home host in 1 s",
                "Run the command again with a longer --wait",
            ),
        ] {
            let (port, thread) = stub(status, value, delay);
            let e = client(port)
                .publish_with_links(&package, Duration::from_secs(1), &[])
                .unwrap_err();
            thread.join().unwrap();
            assert_eq!(e.code, expected);
            assert_eq!(e.message, message);
            assert_eq!(e.fix.as_deref(), Some(fix));
        }
    }

    #[test]
    fn ac2_wrong_host_status() {
        let home = tempfile::tempdir().unwrap();
        for (mode, app, other, cause) in [
            ("preview", true, false, "is a preview"),
            ("home", false, false, "runs without the app"),
            ("home", true, true, "no watane app runs"),
        ] {
            // Bind first so the literal status has the actual loopback port.
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let value = json!({"watane":1,"mode":mode,"app":app,"port":port,
                "home":if other {json!("/another/home")} else {json!(home.path())}});
            let thread = std::thread::spawn(move || {
                let (mut s, _) = listener.accept().unwrap();
                let mut buf = [0; 8192];
                let _ = s.read(&mut buf).unwrap();
                let body = value.to_string();
                write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            });
            std::fs::write(
                home.path().join("run.json"),
                json!({"watane":1,"pid":1,"port":port,"mode":mode}).to_string(),
            )
            .unwrap();
            let e = Client::for_app(home.path(), Some("watane-dev cmd --json"))
                .err()
                .unwrap();
            assert_eq!(e.code, Code::EnvHostNotRunning);
            assert!(e.message.contains(cause), "{}", e.message);
            assert!(e.message.contains(home.path().to_str().unwrap()));
            assert_eq!(
                e.fix.as_deref(),
                Some("Start the watane app, then run: watane-dev cmd --json")
            );
            thread.join().unwrap();
        }
    }

    #[test]
    fn ac5_install_timeout_in_response_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0; 8192];
            let _ = stream.read(&mut buf).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: 16\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            stream.flush().unwrap();
            std::thread::sleep(Duration::from_millis(1200));
            let _ = stream.write_all(br#"{"installed":[]}"#);
        });
        let e = client(port).install_wait(&[], 1).unwrap_err();
        thread.join().unwrap();
        assert_eq!(e.code, Code::HostNoAnswer);
        assert_eq!(e.message, "no answer from the home host in 1 s");
    }

    /// The person said no in the app: there is no host error to correct, and running the
    /// command again asks them again. TECHNICAL 2.2 requires asking before retrying.
    #[test]
    fn regress_s12b_declined_fix_does_not_ask_to_correct_and_retry() {
        let (port, thread) = stub(
            403,
            json!({"error":{"code":"refused","message":"declined"}}),
            Duration::ZERO,
        );
        let e = client(port)
            .install_wait(&["/mods/counter".into()], 2)
            .unwrap_err();
        thread.join().unwrap();
        assert_eq!(e.code, Code::HostDeclined);
        assert_eq!(
            e.message,
            "The person declined installing counter in their watane app"
        );
        assert_eq!(
            e.fix.as_deref(),
            Some(
                "Ask the person whether they want counter installed in their watane app, then run install again with --wait <seconds>"
            )
        );
        assert!(!e.fix.as_deref().unwrap().to_lowercase().contains("correct"));
        assert_ne!(
            e.fix.as_deref(),
            Some(
                "The watane app could not complete this command; ask the person to check the watane app, then run the command again"
            ),
            "a declined install has nothing to correct; retrying asks the person again"
        );
    }

    #[test]
    fn ac5_install_timeout_and_host_errors() {
        let (port, thread) = stub(200, json!({"installed":[]}), Duration::from_millis(1200));
        let e = client(port).install_wait(&[], 1).unwrap_err();
        assert_eq!(e.code, Code::HostNoAnswer);
        assert_eq!(e.message, "no answer from the home host in 1 s");
        assert_eq!(
            e.fix.as_deref(),
            Some("Run the command again with a longer --wait")
        );
        thread.join().unwrap();
        for (status, code, expected, message) in [
            (
                403,
                "refused",
                Code::HostDeclined,
                "The person declined installing the mod in their watane app",
            ),
            (500, "broken", Code::HostFailed, "[broken] broken host"),
            (200, "broken", Code::HostRefused, "[broken] broken host"),
        ] {
            let (port, thread) = stub(
                status,
                json!({"error":{"code":code,"message":if code=="refused" {"declined"} else {"broken host"}}}),
                Duration::ZERO,
            );
            let e = client(port).install_wait(&[], 2).unwrap_err();
            assert_eq!(e.code, expected);
            assert_eq!(e.message, message);
            assert_eq!(
                e.fix.as_deref(),
                Some(if expected == Code::HostDeclined {
                    "Ask the person whether they want the mod installed in their watane app, then run install again with --wait <seconds>"
                } else if expected == Code::HostFailed {
                    "The watane app failed on this request; ask the person to read the watane app's log, then run: watane-dev cmd --json"
                } else {
                    "The watane app could not complete this command; ask the person to check the watane app, then run the command again"
                })
            );
            thread.join().unwrap();
        }
    }

    #[test]
    fn s86_fetch_without_an_answer_ends_in_the_rerun() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let e = client(port)
            .fetch(&format!("{}/demo@0.2.3", "y".repeat(52)))
            .unwrap_err();
        assert_eq!(e.code, Code::HostNoAnswer);
        assert_eq!(
            e.fix.as_deref(),
            Some("Check that the watane app is running, then run: watane-dev cmd --json")
        );
    }

    #[test]
    fn s86_for_app_without_a_rerun_keeps_the_older_fix() {
        let home = tempfile::tempdir().unwrap();
        let e = Client::for_app(home.path(), None).err().unwrap();
        assert_eq!(
            e.fix.as_deref(),
            Some("Start the watane app, then run this command again")
        );
        let e = Client::for_app(home.path(), Some("watane-dev x --json"))
            .err()
            .unwrap();
        assert_eq!(
            e.fix.as_deref(),
            Some("Start the watane app, then run: watane-dev x --json")
        );
    }
}
