use super::driver::App;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

// Build the maker program, never a mod. Read Cargo's artifact message so a
// custom target directory still selects this checkout's binary, not PATH.
pub fn workspace_dev() -> PathBuf {
    workspace_binary("watane-dev", &[])
}

pub fn workspace_fake_gh() -> PathBuf {
    workspace_binary("fake-gh", &["--features", "fake-host"])
}

fn workspace_binary(name: &str, extra: &[&str]) -> PathBuf {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "build",
            "-p",
            "watane-dev",
            "--bin",
            name,
            "--message-format=json",
        ])
        .args(extra)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "building workspace {name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|v| {
            (v["reason"] == "compiler-artifact" && v["target"]["name"] == name)
                .then(|| v["executable"].as_str().map(PathBuf::from))
                .flatten()
        })
        .unwrap_or_else(|| panic!("Cargo did not report the workspace {name} executable"))
}
pub fn dev_command(dev: &Path, app: &App, dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(dev);
    command
        .current_dir(dir.parent().unwrap())
        .args(args)
        .arg("--json")
        .arg("--home")
        .arg(&app.home)
        .arg("--dir")
        .arg(dir)
        .env("WATANE_DEV_CACHE", app.home.join("dev-cache"))
        .env("NO_PROXY", "*")
        .env("no_proxy", "*");
    // dev discovers this app's ephemeral port through <home>/run.json.
    command
}

pub fn dev_run(dev: &Path, app: &App, dir: &Path, args: &[&str]) -> Value {
    let output = dev_command(dev, app, dir, args).output().unwrap();
    let value: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("watane-dev {args:?}: {e}; {output:?}"));
    assert!(
        output.status.success(),
        "watane-dev {args:?}: {value}; {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(value["ok"], true, "{value}");
    value["result"].clone()
}

// Local GitHub stand-in shared by the step-6 app journeys. Every child gets
// its own environment; concurrent participants never change the test process.
pub struct Github {
    root: PathBuf,
    path: std::ffi::OsString,
}
impl Github {
    pub fn new(root: &Path, fake_gh: &Path) -> Self {
        std::fs::create_dir_all(root.join("remotes")).unwrap();
        std::fs::create_dir(root.join("bin")).unwrap();
        std::os::unix::fs::symlink(fake_gh, root.join("bin/gh")).unwrap();
        std::fs::write(root.join("gitconfig"), format!(
            "[user]\n name = Alice\n email = alice@example.test\n[url \"file://{}/remotes/\"]\n insteadOf = https://github.com/\n", root.display()
        )).unwrap();
        let path = std::env::join_paths(
            std::iter::once(root.join("bin"))
                .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
        )
        .unwrap();
        Self {
            root: root.into(),
            path,
        }
    }

    fn git_env(&self, command: &mut Command) {
        command
            .env("GIT_CONFIG_GLOBAL", self.root.join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1");
    }

    pub fn dev_command(
        &self,
        dev: &Path,
        app: &App,
        dir: &Path,
        user: &str,
        args: &[&str],
    ) -> Command {
        let mut command = dev_command(dev, app, dir, args);
        self.git_env(&mut command);
        command
            .env("PATH", &self.path)
            .env("FAKE_GH_ROOT", self.root.join("remotes"))
            .env("FAKE_GH_USER", user);
        command
    }

    pub fn git(&self, args: &[&str]) -> String {
        let mut command = Command::new("git");
        self.git_env(&mut command);
        let output = command.args(args).output().unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}
