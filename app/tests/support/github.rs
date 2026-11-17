//! Opt-in GitHub journeys. Nothing in this module contacts GitHub on import.
#![allow(clippy::unwrap_used, reason = "tests may panic")]

pub fn redact(token: &str, text: &str) -> String {
    text.replace(token, "[token]")
}

#[cfg(feature = "real-github")]
mod real {
    use super::super::dev;
    use super::super::driver::App;
    use super::redact;
    use serde_json::Value;
    use std::fs;
    use std::path::Path;
    use std::process::{Command, Output};
    use std::time::{Duration, Instant};

    pub const PREFIX: &str = "minmax-fund/watane-j-";

    pub struct RealGithub {
        token: String,
        config: tempfile::TempDir,
        pub(crate) repos: Vec<String>,
    }

    impl RealGithub {
        pub fn new() -> Self {
            let token = std::env::var("CLAUDE_GH_TOKEN").unwrap_or_else(|_| {
                panic!("CLAUDE_GH_TOKEN is not set (real GitHub tests need it)")
            });
            assert!(!token.is_empty(), "CLAUDE_GH_TOKEN is empty");
            Self::with_token(token)
        }

        pub fn with_token(token: String) -> Self {
            let config = tempfile::tempdir().unwrap();
            fs::create_dir(config.path().join("gh")).unwrap();
            fs::write(config.path().join("gitconfig"),
                "[user]\n name = Watane Journey\n email = journey@example.test\n[credential \"https://github.com\"]\n helper = \n helper = !gh auth git-credential\n").unwrap();
            Self {
                token,
                config,
                repos: Vec::new(),
            }
        }

        pub fn env(&self, command: &mut Command) {
            command
                .env_remove("CLAUDE_GH_TOKEN")
                .env("GH_TOKEN", &self.token)
                .env("GH_CONFIG_DIR", self.config.path().join("gh"))
                .env("GH_PROMPT_DISABLED", "1")
                .env("GIT_TERMINAL_PROMPT", "0")
                .env("GIT_CONFIG_GLOBAL", self.config.path().join("gitconfig"))
                .env("GIT_CONFIG_NOSYSTEM", "1");
        }

        fn run(&self, command: &mut Command) -> Output {
            self.env(command);
            command
                .output()
                .unwrap_or_else(|e| panic!("{}", redact(&self.token, &e.to_string())))
        }

