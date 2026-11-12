use crate::outcome::{Code, Problem};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use watane_host::{InstalledMod, RunInfo};
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
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(None)
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .max_redirects(0)
            .build()
            .into();
        let url = format!("http://localhost:{}/api/{path}", self.port);
        let response = if let Some((content_type, body)) = body {
            agent
                .post(&url)
                .header("Content-Type", content_type)
                .send(body)
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
                .fix("Raise the version in wit/world.wit, then run publish again"));
            }
            let code = if path != "releases" && value["error"]["code"] == "refused" {
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
            .fix(if code == Code::HostDeclined {
                if path == "publish" {
                    "The person declined in the app. Ask them before you run publish again"
                } else {
                    "The person declined in the app. Ask them before you run install again"
                }
            } else {
                "Correct the reported host error and run the command again"
            }));
        }
        Ok(value)
    }
    pub fn releases(&self) -> Result<Value, Problem> {
        self.request("releases", None, Duration::from_secs(2))
    }

    pub fn publish(&self, package: &watane_pkg::Package, wait: Duration) -> Result<Value, Problem> {
        self.request(
            "publish",
            Some(("application/octet-stream", package.as_bytes())),
            wait,
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
            Some("Raise the version in wit/world.wit, then run publish again")
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
                "Correct the reported host error and run the command again",
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
            Some("Correct the reported host error and run the command again"),
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
                    "Correct the reported host error and run the command again"
                })
            );
            thread.join().unwrap();
        }
    }
}
