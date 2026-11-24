//! Unix preview process groups and state (TECHNICAL 9.1, 9.3, 9.4).

use super::{find_fake_claude, find_watane};
use crate::api::Client;
use crate::env::Env;
use crate::outcome::{Code, Problem};
use crate::{check, deps};
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use watane_host::{Ready, RunInfo};

#[path = "openai.rs"]
mod openai;
use openai::OpenAi;
const DEFAULT_SCRIPT: &str = r#"{"steps":[{"do":"expect-user"},{"do":"delta","text":"This is the fake claude."},{"do":"message","text":"This is the fake claude."},{"do":"result"},{"do":"idle"}]}"#;
/// Links the fake into `<P>/claude/` (a copy where links fail), keeps the maker's script (TECHNICAL 18.4).
fn link_fake_claude(folder: &Path, program: &Path) -> Result<PathBuf, Problem> {
    let claude = folder.join("claude");
    fs::create_dir_all(&claude).map_err(Problem::internal)?;
    let link = claude.join("fake-claude");
    place(program, &link, |p, l| std::os::unix::fs::symlink(p, l))?;
    let script = claude.join("fake-claude.json");
    if !script.exists() {
        fs::write(&script, DEFAULT_SCRIPT).map_err(Problem::internal)?;
    }
    Ok(link)
}
/// `link` as a link to `program`, or a copy where `symlink` fails.
fn place(
    program: &Path,
    link: &Path,
    symlink: fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), Problem> {
    let _ = fs::remove_file(link);
    if symlink(program, link).is_err() {
        fs::copy(program, link).map_err(Problem::internal)?;
    }
    Ok(())
}
fn folder(dir: &Path, env: &Env, slug: &str) -> PathBuf {
    let hash = blake3::hash(dir.as_os_str().as_encoded_bytes()).to_hex();
    env.cache
        .join("previews")
        .join(format!("{slug}-{}", &hash[..8]))
}
/// The arguments of the preview host: `--fake-claude` after `--preview` when the fake was linked.
fn spawn_args(home: &Path, fake_claude: Option<&Path>) -> Vec<std::ffi::OsString> {
    let mut args = vec!["--preview".into(), home.into()];
    if let Some(fake) = fake_claude {
        args.extend(["--fake-claude".into(), fake.into()]);
    }
    args
}
fn start(
    home: &Path,
    folder: &Path,
    log: &Path,
    fake_claude: Option<&Path>,
    openai: Option<&OpenAi>,
) -> Result<Ready, Problem> {
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
    let mut command = Command::new(binary);
    command.args(spawn_args(home, fake_claude));
    let _snapshot = match openai {
        Some(openai) => openai.configure(&mut command, folder)?,
        None => None,
    };
    let mut child = command
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
            let problem = failure(e);
            return Err(match openai {
                Some(openai) => openai.start_failure(problem),
                None => problem,
            });
        }
    };
    if let Err(error) =
        OpenAi::remember(openai, folder, ready.port).and_then(|()| save_ready(folder, &ready))
    {
        let _ = stop_home(home);
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    // Reap this child if dev remains alive long enough; the host itself outlives dev.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(ready)
}
fn save_ready(folder: &Path, ready: &Ready) -> Result<(), Problem> {
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
    Ok(())
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

pub(crate) fn previous_warning(dir: &Path, env: &Env, args: &str) -> Option<Problem> {
    preview_folders(dir, env).ok()?.iter().find_map(|folder| {
        let (_, ready) = running_preview(folder).ok()?;
        let ready = ready?;
        Some(
            Problem::new(
                Code::PreviewPrevious,
                format!(
                    "the preview at {} still serves the previous build",
                    previous_url(&ready)
                ),
            )
            .fix(format!(
                "Fix the errors, then run: watane-dev serve --json{args}"
            )),
        )
    })
}

pub(crate) fn serve_with_script(
    dir: &Path,
    env: &Env,
    rerun: &str,
    script: Option<&Path>,
) -> Result<Value, Vec<Problem>> {
    let (m, _) = check::check(dir)?;
    let folders = deps::local_order(dir).map_err(|e| vec![e])?;

    let work = || -> Result<Value, Problem> {
        let folder = folder(dir, env, m.slug().as_str());
        let home = folder.join("home");
        let log = folder.join("watane.log");
        let openai = OpenAi::select(dir, &m, &folders, script)?;
        let (running, cached) = running_preview(&folder)?;
        let cached = cached.filter(|ready| OpenAi::matches(openai.as_ref(), &folder, ready.port));
        let fake_claude = match find_fake_claude() {
            Ok(program) => Some(link_fake_claude(&folder, &program)?),
            Err(_) => None,
        };
        let reuse = cached.is_some();
        let mut ready = if let Some(ready) = cached {
            ready
        } else {
            if running.is_some() {
                stop_home(&home)?;
            }
            start(
                &home,
                &folder,
                &log,
                fake_claude.as_deref(),
                openai.as_ref(),
            )?
        };
        let client = Client::for_home(&home)?
            .ok_or_else(|| {
                Problem::new(
                    Code::EnvPreviewStart,
                    "the preview stopped before installation",
                )
                .fix(format!("Read {}", log.display()))
            })?
            .with_rerun(rerun);
        let installed = client.install(&folders)?;
        ready.mods = installed
            .iter()
            .map(|m| watane_host::ReadyMod {
                local: m.local.clone(),
                page: m.page.clone(),
            })
            .collect();
        save_ready(&folder, &ready)?;
        let pages = installed
            .into_iter()
            .map(|m| json!({"slug":m.slug,"local":m.local,"url":m.page}))
            .collect::<Vec<_>>();
        // Construct loopback URLs from the host's port, never follow URLs supplied by files.
        Ok(
            json!({"home":home,"api":api_url(&ready),"reused":reuse,"pages":pages,"mcp":mcp(&ready),"log":log}),
        )
    };
    work().map_err(|e| vec![e])
}
fn api_url(ready: &Ready) -> String {
    format!("http://localhost:{}/api/", ready.port)
}
fn previous_url(ready: &Ready) -> String {
    ready.mods.last().filter(|m| m.page.is_some()).map_or_else(
        || api_url(ready),
        |m| format!("http://{}.localhost:{}/", m.local, ready.port),
    )
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
    fn ac6_serve_posts_all_folders_once_in_provider_order() {
        use std::io::Read;
        use std::net::TcpListener;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        for slug in ["a", "b", "c"] {
            crate::template::new_mod(&root.join(slug), slug).unwrap();
        }
        for (consumer, provider) in [("b", "a"), ("c", "b")] {
            let dir = root.join(consumer);
            deps::add_local(&dir, &root.join(provider)).unwrap();
            let world = fs::read_to_string(dir.join("wit/world.wit")).unwrap();
            fs::write(
                dir.join("wit/world.wit"),
                world.replace(
                    &format!("world {consumer} {{"),
                    &format!("world {consumer} {{\n    import mods:{provider}/api@0.1.0;"),
                ),
            )
            .unwrap();
        }
        let dir = root.join("c");
        let env = Env {
            cache: root.join("cache"),
            unit_seconds: 10,
        };
        let hash = blake3::hash(dir.as_os_str().as_encoded_bytes()).to_hex();
        let preview = env.cache.join(format!("previews/c-{}", &hash[..8]));
        let home = preview.join("home");
        fs::create_dir_all(&home).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        fs::write(
            home.join("run.json"),
            json!({"watane":1,"mode":"preview","pid":std::process::id(),"port":port}).to_string(),
        )
        .unwrap();
        fs::write(preview.join("ready.json"), json!({"watane":1,"mode":"preview","home":home,"port":port,"api":"untrusted","mcp":null,"token":null,"mods":[]}).to_string()).unwrap();
        let expected = json!({"folders":[root.join("a"),root.join("b"),root.join("c")]});
        let server = std::thread::spawn(move || {
            for request in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                let header = String::from_utf8(header).unwrap();
                let body = if request < 2 {
                    assert!(header.starts_with("GET /api/status "));
                    json!({"watane":1,"mode":"preview","home":home,"port":port})
                } else {
                    assert!(header.starts_with("POST /api/install "));
                    let length: usize = header
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|n| n.trim().parse().unwrap())
                        })
                        .unwrap();
                    let mut bytes = vec![0; length];
                    stream.read_exact(&mut bytes).unwrap();
                    assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), expected);
                    let installed = ["a", "b", "c"].map(|slug| json!({
                        "local":slug, "identity":format!("local/{slug}"), "slug":slug,
                        "name":slug, "summary":"", "version":"0.1.0", "publisher":null,
                        "package":"0000000000000000000000000000000000000000000000000000000000000000",
                        "release":null, "origin":null, "enabled":true, "has_logic":false,
                        "page":format!("http://{slug}.localhost:{port}/"), "grants":[], "links":[], "error":null
                    }));
                    json!({"installed":installed})
                }
                .to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let result = crate::preview::serve(&dir, &env, "watane-dev serve --json").unwrap();
        assert_eq!(result["reused"], true);
        assert_eq!(
            result["pages"],
            json!(["a", "b", "c"].map(|slug| json!({
                "slug":slug, "local":slug, "url":format!("http://{slug}.localhost:{port}/")
            })))
        );
        let cached: Ready =
            serde_json::from_slice(&fs::read(preview.join("ready.json")).unwrap()).unwrap();
        assert_eq!(previous_url(&cached), format!("http://c.localhost:{port}/"));
        server.join().unwrap();
    }

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

    #[test]
    fn ac7_previous_url_uses_consumer_and_falls_back_only_without_page() {
        let mut ready: Ready = serde_json::from_value(json!({"watane":1,"mode":"preview","home":"/home","port":1234,"api":"untrusted","mcp":null,"token":null,"mods":[{"local":"provider","page":"untrusted"},{"local":"consumer","page":"untrusted"}]})).unwrap();
        assert_eq!(previous_url(&ready), "http://consumer.localhost:1234/");
        ready.mods.last_mut().unwrap().page = None;
        assert_eq!(previous_url(&ready), "http://localhost:1234/api/");
    }

    #[test]
    fn ac7_previous_warning_requires_answering_preview_and_names_page() {
        use std::io::Read;
        use std::net::TcpListener;
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("counter");
        fs::create_dir(&dir).unwrap();
        let env = Env {
            cache: temp.path().join("cache"),
            unit_seconds: 60,
        };
        assert!(previous_warning(&dir, &env, "").is_none());
        let preview = folder(&dir, &env, "counter");
        let home = preview.join("home");
        fs::create_dir_all(&home).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        fs::write(
            home.join("run.json"),
            json!({"watane":1,"mode":"preview","pid":std::process::id(),"port":port}).to_string(),
        )
        .unwrap();
        fs::write(preview.join("ready.json"), json!({"watane":1,"mode":"preview","home":home,"port":port,"api":"untrusted","mcp":null,"token":null,"mods":[{"local":"counter","page":"untrusted"}]}).to_string()).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let body = json!({"watane":1,"mode":"preview","home":home,"port":port}).to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let warning = previous_warning(&dir, &env, " --dir /m --home /h").unwrap();
        assert_eq!(
            serde_json::to_value(warning).unwrap(),
            json!({
                "code":"preview.previous",
                "message":format!("the preview at http://counter.localhost:{port}/ still serves the previous build"),
                "file":null,"line":null,"column":null,
                "fix":"Fix the errors, then run: watane-dev serve --json --dir /m --home /h"
            })
        );
        server.join().unwrap();
        assert!(previous_warning(&dir, &env, "").is_none());
    }

    #[test]
    fn ac7_link_writes_default_script_once_and_keeps_the_makers() {
        let temp = tempfile::tempdir().unwrap();
        let program = temp.path().join("fake-claude-bin");
        fs::write(&program, "x").unwrap();
        let folder = temp.path().join("P");
        let link = link_fake_claude(&folder, &program).unwrap();
        assert_eq!(link, folder.join("claude/fake-claude"));
        assert_eq!(fs::read_link(&link).unwrap(), program);
        let script = folder.join("claude/fake-claude.json");
        assert_eq!(
            fs::read_to_string(&script).unwrap(),
            r#"{"steps":[{"do":"expect-user"},{"do":"delta","text":"This is the fake claude."},{"do":"message","text":"This is the fake claude."},{"do":"result"},{"do":"idle"}]}"#
        );
        fs::write(&script, r#"{"steps":[]}"#).unwrap();
        link_fake_claude(&folder, &program).unwrap();
        assert_eq!(fs::read_to_string(&script).unwrap(), r#"{"steps":[]}"#);
    }

    #[test]
    fn ac7_link_is_replaced_and_copied_where_linking_fails() {
        let temp = tempfile::tempdir().unwrap();
        let (a, b) = (temp.path().join("a"), temp.path().join("b"));
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let folder = temp.path().join("P");
        link_fake_claude(&folder, &a).unwrap();
        let link = link_fake_claude(&folder, &b).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), b);
        let copy = temp.path().join("copy");
        place(
            &a,
            &copy,
            |_, _| Err(std::io::ErrorKind::Unsupported.into()),
        )
        .unwrap();
        fs::read_link(&copy).unwrap_err();
        assert_eq!(fs::read_to_string(&copy).unwrap(), "a");
    }

    #[test]
    fn ac7_find_fake_claude_reads_the_variable() {
        let temp = tempfile::tempdir().unwrap();
        let program = temp.path().join("fake-claude");
        fs::write(&program, "x").unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        let found = crate::env::with_vars(
            &[("WATANE_FAKE_CLAUDE", Some(program.as_os_str()))],
            find_fake_claude,
        );
        assert_eq!(found.unwrap(), program.canonicalize().unwrap());
        let missing = temp.path().join("nope");
        let warning = crate::env::with_vars(
            &[("WATANE_FAKE_CLAUDE", Some(missing.as_os_str()))],
            find_fake_claude,
        )
        .unwrap_err();
        let json = serde_json::to_value(&warning).unwrap();
        assert_eq!(json["code"], "preview.fake_claude_missing");
        assert_eq!(
            json["message"],
            format!(
                "the program fake-claude was not found ({}); a preview cannot start Claude Code",
                missing.display()
            )
        );
        assert_eq!(
            json["fix"],
            "Install watane-dev with its agent layer fakes, or set WATANE_FAKE_CLAUDE to the program"
        );
    }

    #[test]
    fn ac7_spawn_arguments_with_and_without_the_fake() {
        let home = Path::new("/p/home");
        assert_eq!(spawn_args(home, None), ["--preview", "/p/home"]);
        assert_eq!(
            spawn_args(home, Some(Path::new("/p/claude/fake-claude"))),
            [
                "--preview",
                "/p/home",
                "--fake-claude",
                "/p/claude/fake-claude"
            ]
        );
    }
}
