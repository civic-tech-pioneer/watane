use crate::outcome::{Code, Problem};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use watane_host::{InstalledMod, RunInfo};
const FETCH_FIX: &str =
    "Pass a mod link watane:<key>@<url>#<slug> or a release <key>/<slug>@<version>";
pub struct Client {
    port: u16,
}
impl Client {
    pub fn for_app(home: &Path) -> Result<Self, Problem> {
        let missing = |message| {
            Problem::new(Code::EnvHostNotRunning, message)
                .fix("Start the watane app, then run this command again")
        };
        let absent = || {
            missing(format!(
                "no watane app runs for the home {}",
                home.display()
            ))
        };
        let run = RunInfo::read(home).ok().flatten().ok_or_else(absent)?;
        let client = Self { port: run.port };
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
        let status = Self { port: run.port }
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
        let client = Self { port: run.port };
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
                    .fix("Check the watane app, then run this command again")
            })?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let value: Value = response.body_mut().read_json().map_err(Problem::internal)?;
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
                FETCH_FIX
            } else {
                "Check the watane app, then run this command again"
            }));
        }
        let release: ReleaseRef = response
            .headers()
            .get("Watane-Release")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| Problem::internal("missing Watane-Release"))?
            .parse()
            .map_err(Problem::internal)?;
        let id: PackageId = response
            .headers()
            .get("Watane-Package")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| Problem::internal("missing Watane-Package"))?
            .parse()
            .map_err(Problem::internal)?;
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
            return Err(Problem::internal(
                "Watane-Release does not match the requested release",
            ));
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
        let package = Package::from_bytes(bytes).map_err(Problem::internal)?;
        if package.id() != id {
            return Err(Problem::internal(
                "Watane-Package does not match the package body",
            ));
        }
        let world = package
            .file("wit/world.wit")
            .ok_or_else(|| Problem::internal("package has no wit/world.wit"))?;
        let (_, slug, version) =
            crate::build_on::declaration(std::str::from_utf8(world).map_err(Problem::internal)?)
                .map_err(|e| Problem::internal(e.message))?;
        if slug != release.slug || version != release.version {
            return Err(Problem::internal(
                "Watane-Release does not match the package body",
            ));
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

    pub fn publish(&self, package: &watane_pkg::Package, wait: Duration) -> Result<Value, Problem> {
        self.publish_with_links(package, wait, &[])
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
                                        e.fix = Some(format!("Install {release} in the watane app (from its link), then run install again"));
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
        let error = Client { port }.install_wait(&[dir], 1).unwrap_err();
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
    fn ac8_fetch_refuses_corrupt_headers_bytes_and_app_errors() {
        fn binary(
            status: u16,
            release: &str,
            id: &str,
            body: Vec<u8>,
        ) -> (u16, std::thread::JoinHandle<()>) {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let headers = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nWatane-Release: {release}\r\nWatane-Package: {id}\r\nConnection: close\r\n\r\n",
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
                assert!(
                    serde_json::from_slice::<Value>(&request)
                        .unwrap()
                        .get("release")
                        .is_some()
                );
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
                Code::Internal,
            ),
            (
                "nonsense".into(),
                p.id().to_string(),
                p.as_bytes().to_vec(),
                Code::Internal,
            ),
            (
                format!("{}/other@0.2.3", "y".repeat(52)),
                p.id().to_string(),
                p.as_bytes().to_vec(),
                Code::Internal,
            ),
            (
                asked.clone(),
                p.id().to_string(),
                b"not a package".to_vec(),
                Code::Internal,
            ),
            (
                asked.clone(),
                p.id().to_string(),
                vec![0; watane_pkg::MAX_PACKAGE_BYTES as usize + 1],
                Code::HostRefused,
            ),
        ] {
            let (port, thread) = binary(200, &release, &id, bytes);
            let error = Client { port }
                .fetch(&asked)
                .expect_err("reject corrupt response");
            thread.join().unwrap();
            assert_eq!(error.code, expected, "{error:?}");
        }
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
            let error = Client { port }.fetch(&asked).err().unwrap();
            thread.join().unwrap();
            assert_eq!(error.code, Code::HostRefused);
            assert_eq!(error.message, format!("[{code}] cannot fetch"));
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
        let errors =
            crate::build_on::fetch_providers(&consumer, &Client { port: 0 }, &env).unwrap_err();
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
            &Client { port: 0 },
            &env,
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
            let error = Client { port }.search("reading", None, None).unwrap_err();
            thread.join().unwrap();
            assert_eq!(error.code, Code::HostRefused);
            assert_eq!(error.message, format!("[{code}] query rejected"));
            assert_eq!(
                error.fix.as_deref(),
                Some(if status == 400 {
                    "Check --publisher (a 52-character key) and --interface (for example mods:reading-list/books or mods:reading-list/books@0.1.0)"
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
        let error = Client { port }.search("", None, None).unwrap_err();
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
        assert_eq!(Client { port }.releases().unwrap(), json!({"releases":[]}));
        thread.join().unwrap();
    }

    #[test]
    fn ac1_releases_status_timeout_and_refused_errors() {
        let (port, thread) = stub(200, json!({"releases":[]}), Duration::from_millis(2200));
        assert_eq!(
            Client { port }.releases().unwrap_err().code,
            Code::HostNoAnswer
        );
        thread.join().unwrap();
        for status in [200, 403, 500] {
            let (port, thread) = stub(
                status,
                json!({"error":{"code":"refused","message":"unavailable"}}),
                Duration::ZERO,
            );
            let e = Client { port }.releases().unwrap_err();
            assert_eq!(e.code, Code::HostRefused);
            assert_eq!(e.message, "[refused] unavailable");
            assert_eq!(
                e.fix.as_deref(),
                Some(
                    "The watane app could not complete this command; ask the person to check the watane app, then run the command again"
                )
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
        let e = Client { port }
            .publish(&publish_package(), Duration::from_secs(2))
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
        let result = Client { port }
            .publish(&package, Duration::from_secs(2))
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
            let e = Client { port }
                .publish(&package, Duration::from_secs(1))
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
            let e = Client::for_app(home.path()).err().unwrap();
            assert_eq!(e.code, Code::EnvHostNotRunning);
            assert!(e.message.contains(cause), "{}", e.message);
            assert!(e.message.contains(home.path().to_str().unwrap()));
            assert_eq!(
                e.fix.as_deref(),
                Some("Start the watane app, then run this command again")
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
        let e = Client { port }.install_wait(&[], 1).unwrap_err();
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
        let e = Client { port }
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
        let e = Client { port }.install_wait(&[], 1).unwrap_err();
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
            (500, "broken", Code::HostRefused, "[broken] broken host"),
            (200, "broken", Code::HostRefused, "[broken] broken host"),
        ] {
            let (port, thread) = stub(
                status,
                json!({"error":{"code":code,"message":if code=="refused" {"declined"} else {"broken host"}}}),
                Duration::ZERO,
            );
            let e = Client { port }.install_wait(&[], 2).unwrap_err();
            assert_eq!(e.code, expected);
            assert_eq!(e.message, message);
            assert_eq!(
                e.fix.as_deref(),
                Some(if expected == Code::HostDeclined {
                    "Ask the person whether they want the mod installed in their watane app, then run install again with --wait <seconds>"
                } else {
                    "The watane app could not complete this command; ask the person to check the watane app, then run the command again"
                })
            );
            thread.join().unwrap();
        }
    }
}
