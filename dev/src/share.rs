//! Sharing uses the maker's git and gh, with no credentials held by dev.
use crate::outcome::{Code, Problem};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

struct Run {
    program: &'static str,
    args: Vec<String>,
}
impl Run {
    fn output(&self) -> std::io::Result<Output> {
        Command::new(self.program)
            .args(&self.args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GH_PROMPT_DISABLED", "1")
            .env("GIT_LFS_SKIP_SMUDGE", "1")
            .output()
    }
    /// The command as shown in messages and fixes: without dev's own
    /// `-c http.lowSpeed*` guard, which `args` (the argv) keeps.
    fn text(&self) -> String {
        let mut shown = vec![self.program];
        let mut args = self.args.iter().map(String::as_str).peekable();
        while let Some(arg) = args.next() {
            if arg == "-c" && args.peek().is_some_and(|v| v.starts_with("http.lowSpeed")) {
                args.next();
            } else {
                shown.push(arg);
            }
        }
        shown.into_iter().map(quote).collect::<Vec<_>>().join(" ")
    }
    fn failure(&self, stderr: &str, rest: &[Run]) -> Problem {
        let tail = stderr_tail(stderr);
        Problem::new(
            Code::PublishPushFailed,
            format!("{} failed: {tail}", self.text()),
        )
        .fix(finishing(rest))
    }
}
fn stderr_tail(stderr: &str) -> String {
    stderr
        .lines()
        .map(|line| {
            let mut line = line.trim_end();
            while line.starts_with("remote: remote:") {
                line = &line["remote: ".len()..];
            }
            line
        })
        .filter(|line| !line.is_empty() && *line != "remote:")
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}
/// Only git's last stderr line, without its guess `(got 'user@host...')`,
/// which is the person's private machine name.
fn identity_message(stderr: &str) -> String {
    let last = stderr.lines().rev().find(|l| !l.trim().is_empty());
    match last {
        None => "git has no name and email for commits".into(),
        Some(line) => {
            let line = line.trim();
            let line = line.find(" (got ").map_or(line, |i| &line[..i]);
            format!("git has no name and email for commits: {line}")
        }
    }
}
fn quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-@=:".contains(c))
    {
        s.into()
    } else {
        format!("'{}'", s.replace('\'', "'\"'\"'"))
    }
}
fn finishing(commands: &[Run]) -> String {
    commands
        .iter()
        .map(Run::text)
        .collect::<Vec<_>>()
        .join(" && ")
}
fn git(dir: &Path, args: &[&str]) -> Run {
    let mut all = vec!["-C".into(), dir.to_string_lossy().into_owned()];
    if matches!(args.first(), Some(&"push" | &"fetch" | &"ls-remote")) {
        all.extend(
            [
                "-c",
                "http.lowSpeedLimit=1000",
                "-c",
                "http.lowSpeedTime=60",
            ]
            .map(String::from),
        );
    }
    all.extend(args.iter().map(|s| (*s).into()));
    Run {
        program: "git",
        args: all,
    }
}
fn gh(args: &[&str]) -> Run {
    Run {
        program: "gh",
        args: args.iter().map(|s| (*s).into()).collect(),
    }
}
fn checked(command: &Run) -> Result<String, Problem> {
    let output = command
        .output()
        .map_err(|e| command.failure(&e.to_string(), std::slice::from_ref(command)))?;
    if !output.status.success() {
        return Err(command.failure(
            &String::from_utf8_lossy(&output.stderr),
            std::slice::from_ref(command),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}
fn read_git(dir: &Path, args: &[&str]) -> Result<Option<String>, Problem> {
    let command = git(dir, args);
    let output = command.output().map_err(Problem::internal)?;
    if output.status.success() {
        Ok(Some(String::from_utf8_lossy(&output.stdout).trim().into()))
    } else if output.status.code() == Some(1) {
        Ok(None)
    } else {
        Err(command.failure(
            &String::from_utf8_lossy(&output.stderr),
            std::slice::from_ref(&command),
        ))
    }
}
fn normalize(url: &str) -> &str {
    let url = url.strip_suffix('/').unwrap_or(url);
    url.strip_suffix(".git").unwrap_or(url)
}
fn same_url(a: &str, b: &str) -> bool {
    normalize(a) == normalize(b)
}
fn github_name(url: &str) -> Option<&str> {
    let name = normalize(url).strip_prefix("https://github.com/")?;
    let (owner, repo) = name.split_once('/')?;
    (!owner.is_empty() && !repo.is_empty() && !repo.contains('/')).then_some(name)
}
fn first_difference(
    expected: &watane_pkg::Package,
    actual: &watane_pkg::Package,
) -> Option<String> {
    let paths: std::collections::BTreeSet<_> = expected.paths().chain(actual.paths()).collect();
    paths.into_iter().find_map(|path| {
        let reason = match (expected.file(path), actual.file(path)) {
            (Some(_), None) => "missing",
            (None, Some(_)) => "extra",
            (Some(a), Some(b)) if a != b => "different",
            _ => return None,
        };
        Some(format!("{path}: {reason}"))
    })
}
/// 10.4.2 steps 2 to 4: clone the release tag into `folder` and keep only the
/// package's files. `Err` is the warning, with `folder` removed.
pub fn fork(
    folder: &Path,
    repository: &str,
    release: &watane_pkg::ReleaseRef,
    package: &watane_pkg::Package,
) -> Result<(), Problem> {
    let slug = &release.slug;
    let release = &release.to_string();
    let command = Run {
        program: "git",
        args: [
            "-c",
            "advice.detachedHead=false",
            "-c",
            "core.symlinks=false",
            "-c",
            "http.lowSpeedLimit=1000",
            "-c",
            "http.lowSpeedTime=60",
            "clone",
            "-q",
            "--branch",
            release,
            "--",
            repository,
        ]
        .map(String::from)
        .into_iter()
        .chain([folder.to_string_lossy().into_owned()])
        .collect(),
    };
    let failure = match command.output() {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .last()
                .unwrap_or("clone failed")
                .to_string(),
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some("git not found".into()),
        Err(e) => Some(e.to_string()),
    };
    if let Some(reason) = failure {
        let _ = fs::remove_dir_all(folder);
        return Err(Problem::new(Code::NewForkUnavailable, format!("could not clone {repository} at {release} ({}); the mod was made from the verified package, with no git history", reason))
            .fix(format!("To keep the publisher's history, check that git can reach {repository}, delete {}, then run new --from again", folder.display())));
    }
    keep_package(folder, package).map_err(|mut e| {
        e.message = format!("{repository} at {release} differs from the verified package ({}); the mod was made from the verified package", e.message);
        e.fix = Some(format!("Tell the person: the repository of {slug} does not hold the published source of {release}"));
        e
    })
}
/// 10.4.2 steps 3 and 4 on a clone; on failure remove the checkout.
fn keep_package(folder: &Path, package: &watane_pkg::Package) -> Result<(), Problem> {
    fn prune(dir: &Path, relative: &Path, paths: &[&str]) -> std::io::Result<()> {
        for entry in fs::read_dir(dir.join(relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            if path == Path::new(".git") {
                continue;
            }
            let text = path.to_string_lossy();
            let ty = fs::symlink_metadata(entry.path())?.file_type();
            if ty.is_dir() && paths.iter().any(|p| p.starts_with(&format!("{text}/"))) {
                prune(dir, &path, paths)?;
            } else if !paths.contains(&text.as_ref()) || !ty.is_file() {
                if ty.is_dir() {
                    fs::remove_dir_all(entry.path())?;
                } else {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }
    let result = (|| {
        if let Some(bytes) = package.file("logic.wasm") {
            // Never follow a repository's link when supplying the built component.
            let path = folder.join("logic.wasm");
            if path.symlink_metadata().is_ok() {
                fs::remove_file(&path).map_err(Problem::internal)?;
            }
            fs::write(path, bytes).map_err(Problem::internal)?;
        }
        let actual = watane_pkg::pack(folder, true)
            .map_err(|e| Problem::new(Code::NewForkDiffers, e.to_string()))?;
        if let Some(path) = first_difference(package, &actual) {
            return Err(Problem::new(Code::NewForkDiffers, path));
        }
        prune(folder, Path::new(""), &package.paths().collect::<Vec<_>>())
            .map_err(Problem::internal)?;
        checked(&git(folder, &["checkout", "-q", "-B", "main"]))?;
        checked(&git(folder, &["remote", "rename", "origin", "upstream"]))?;
        Ok(())
    })();
    result.map_err(|mut e: Problem| {
        let _ = fs::remove_dir_all(folder);
        e.code = Code::NewForkDiffers;
        e
    })
}
fn raw(repository: &str, branch: &str) -> Option<String> {
    let name = normalize(repository).strip_prefix("https://github.com/")?;
    Some(format!(
        "https://raw.githubusercontent.com/{name}/{branch}/"
    ))
}
fn gh_code(stderr: &str) -> Option<Code> {
    if stderr.contains("HTTP 404") {
        None
    } else if stderr.contains("HTTP 401") || stderr.contains("gh auth login") {
        Some(Code::EnvGhAuth)
    } else {
        Some(Code::PublishPushFailed)
    }
}
/// No writes or network mutations until all preflight checks have passed.
pub fn owner(dir: &Path, rerun: &str) -> Result<String, Problem> {
    let version = git(dir, &["--version"]);
    let output = version.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Problem::new(Code::EnvGitMissing, "the program git was not found on PATH")
                .fix(format!("Install git, then run: {rerun}"))
        } else {
            version.failure(&e.to_string(), std::slice::from_ref(&version))
        }
    })?;
    if !output.status.success() {
        return Err(version.failure(
            &String::from_utf8_lossy(&output.stderr),
            std::slice::from_ref(&version),
        ));
    }
    let ident = git(dir, &["var", "GIT_COMMITTER_IDENT"]);
    let output = ident.output().map_err(Problem::internal)?;
    if !output.status.success() {
        return Err(Problem::new(Code::EnvGitIdentity,identity_message(&String::from_utf8_lossy(&output.stderr))).fix(format!("Ask the person to run: git config --global user.name \"<name>\" and git config --global user.email \"<email>\", then run: {rerun}")));
    }
    let command = gh(&["api", "user", "--jq", ".login"]);
    let output=command.output().map_err(|e| {
        if e.kind()==std::io::ErrorKind::NotFound { Problem::new(Code::EnvGhMissing,"the GitHub CLI gh was not found on PATH; publish --push creates repositories with it").fix(format!("Install the GitHub CLI (gh), ask the person to run: gh auth login, then run: {rerun}")) }
        else { command.failure(&e.to_string(),std::slice::from_ref(&command)) }
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if gh_code(&stderr) == Some(Code::EnvGhAuth) {
            return Err(Problem::new(Code::EnvGhAuth,format!("gh is not logged in: {}",stderr.lines().last().unwrap_or_default())).fix(format!("Ask the person to run: gh auth login (dev never logs in for them), then run: {rerun}")));
        }
        return Err(command.failure(&stderr, std::slice::from_ref(&command)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}
fn parent(name: &str) -> Result<Option<String>, Problem> {
    let command = gh(&["api", &format!("repos/{name}"), "--jq", ".parent.full_name"]);
    let output = command.output().map_err(Problem::internal)?;
    if output.status.success() {
        Ok(Some(String::from_utf8_lossy(&output.stdout).trim().into()))
    } else if gh_code(&String::from_utf8_lossy(&output.stderr)).is_none() {
        Ok(None)
    } else {
        Err(command.failure(
            &String::from_utf8_lossy(&output.stderr),
            std::slice::from_ref(&command),
        ))
    }
}
fn exists(name: &str) -> Result<bool, Problem> {
    Ok(parent(name)?.is_some())
}
fn origin(dir: &Path) -> Result<Option<String>, Problem> {
    if dir.join(".git").exists() {
        read_git(dir, &["config", "--get", "remote.origin.url"])
    } else {
        Ok(None)
    }
}
fn join_commands(
    dir: &Path,
    repository: &str,
    new: bool,
    has_origin: bool,
    refs: &str,
) -> Vec<Run> {
    let mut commands = Vec::new();
    if new {
        commands.push(git(dir, &["init", "-q", "-b", "main"]));
    }
    if !has_origin {
        commands.push(git(dir, &["remote", "add", "origin", "--", repository]));
    }
    if new && !refs.is_empty() {
        commands.push(git(dir, &["fetch", "-q", "origin"]));
        if refs
            .lines()
            .any(|line| line.split_whitespace().nth(1) == Some("refs/heads/main"))
        {
            commands.push(git(dir, &["reset", "-q", "origin/main"]));
        }
    }
    commands
}
fn join(dir: &Path, repository: &str) -> Result<(), Problem> {
    let new = !dir.join(".git").exists();
    let refs = if new {
        checked(&git(dir, &["ls-remote", "--", repository]))?
    } else {
        String::new()
    };
    execute(
        dir,
        &join_commands(dir, repository, new, origin(dir)?.is_some(), &refs),
    )
}
fn site_commands(dir: &Path, paths: &[&str], release: &str) -> Vec<Run> {
    let mut add = vec!["add", "-A", "--"];
    add.extend_from_slice(paths);
    vec![
        git(dir, &add),
        git(dir, &["commit", "-q", "-m", release]),
        git(dir, &["push", "-q", "origin", "HEAD"]),
    ]
}
pub fn prepare_mod(
    dir: &Path,
    owner: &str,
    rerun: &str,
) -> Result<(Value, Vec<Problem>), Vec<Problem>> {
    (|| {
        let m = watane_pkg::Mod::read(dir).map_err(crate::check::mod_problems)?;
        let stored = origin(dir).map_err(|e| vec![e])?;
        if let Some(url) = &stored
            && m.repository().is_none_or(|r| !same_url(r, url))
        {
            let repository = m.repository().unwrap_or("<repository>");
            let change_origin = git(dir, &["remote", "set-url", "origin", repository]).text();
            return Err(vec![Problem::new(
                Code::PublishOriginDiffers,
                format!("the mod folder's git remote origin is {url}, and mod.toml names {}", m.repository().unwrap_or("no repository")),
            )
            .at("mod.toml", None, None)
            .fix(format!("Set repository in mod.toml to the https URL of the origin repository, or change the origin with: {change_origin}; then run: {rerun}"))]);
        }
        let mut created = false;
        let mut forked = false;
        let mut warnings = Vec::new();
        let repository = if let Some(repository) = m.repository() {
            repository.to_string()
        } else {
            let name = format!("{owner}/{}", m.slug());
            let repository = format!("https://github.com/{name}");
            let derived = m.origin().is_some();
            let upstream_url = if dir.join(".git").exists() {
                read_git(dir, &["config", "--get", "remote.upstream.url"]).map_err(|e| vec![e])?
            } else {
                None
            };
            let upstream = upstream_url.as_deref().and_then(github_name);
            let existing = parent(&name).map_err(|e| vec![e])?;
            if let Some(p) = existing {
                if derived && upstream.is_some_and(|u| p == u) {
                    forked = true;
                } else if !checked(&git(dir, &["ls-remote", "--", &repository]))
                    .map_err(|e| vec![e])?
                    .is_empty()
                {
                    return Err(vec![Problem::new(Code::PublishRepositoryExists,
                        format!("{repository} exists and holds commits that watane-dev did not push for this mod"))
                        .at("mod.toml", None, None)
                        .fix(format!("Set repository in mod.toml to the https URL of an empty or new repository for this mod, then run: {rerun}"))]);
                }
            } else {
                let mut reason = "no upstream repository".to_string();
                if derived && let Some(upstream) = upstream {
                    let request = gh(&[
                        "api",
                        "-X",
                        "POST",
                        &format!("repos/{upstream}/forks"),
                        "-f",
                        &format!("name={}", m.slug()),
                        "-F",
                        "default_branch_only=true",
                        "--jq",
                        ".full_name",
                    ]);
                    match request.output() {
                        Ok(out)
                            if out.status.success()
                                && String::from_utf8_lossy(&out.stdout).trim() == name =>
                        {
                            forked = true;
                            created = true;
                            // GitHub creates forks asynchronously. Wait only for this repository.
                            for attempt in 0..=10 {
                                if parent(&name).map_err(|e| vec![e])?.is_some() {
                                    break;
                                }
                                if attempt < 10 {
                                    std::thread::sleep(std::time::Duration::from_secs(1));
                                }
                            }
                        }
                        Ok(out) if out.status.success() => {
                            reason = "the account already has a fork of it".into()
                        }
                        Ok(out) => {
                            reason = String::from_utf8_lossy(&out.stderr)
                                .lines()
                                .last()
                                .unwrap_or("fork failed")
                                .into()
                        }
                        Err(e) => reason = e.to_string(),
                    }
                }
                if !forked {
                    if let Err(e) = checked(&gh(&["repo", "create", &name, "--public"])) {
                        // A late fork may have won the race with repo create.
                        if derived
                            && upstream.is_some()
                            && parent(&name).map_err(|e| vec![e])?.as_deref() == upstream
                        {
                            forked = true;
                        } else {
                            return Err(vec![e]);
                        }
                    } else {
                        created = true;
                    }
                    if derived && !forked {
                        warnings.push(Problem::new(Code::PublishForkUnavailable,
                            format!("could not fork {} as {name} ({reason}); {repository} was created without GitHub's fork link", upstream.unwrap_or("no upstream repository"))));
                    }
                }
            }
            if forked
                && checked(&git(dir, &["branch", "--show-current"])).map_err(|e| vec![e])?
                    != "watane"
            {
                checked(&git(dir, &["branch", "-m", "watane"])).map_err(|e| vec![e])?;
            }
            let path = dir.join("mod.toml");
            let mut table = fs::read_to_string(&path)
                .map_err(|e| vec![Problem::internal(e)])?
                .parse::<toml::Table>()
                .map_err(|e| vec![Problem::internal(e)])?;
            table.insert("repository".into(), toml::Value::String(repository.clone()));
            fs::write(
                path,
                toml::to_string(&table).map_err(|e| vec![Problem::internal(e)])?,
            )
            .map_err(|e| vec![Problem::internal(e)])?;
            repository
        };
        join(dir, &repository).map_err(|e| vec![e])?;
        Ok((
            json!({"repository":repository,"created":created,"forked":forked,"pushed":false}),
            warnings,
        ))
    })()
}
fn mod_commands(
    dir: &Path,
    paths: &[&str],
    built: bool,
    release: &str,
    fork_repository: Option<&str>,
) -> Vec<Run> {
    let mut add = vec!["add", "-f", "--"];
    add.extend(
        paths
            .iter()
            .copied()
            .filter(|p| !built || *p != "logic.wasm"),
    );
    let label = release
        .split_once('/')
        .map_or(release, |(_, r)| r)
        .replace('@', " ");
    let mut commands = vec![
        git(
            dir,
            &["rm", "-r", "-q", "--cached", "--ignore-unmatch", "."],
        ),
        git(dir, &add),
        git(dir, &["commit", "-q", "-m", &label]),
        git(dir, &["tag", release]),
        git(
            dir,
            &[
                "push",
                "-q",
                "origin",
                "HEAD",
                &format!("refs/tags/{release}"),
            ],
        ),
    ];
    if let Some(repository) = fork_repository {
        commands.push(gh(&[
            "repo",
            "edit",
            repository,
            "--default-branch",
            "watane",
        ]));
    }
    commands
}
fn execute(dir: &Path, commands: &[Run]) -> Result<(), Problem> {
    for (index, command) in commands.iter().enumerate() {
        if command.args.get(2).is_some_and(|s| s == "commit") {
            let diff = git(dir, &["diff", "--cached", "--quiet"]);
            let out = diff.output().map_err(Problem::internal)?;
            if out.status.success() {
                continue;
            }
            if out.status.code() != Some(1) {
                return Err(diff.failure(&String::from_utf8_lossy(&out.stderr), &commands[index..]));
            }
        }
        let output = command
            .output()
            .map_err(|e| command.failure(&e.to_string(), &commands[index..]))?;
        if !output.status.success() {
            return Err(
                command.failure(&String::from_utf8_lossy(&output.stderr), &commands[index..])
            );
        }
        if command.program == "git" && command.args.get(2).is_some_and(|s| s == "init") {
            fs::write(dir.join(".git/info/exclude"), ".*\n").map_err(Problem::internal)?;
        }
    }
    Ok(())
}
/// Run each repository's list independently, retaining partial success on failure.
pub fn push(
    dir: &Path,
    site: &Path,
    package: &watane_pkg::Package,
    owner: &str,
    release: &str,
    prepared: Option<Value>,
) -> (Value, Vec<Problem>) {
    let mut errors = Vec::new();
    let mut site_result = json!({"repository":null,"raw":null,"created":false,"pushed":false});
    let site_run = (|| {
        let stored = origin(site)?;
        let new = !site.join(".git").exists();
        let repository = stored
            .clone()
            .unwrap_or_else(|| format!("https://github.com/{owner}/watane-channel"));
        site_result["repository"] = json!(repository);
        let mut commands = Vec::new();
        let mut created = false;
        if stored.is_none() {
            let name = format!("{owner}/watane-channel");
            if !exists(&name)? {
                commands.push(gh(&["repo", "create", &name, "--public"]));
                created = true;
            }
        }
        let refs = if new && !created {
            checked(&git(site, &["ls-remote", "--", &repository]))?
        } else {
            String::new()
        };
        commands.extend(join_commands(
            site,
            &repository,
            new,
            stored.is_some(),
            &refs,
        ));
        let branch = if new {
            "main".into()
        } else {
            checked(&git(site, &["branch", "--show-current"]))?
        };
        site_result["raw"] = json!(raw(&repository, &branch));
        // Git rejects absent pathspecs; dist may have no entries yet (the fake has none).
        let mut paths = Vec::new();
        for path in ["head", "entries", "packages"] {
            if site.join(path).exists()
                || (!new
                    && read_git(site, &["ls-files", "--", path])?.is_some_and(|s| !s.is_empty()))
            {
                paths.push(path);
            }
        }
        commands.extend(site_commands(site, &paths, release));
        if created {
            checked(&commands[0]).map_err(|mut e| {
                e.fix = Some(finishing(&commands));
                e
            })?;
            site_result["created"] = json!(true);
            commands.remove(0);
        }
        execute(site, &commands)?;
        site_result["pushed"] = json!(true);
        Ok::<_, Problem>(())
    })();
    if let Err(e) = site_run {
        errors.push(e);
    }
    let mut mod_result = prepared.unwrap_or(Value::Null);
    if !mod_result.is_null() {
        mod_result["tag"] = json!(release);
        let paths = package.paths().collect::<Vec<_>>();
        let name = format!(
            "{owner}/{}",
            release
                .split_once('/')
                .map_or(release, |(_, r)| r)
                .split('@')
                .next()
                .unwrap_or_default()
        );
        let branch = checked(&git(dir, &["branch", "--show-current"]));
        let fork_repository = match &branch {
            Ok(branch) if branch == "watane" => Some(name.as_str()),
            Err(e) => {
                errors.push(e.clone());
                None
            }
            _ => None,
        };
        let commands = mod_commands(
            dir,
            &paths,
            dir.join("logic/Cargo.toml").exists(),
            release,
            fork_repository,
        );
        match execute(dir, &commands) {
            Ok(()) => mod_result["pushed"] = json!(true),
            Err(e) => errors.push(e),
        }
    }
    (json!({"site":site_result,"mod":mod_result}), errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac1_text_omits_transfer_guard_but_argv_keeps_it() {
        for sub in ["push", "fetch", "ls-remote"] {
            let run = git(Path::new("d"), &[sub, "-q"]);
            assert_eq!(run.text(), format!("git -C d {sub} -q"));
            assert_eq!(
                run.args[..6],
                [
                    "-C",
                    "d",
                    "-c",
                    "http.lowSpeedLimit=1000",
                    "-c",
                    "http.lowSpeedTime=60"
                ]
            );
            assert_eq!(run.args[6], sub);
        }
    }
    #[test]
    fn ac2_stderr_tail_collapses_prefix_and_drops_empty_lines() {
        assert_eq!(
            stderr_tail("remote: remote: error: denied   \nremote: \n\n"),
            "remote: error: denied"
        );
        assert_eq!(stderr_tail("remote: remote: remote: x\n"), "remote: x");
        let many: String = (1..=8).map(|i| format!("l{i}\n\n")).collect();
        assert_eq!(stderr_tail(&many), "l4\nl5\nl6\nl7\nl8");
    }
    #[test]
    fn ac3_identity_message_drops_guess_and_hint() {
        let stderr = "\n*** Please tell me who you are.\n\nRun\n\n  git config --global user.email \"you@example.com\"\n\nfatal: unable to auto-detect email address (got 'root@box.(none)')\n";
        assert_eq!(
            identity_message(stderr),
            "git has no name and email for commits: fatal: unable to auto-detect email address"
        );
        assert_eq!(
            identity_message(""),
            "git has no name and email for commits"
        );
    }
    #[test]
    fn ac7_watane_branch_finishes_with_default_branch_edit() {
        let commands = mod_commands(
            Path::new("mod"),
            &["mod.toml"],
            false,
            "key/ratings@0.1.0",
            Some("carol/ratings"),
        );
        assert_eq!(
            commands.iter().map(Run::text).collect::<Vec<_>>(),
            [
                "git -C mod rm -r -q --cached --ignore-unmatch .",
                "git -C mod add -f -- mod.toml",
                "git -C mod commit -q -m 'ratings 0.1.0'",
                "git -C mod tag key/ratings@0.1.0",
                "git -C mod push -q origin HEAD refs/tags/key/ratings@0.1.0",
                "gh repo edit carol/ratings --default-branch watane",
            ]
        );
        assert_eq!(
            finishing(&commands[5..]),
            "gh repo edit carol/ratings --default-branch watane"
        );
    }
    #[test]
    fn ac7_first_package_difference_is_in_byte_order() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        for dir in [&a, &b] {
            fs::create_dir_all(dir.join("wit")).unwrap();
            fs::create_dir_all(dir.join("ui/a")).unwrap();
            fs::write(
                dir.join("mod.toml"),
                "name = \"Demo\"\nsummary = \"page\"\n",
            )
            .unwrap();
            fs::write(
                dir.join("wit/world.wit"),
                "package mods:demo@0.1.0; world demo {}",
            )
            .unwrap();
            fs::write(dir.join("ui/index.html"), "same").unwrap();
        }
        let pack = |dir: &Path| watane_pkg::pack(dir, true).unwrap();
        assert_eq!(first_difference(&pack(&a), &pack(&b)), None);
        fs::write(a.join("ui/a-b"), "left").unwrap();
        fs::write(a.join("ui/a/file"), "left").unwrap();
        assert_eq!(
            first_difference(&pack(&a), &pack(&b)),
            Some("ui/a-b: missing".into())
        );
        assert_eq!(
            first_difference(&pack(&b), &pack(&a)),
            Some("ui/a-b: extra".into())
        );
        fs::write(b.join("ui/a-b"), "right").unwrap();
        assert_eq!(
            first_difference(&pack(&a), &pack(&b)),
            Some("ui/a-b: different".into())
        );
    }
    #[test]
    fn ac7_upstream_github_name() {
        for url in [
            "https://github.com/o/r",
            "https://github.com/o/r.git",
            "https://github.com/o/r/",
        ] {
            assert_eq!(github_name(url), Some("o/r"));
        }
        assert_eq!(github_name("https://other.test/o/r"), None);
    }
    #[test]
    fn ac1_site_lists_for_new_and_existing_repository() {
        let dir = Path::new("site");
        let mut commands = join_commands(
            dir,
            "https://github.com/alice/watane-channel",
            true,
            false,
            "",
        );
        commands.extend(site_commands(
            dir,
            &["head", "entries", "packages"],
            "key/demo@0.1.0",
        ));
        assert_eq!(
            commands.iter().map(Run::text).collect::<Vec<_>>(),
            [
                "git -C site init -q -b main",
                "git -C site remote add origin -- https://github.com/alice/watane-channel",
                "git -C site add -A -- head entries packages",
                "git -C site commit -q -m key/demo@0.1.0",
                "git -C site push -q origin HEAD",
            ]
        );
        assert!(
            join_commands(
                dir,
                "https://github.com/alice/watane-channel",
                false,
                true,
                ""
            )
            .is_empty()
        );
        let commands = join_commands(
            dir,
            "https://github.com/alice/watane-channel",
            true,
            false,
            "abc\trefs/heads/main\n",
        );
        assert_eq!(
            commands.iter().map(Run::text).collect::<Vec<_>>(),
            [
                "git -C site init -q -b main",
                "git -C site remote add origin -- https://github.com/alice/watane-channel",
                "git -C site fetch -q origin",
                "git -C site reset -q origin/main",
            ]
        );
    }
    #[test]
    fn ac1_urls_and_raw_use_stored_origin() {
        assert!(same_url(
            "https://github.com/o/r.git/",
            "https://github.com/o/r"
        ));
        assert!(!same_url(
            "https://github.com/o/r",
            "https://github.com/o/other"
        ));
        assert_eq!(
            raw("https://github.com/o/r", "main"),
            Some("https://raw.githubusercontent.com/o/r/main/".into())
        );
        assert_eq!(
            raw("https://github.com/o/r.git", "main"),
            Some("https://raw.githubusercontent.com/o/r/main/".into())
        );
        assert_eq!(raw("git@github.com:o/r", "main"), None);
        assert_eq!(raw("https://example.test/o/r", "main"), None);
    }
    #[test]
    fn ac3_gh_stderr_classes() {
        assert_eq!(gh_code("gh: Not Found (HTTP 404)"), None);
        assert_eq!(gh_code("HTTP 401"), Some(Code::EnvGhAuth));
        assert_eq!(gh_code("please run: gh auth login"), Some(Code::EnvGhAuth));
        assert_eq!(gh_code("connection failed"), Some(Code::PublishPushFailed));
    }
    #[test]
    fn ac4_remaining_commands_are_shell_quoted() {
        let cmds = [
            git(Path::new("Alice's mod"), &["tag", "key/demo@0.1.0"]),
            git(
                Path::new("Alice's mod"),
                &["push", "-q", "origin", "HEAD", "refs/tags/key/demo@0.1.0"],
            ),
        ];
        assert_eq!(
            finishing(&cmds[1..]),
            "git -C 'Alice'\"'\"'s mod' push -q origin HEAD refs/tags/key/demo@0.1.0"
        );
    }
    #[test]
    fn ac1_ac6_mod_commands_use_only_package_paths() {
        let paths = ["logic.wasm", "mod.toml", "ui/index.html"];
        assert_eq!(
            mod_commands(Path::new("mod"), &paths, true, "key/demo@0.1.0", None)
                .iter()
                .map(Run::text)
                .collect::<Vec<_>>(),
            [
                "git -C mod rm -r -q --cached --ignore-unmatch .",
                "git -C mod add -f -- mod.toml ui/index.html",
                "git -C mod commit -q -m 'demo 0.1.0'",
                "git -C mod tag key/demo@0.1.0",
                "git -C mod push -q origin HEAD refs/tags/key/demo@0.1.0",
            ]
        );
        assert_eq!(
            mod_commands(Path::new("mod"), &paths, true, "key/demo@0.1.0", None)[1].args,
            ["-C", "mod", "add", "-f", "--", "mod.toml", "ui/index.html"]
        );
        assert_eq!(
            mod_commands(Path::new("mod"), &paths, false, "key/demo@0.1.0", None)[1].args,
            [
                "-C",
                "mod",
                "add",
                "-f",
                "--",
                "logic.wasm",
                "mod.toml",
                "ui/index.html"
            ]
        );
    }
}
