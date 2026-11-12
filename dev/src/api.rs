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
        body: Option<Value>,
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
        let response = if let Some(body) = body {
            agent.post(&url).send_json(body)
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
            let code = if value["error"]["code"] == "refused" {
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
            .fix("Correct the reported host error and run the command again"));
        }
        Ok(value)
    }
    pub fn install_wait(
        &self,
        folders: &[PathBuf],
        wait: u64,
    ) -> Result<Vec<InstalledMod>, Problem> {
        self.install_timeout(folders, Duration::from_secs(wait))
            .map_err(|mut e| {
                if e.code == Code::HostNoAnswer {
                    e.message = format!("no answer from the home host in {wait} s");
                    e.fix = Some("Run the command again with a longer --wait".into());
                }
                e
            })
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
        let value = self.request("install", Some(json!({"folders":folders})), timeout)?;
        serde_json::from_value::<Installed>(value)
            .map(|v| v.installed)
            .map_err(Problem::internal)
    }
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
        });
        (port, thread)
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
            (403, "refused", Code::HostDeclined, "declined"),
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
            thread.join().unwrap();
        }
    }
}
