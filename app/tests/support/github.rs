//! Opt-in GitHub journeys. Nothing in this module contacts GitHub on import.
pub fn redact(token: &str, text: &str) -> String {
    text.replace(token, "[token]")
}

#[cfg(feature = "real-github")]
mod real {
    use super::super::{dev, driver::App};
    use super::redact;
    use serde_json::Value;
    use std::{
        fs,
        path::Path,
        process::{Command, Output},
    };

    const PREFIX: &str = "minmax-fund/watane-j-";

    pub struct RealGithub {
        token: String,
        config: tempfile::TempDir,
        repos: Vec<String>,
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

        pub fn create(&mut self) -> String {
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
            let created = self.run(Command::new("gh").args(["repo", "create", &name, "--public"]));
            assert!(
                created.status.success(),
                "create: {}",
                self.diagnostic(&created)
            );
            self.repos.push(name.clone());
            name
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
                match command.args(["repo", "delete", name, "--yes"]).output() {
                    Ok(output) if output.status.success() => {}
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
#[allow(unused_imports)]
pub use real::RealGithub;