        pub fn diagnostic(&self, output: &Output) -> String {
            redact(
                &self.token,
                &format!(
                    "status {}; stdout: {}; stderr: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
            )
        }

        pub fn reserve(&mut self) -> String {
            // Rejection sampling avoids bias and guarantees the first character is a letter.
            let mut suffix = String::new();
            while suffix.len() < 10 {
                let mut byte = [0];
                getrandom::fill(&mut byte).unwrap();
                let alphabet = if suffix.is_empty() {
                    &b"abcdefghijklmnopqrstuvwxyz"[..]
                } else {
                    &b"abcdefghijklmnopqrstuvwxyz0123456789"[..]
                };
                if (byte[0] as usize) < 256 / alphabet.len() * alphabet.len() {
                    suffix.push(alphabet[byte[0] as usize % alphabet.len()] as char);
                }
            }
            let name = format!("{PREFIX}{suffix}");
            let check =
                self.run(Command::new("gh").args(["api", "--include", &format!("repos/{name}")]));
            let missing = String::from_utf8_lossy(&check.stdout).lines().any(|line| {
                line.starts_with("HTTP/") && line.split_whitespace().nth(1) == Some("404")
            });
            assert!(
                !check.status.success() && missing,
                "repository preflight: {}",
                self.diagnostic(&check)
            );
            self.repos.push(name.clone());
            name
        }

        pub fn create(&mut self) -> String {
            let name = self.reserve();
            let created = self.run(Command::new("gh").args(["repo", "create", &name, "--public"]));
            assert!(
                created.status.success(),
                "create: {}",
                self.diagnostic(&created)
            );
            name
        }

        pub fn pages(&self, name: &str) -> String {
            assert!(
                name.starts_with(PREFIX),
                "Pages repository outside journey prefix"
            );
            assert!(
                self.repos.iter().any(|repo| repo == name),
                "Pages repository was not reserved"
            );
            let output = self.run(Command::new("gh").args([
                "api",
                "--method",
                "POST",
                &format!("repos/{name}/pages"),
                "-f",
                "source[branch]=main",
                "-f",
                "source[path]=/",
            ]));
            assert!(
                output.status.success(),
                "Pages: {}",
                self.diagnostic(&output)
            );
            let answer = self.json(&output);
            let url = answer["html_url"].as_str().expect("Pages html_url");
            assert!(url.starts_with("https://"), "Pages URL must use HTTPS");
            format!("{}/", url.trim_end_matches('/'))
        }

        pub fn wait_served(&self, url: &str, expect: &[u8], deadline: Duration) {
            self.wait_pages(url, 200, Some(expect), deadline);
        }

        pub fn wait_status(&self, url: &str, status: u16, deadline: Duration) {
            self.wait_pages(url, status, None, deadline);
        }

        fn wait_pages(
            &self,
            url: &str,
            expected_status: u16,
            expect: Option<&[u8]>,
            deadline: Duration,
        ) {
            assert!(url.starts_with("https://"), "Pages fetch must use HTTPS");
            let client: ureq::Agent = ureq::Agent::config_builder()
                .https_only(true)
                .http_status_as_error(false)
                .build()
                .into();
            let started = Instant::now();
            let mut last_status = None;
            let mut last_error = None;
            while let Some(remaining) = deadline.checked_sub(started.elapsed()) {
                if remaining.is_zero() {
                    break;
                }
                match client
                    .get(url)
                    .config()
                    .timeout_global(Some(remaining.min(Duration::from_secs(10))))
                    .build()
                    .call()
                {
                    Ok(mut response) => {
                        let status = response.status();
                        last_status = Some(status);
                        if status.as_u16() == expected_status {
                            let Some(expect) = expect else { return };
                            match response.body_mut().read_to_vec() {
                                Ok(body) if body == expect => return,
                                Ok(_) => {}
                                Err(error) => {
                                    last_error = Some(redact(&self.token, &error.to_string()))
                                }
                            }
                        }
                    }
                    Err(error) => last_error = Some(redact(&self.token, &error.to_string())),
                }
                std::thread::sleep(
                    Duration::from_secs(10).min(deadline.saturating_sub(started.elapsed())),
                );
            }
            panic!(
                "Pages did not serve {} within {deadline:?}; last status: {last_status:?}; last error: {last_error:?}; waited {:?}",
                redact(&self.token, url),
                started.elapsed()
            );
        }

        pub fn dev_command(&self, dev: &Path, app: &App, dir: &Path, args: &[&str]) -> Command {
            let mut command = dev::dev_command(dev, app, dir, args);
            self.env(&mut command);
            command
        }

        pub fn git(&self, args: &[&str]) -> String {
            let output = self.run(Command::new("git").args(args));
            assert!(output.status.success(), "git: {}", self.diagnostic(&output));
            redact(&self.token, &String::from_utf8_lossy(&output.stdout))
        }

        pub fn json(&self, output: &Output) -> Value {
            serde_json::from_str(&redact(
                &self.token,
                &String::from_utf8_lossy(&output.stdout),
            ))
            .unwrap_or_else(|e| {
                panic!(
                    "JSON: {}; {}",
                    redact(&self.token, &e.to_string()),
                    self.diagnostic(output)
                )
            })
        }

        pub fn site_origin(&self, home: &Path, name: &str) {
            let site = home.join("dist/site");
            fs::create_dir_all(&site).unwrap();
            let dir = site.to_str().unwrap();
            self.git(&["-C", dir, "init", "-q", "-b", "main"]);
            fs::write(site.join(".git/info/exclude"), ".*\n").unwrap();
            self.git(&[
                "-C",
                dir,
                "remote",
                "add",
                "origin",
                &format!("https://github.com/{name}"),
            ]);
        }

        pub fn repository(&self, dir: &Path, url: &str) {
            let path = dir.join("mod.toml");
            let manifest = fs::read_to_string(&path).unwrap();
            fs::write(path, format!("repository = \"{url}\"\n{manifest}")).unwrap();
        }
    }

    impl Drop for RealGithub {
        fn drop(&mut self) {
            for name in &self.repos {
                assert!(name.starts_with(PREFIX));
                let mut command = Command::new("gh");
                self.env(&mut command);
                match command
                    .args([
                        "api",
                        "--include",
                        "--method",
                        "DELETE",
                        &format!("repos/{name}"),
                    ])
                    .output()
                {
                    Ok(output) if output.status.success() => {}
                    Ok(output)
                        if String::from_utf8_lossy(&output.stdout).lines().any(|line| {
                            line.starts_with("HTTP/")
                                && line.split_whitespace().nth(1) == Some("404")
                        }) => {}
                    Ok(output) => eprintln!(
                        "{}: {}",
                        redact(&self.token, name),
                        self.diagnostic(&output)
                    ),
                    Err(error) => eprintln!(
                        "{}: {}",
                        redact(&self.token, name),
                        redact(&self.token, &error.to_string())
                    ),
                }
            }
        }
    }
}

#[cfg(feature = "real-github")]
#[allow(unused_imports, reason = "only some test binaries use this re-export")]
pub use real::{PREFIX, RealGithub};
