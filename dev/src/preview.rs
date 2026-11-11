use crate::{
    api::Client,
    check,
    env::Env,
    outcome::{Code, Problem},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use watane_host::{Ready, RunInfo};
pub fn find_watane() -> Result<PathBuf, Problem> {
    let executable = |p: &Path| {
        fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    let missing = |where_: String| {
        Problem::new(
            Code::EnvWataneMissing,
            format!("the program watane was not found ({where_})"),
        )
        .fix("Install watane, or set WATANE_BIN to the program")
    };
    if let Some(p) = std::env::var_os("WATANE_BIN") {
        let p = PathBuf::from(p);
        return if executable(&p) {
            p.canonicalize().map_err(Problem::internal)
        } else {
            Err(missing(p.display().to_string()))
        };
    }
    if let Ok(exe) = std::env::current_exe() {
        let p = exe.with_file_name("watane");
        if executable(&p) {
            return Ok(p);
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            let p = path.join("watane");
            if executable(&p) {
                return p.canonicalize().map_err(Problem::internal);
            }
        }
    }
    Err(missing("beside watane-dev or on PATH".into()))
}
fn folder(dir: &Path, env: &Env, slug: &str) -> PathBuf {
    let hash = blake3::hash(dir.as_os_str().as_encoded_bytes()).to_hex();
    env.cache
        .join("previews")
        .join(format!("{slug}-{}", &hash[..8]))
}
fn start(home: &Path, folder: &Path, log: &Path) -> Result<Ready, Problem> {
    let binary = find_watane()?;
    fs::create_dir_all(folder).map_err(Problem::internal)?;
    let stderr = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(Problem::internal)?;
    let failure = |detail: String| {
        let tail = fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .rev()
            .take(20)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        Problem::new(
            Code::EnvPreviewStart,
            format!("the preview did not start: {detail}\n{tail}"),
        )
        .fix(format!("Read {}", log.display()))
    };
    let mut child = Command::new(binary)
        .arg("--preview")
        .arg(home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .process_group(0)
        .spawn()
        .map_err(|e| failure(e.to_string()))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
        let _ = tx.send(result);
    });
    let ready = rx
        .recv_timeout(Duration::from_secs(30))
        .map_err(|e| e.to_string())
        .and_then(|v| v.map_err(|e| e.to_string()))
        .and_then(|line| serde_json::from_str::<Ready>(&line).map_err(|e| format!("{e}: {line}")));
    let ready = match ready {
        Ok(r) if r.watane == 1 && r.mode == "preview" && r.home == home && r.port != 0 => r,
        Ok(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(failure("invalid ready line".into()));
        }
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(failure(e));
        }
    };
    // Reap this child if dev remains alive long enough; the host itself outlives dev.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    let path = folder.join("ready.json");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .map_err(Problem::internal)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(Problem::internal)?;
    file.write_all(&serde_json::to_vec(&ready).map_err(Problem::internal)?)
        .map_err(Problem::internal)?;
    Ok(ready)
}
fn running_preview(folder: &Path) -> Result<(Option<Client>, Option<Ready>), Problem> {
    let home = folder.join("home");
    let running = Client::for_home(&home)?;
    let cached = fs::read(folder.join("ready.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Ready>(&b).ok())
        .filter(|r| {
            running.is_some()
                && r.watane == 1
                && r.mode == "preview"
                && r.home == home
                && RunInfo::read(&home)
                    .ok()
                    .flatten()
                    .is_some_and(|run| run.port == r.port)
        });
    Ok((running, cached))
}

pub(crate) fn previous_warning(dir: &Path, env: &Env) -> Option<Problem> {
    preview_folders(dir, env).ok()?.iter().find_map(|folder| {
        let (_, ready) = running_preview(folder).ok()?;
        let ready = ready?;
        Some(
            Problem::new(
                Code::PreviewPrevious,
                format!(
                    "the preview at http://localhost:{}/api/ still serves the previous build",
                    ready.port
                ),
            )
            .fix("Fix the errors, then run: watane-dev serve --json"),
        )
    })
}

pub fn serve(dir: &Path, env: &Env) -> Result<Value, Vec<Problem>> {
    let (m, _) = check::check(dir)?;

    let work = || -> Result<Value, Problem> {
        let folder = folder(dir, env, m.slug().as_str());
        let home = folder.join("home");
        let log = folder.join("watane.log");
        let (running, cached) = running_preview(&folder)?;
        let reuse = cached.is_some();
        let ready = if let Some(ready) = cached {
            ready
        } else {
            if running.is_some() {
                stop_home(&home)?;
            }
            start(&home, &folder, &log)?
        };
        let client = Client::for_home(&home)?.ok_or_else(|| {
            Problem::new(
                Code::EnvPreviewStart,
                "the preview stopped before installation",
            )
            .fix(format!("Read {}", log.display()))
        })?;
        let installed = client.install(&[dir.to_path_buf()])?;
        let pages = installed
            .into_iter()
            .map(|m| json!({"slug":m.slug,"local":m.local,"url":m.page}))
            .collect::<Vec<_>>();
        // Construct loopback URLs from the host's port, never follow URLs supplied by files.
        Ok(
            json!({"home":home,"api":format!("http://localhost:{}/api/",ready.port),"reused":reuse,"pages":pages,"mcp":mcp(&ready),"log":log}),
        )
    };
    work().map_err(|e| vec![e])
}
fn mcp(ready: &Ready) -> Value {
    ready.token.as_ref().map_or(Value::Null, |token| {
        json!({"url":format!("http://localhost:{}/mcp/preview", ready.port),"toolset":"preview","token":token})
    })
}
fn stop_home(home: &Path) -> Result<bool, Problem> {
    if Client::for_home(home)?.is_none() {
        return Ok(false);
    }
    let Some(run) = RunInfo::read(home).ok().flatten() else {
        return Ok(false);
    };
    let status = Command::new("/bin/kill")
        .arg("-TERM")
        .arg(run.pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(Problem::internal)?;
    if !status.success() {
        return Err(Problem::internal("could not stop the preview"));
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while home.join("run.json").exists() {
        let alive = Command::new("/bin/kill")
            .arg("-0")
            .arg(run.pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(Problem::internal)?;
        if !alive.success() {
            break;
        }
        if Instant::now() >= deadline {
            return Err(
                Problem::new(Code::EnvPreviewStart, "preview did not stop in 10 seconds")
                    .fix("Read the preview watane.log and stop the host before trying again"),
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(true)
}
fn preview_folders(dir: &Path, env: &Env) -> Result<Vec<PathBuf>, Problem> {
    let hash = blake3::hash(dir.as_os_str().as_encoded_bytes()).to_hex();
    let suffix = format!("-{}", &hash[..8]);
    let entries = match fs::read_dir(env.cache.join("previews")) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(vec![]);
        }
        Err(e) => return Err(Problem::internal(e)),
    };
    let mut folders = Vec::new();
    for entry in entries {
        let entry = entry.map_err(Problem::internal)?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.ends_with(&suffix))
            && entry.file_type().map_err(Problem::internal)?.is_dir()
        {
            folders.push(entry.path());
        }
    }
    folders.sort();
    Ok(folders)
}

pub fn stop(dir: &Path, env: &Env) -> Result<Value, Vec<Problem>> {
    let work = || -> Result<Value, Problem> {
        // A deleted mod folder keeps its absolute path, so its preview can still stop.
        let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let folders = preview_folders(&dir, env)?;
        let mut home = None;
        let mut stopped = false;
        for folder in folders {
            let preview_home = folder.join("home");
            stopped |= stop_home(&preview_home)?;
            home.get_or_insert(preview_home);
            fs::remove_dir_all(&folder).map_err(Problem::internal)?;
        }
        Ok(json!({"stopped":stopped,"home":home}))
    };
    work().map_err(|e| vec![e])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac4_mcp_from_literal_ready() {
        let mut ready = Ready {
            watane: 1,
            mode: "preview".into(),
            home: "/tmp/preview".into(),
            port: 52011,
            api: "untrusted".into(),
            mcp: Some("http://elsewhere/".into()),
            token: Some("wt_x".into()),
            mods: vec![],
        };
        assert_eq!(
            json!({"mcp": mcp(&ready)}),
            json!({"mcp": {
                "url": "http://localhost:52011/mcp/preview", "toolset": "preview", "token": "wt_x"
            }})
        );
        ready.token = None;
        assert_eq!(json!({"mcp": mcp(&ready)}), json!({"mcp": null}));
    }
}
