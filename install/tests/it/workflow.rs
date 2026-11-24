//! Release workflow line rules and their mutations (TECHNICAL 7, INST-S-29).

use std::fs;
use std::path::Path;

fn read(relative: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(relative),
    )
    .expect(relative)
}

// These are deliberately line rules, not a YAML parser. Keep security checks
// scoped to their job, and join shell continuations before checking commands.
fn lines(text: &str) -> String {
    text.replace("\\\n", " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

const TOP: &[&str] = &[
    "on:",
    "push:",
    "tags: ['v[0-9]+.[0-9]+.[0-9]+*']",
    "workflow_dispatch: {}",
    "permissions: {}",
];
const PACKAGE: &[&str] = &[
    "runs-on: macos-15",
    "contents: read",
    "RUSTUP_TOOLCHAIN: stable",
    "persist-credentials: false",
    "fetch-depth: 0",
    "id: version",
    "shell: bash",
    "set -euo pipefail",
    "import json, os, re, tomllib",
    "paths = ['host/Cargo.toml', 'dev/Cargo.toml', 'node/Cargo.toml', 'app/Cargo.toml']",
    "versions = [tomllib.loads(open(path).read())['package']['version'] for path in paths]",
    "versions.append(json.load(open('app/tauri.conf.json'))['version'])",
    "tag_run = os.environ['GITHUB_REF'].startswith('refs/tags/')",
    "version = os.environ['GITHUB_REF_NAME'][1:] if tag_run else versions[0]",
    r"assert re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?', version), 'invalid release version'",
    "assert all(v == version for v in versions), f'version mismatch: {versions} != {version}'",
    "with open(os.environ['GITHUB_OUTPUT'], 'a') as output:",
    "print(f'version={version}', file=output)",
    "if [[ $GITHUB_REF == refs/tags/* ]]; then",
    "git merge-base --is-ancestor \"$GITHUB_SHA\" refs/remotes/origin/main",
    "rustc --version",
    "cargo --version",
    "MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --locked -p watane-host -p watane-dev -p watane-node",
    "cargo install tauri-cli --version '=2.12.4' --locked",
    "working-directory: app",
    "MACOSX_DEPLOYMENT_TARGET: '14.0'",
    "--config '{\"bundle\":{\"active\":true,\"targets\":[\"app\"],\"macOS\":{\"minimumSystemVersion\":\"14.0\"}}}'",
    "-- --locked",
    "VERSION: ${{ steps.version.outputs.version }}",
    "run: install/package.sh aarch64-apple-darwin \"$VERSION\" dist-out/",
    "run: sh install/tests/clean_mac.sh --from \"$PWD/dist-out\" --fresh-home",
    "name: macos-release",
    "path: dist-out/",
    "if-no-files-found: error",
];
const RELEASE: &[&str] = &[
    "runs-on: ubuntu-latest",
    "needs: package-macos",
    "if: ${{ github.event_name == 'push' && startsWith(github.ref, 'refs/tags/') }}",
    "environment: release",
    "contents: write",
    "name: macos-release",
    "path: dist-out/",
    "GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}",
    "shell: bash",
    "set -euo pipefail",
    "version=${GITHUB_REF_NAME#v}",
    "curl -fsSL https://github.com/civic-tech-pioneer/watane/releases/latest/download/install.sh | sh",
    "curl -fsSL https://github.com/civic-tech-pioneer/watane/releases/download/v${version}/install.sh | sh",
    "https://github.com/civic-tech-pioneer/watane/blob/v${version}/install/TECHNICAL.md#62-the-text",
    "prerelease=false",
    "[[ $version != *-* ]] || prerelease=true",
    "gh release create \"$GITHUB_REF_NAME\"",
    "--repo \"$GITHUB_REPOSITORY\" --draft --title \"watane $version\"",
    "--notes-file release-notes.md --prerelease=\"$prerelease\"",
    "\"dist-out/watane-$version-aarch64-apple-darwin.tar.gz\"",
    "dist-out/SHA256SUMS dist-out/install.sh",
    "gh release edit \"$GITHUB_REF_NAME\" --repo \"$GITHUB_REPOSITORY\" --draft=false",
];

fn violations(text: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let (top, jobs) = text.split_once("\njobs:\n").unwrap_or((text, ""));
    let (package, release) = jobs.split_once("\n  release:\n").unwrap_or((jobs, ""));
    for (scope, body, required) in [
        ("top", top, TOP),
        ("package", package, PACKAGE),
        ("release", release, RELEASE),
    ] {
        let normalized = lines(body);
        for needle in required {
            if !normalized.contains(needle) {
                errors.push(format!("{scope}: missing {needle}"));
            }
        }
    }
    if !jobs.starts_with("  package-macos:\n") {
        errors.push("package-macos must be the first job".into());
    }
    // Only these two job headers and the two specified triggers are supported.
    for line in jobs.lines() {
        if line.starts_with("  ")
            && !line.starts_with("   ")
            && !matches!(line, "  package-macos:" | "  release:")
        {
            errors.push("unexpected job".into());
        }
    }
    for line in text.lines().map(str::trim) {
        if line.starts_with('#') {
            continue;
        }
        if line.contains("cache:")
            || line.contains("cache/")
            || line.contains("rust-cache")
            || line.contains("actions/cache")
            || line.contains("setup-")
            || line.starts_with("inputs:")
            || line.starts_with("pull_request")
            || line.starts_with("branches:")
            || line.starts_with("schedule:")
            || line.contains("secrets.") && !line.contains("secrets.GITHUB_TOKEN")
        {
            errors.push(format!("forbidden workflow input: {line}"));
        }
    }
    let actions: Vec<_> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- uses: "))
        .collect();
    if actions.len() != 3 {
        errors.push("exactly checkout, upload and download actions".into());
    }
    for (line, (action, version)) in actions.iter().zip([
        ("actions/checkout", "v7.0.1"),
        ("actions/upload-artifact", "v7.0.2"),
        ("actions/download-artifact", "v8.0.2"),
    ]) {
        let (reference, comment) = line.split_once(" # ").unwrap_or((line, ""));
        let (name, sha) = reference.split_once('@').unwrap_or((reference, ""));
        if name != action
            || sha.len() != 40
            || !sha.bytes().all(|b| b.is_ascii_hexdigit())
            || comment != version
        {
            errors.push(format!(
                "action must be SHA pinned with tag comment: {line}"
            ));
        }
    }
    if package.contains("write") || release.contains("actions/checkout") {
        errors.push("build must be read-only; publisher must not checkout".into());
    }
    for line in release.lines().map(str::trim) {
        let command = line.strip_prefix("run: ").unwrap_or(line);
        if command.starts_with("cargo ")
            || command.starts_with("sh ")
            || command.starts_with("bash ")
            || command.starts_with("./")
        {
            errors.push("publisher must not run repository code".into());
        }
    }
    let package = lines(package);
    let tauri_locked = package.lines().any(|line| {
        let words: Vec<_> = line.split_whitespace().collect();
        words.starts_with(&["cargo", "tauri", "build", "--bundles", "app"])
            && words.ends_with(&["--", "--locked"])
    });
    if !tauri_locked {
        errors.push("Tauri's build command must pass -- --locked".into());
    }
    let mut previous = 0;
    for step in [
        "actions/checkout@",
        "id: version",
        "rustc --version",
        "cargo build --release --locked",
        "cargo install tauri-cli",
        "cargo tauri build --bundles app",
        "install/package.sh",
        "sh install/tests/clean_mac.sh",
        "actions/upload-artifact@",
    ] {
        match package.find(step) {
            Some(position) if position >= previous => previous = position,
            _ => errors.push(format!("missing or out-of-order step: {step}")),
        }
    }
    let release = lines(release);
    if release.lines().last() != RELEASE.last().copied() {
        errors.push("publishing the draft must be last".into());
    }
    errors
}

#[test]
fn ac3_short_origin_main_ref_is_rejected() {
    let original = read(".github/workflows/release.yml");
    let broken = original.replace("refs/remotes/origin/main", "origin/main");
    assert!(
        violations(&broken)
            .iter()
            .any(|error| error.contains("refs/remotes/origin/main")),
        "accepted the ambiguous origin/main ref"
    );
}

#[test]
fn ac1_inst_s29_release_workflow_obeys_contract() {
    let errors = violations(&read(".github/workflows/release.yml"));
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[cfg(unix)] // Runs the POSIX publisher script with an executable sh fixture.
#[test]
fn ac1_publisher_uploads_a_draft_then_publishes_stable_and_prerelease() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let workflow = read(".github/workflows/release.yml");
    let script = workflow
        .rsplit_once("        run: |\n")
        .expect("publisher")
        .1;
    let script = script
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    let fixture = tempfile::tempdir().expect("fixture");
    let gh = fixture.path().join("gh");
    fs::write(
        &gh,
        "#!/bin/sh\nprintf 'CALL\\n' >> calls\nprintf '<%s>\\n' \"$@\" >> calls\n",
    )
    .expect("fake gh");
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).expect("chmod");
    for version in ["0.1.0", "0.2.0-rc1"] {
        let calls = fixture.path().join("calls");
        if calls.exists() {
            fs::remove_file(&calls).expect("clear calls");
        }
        let output = Command::new("/bin/bash")
            .args(["-c", &script])
            .current_dir(fixture.path())
            .env_clear()
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", fixture.path().display()),
            )
            .env("GITHUB_REF_NAME", format!("v{version}"))
            .env("GITHUB_REPOSITORY", "civic-tech-pioneer/watane")
            .output()
            .expect("publisher fixture");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let calls = fs::read_to_string(calls).expect("calls");
        let expected = format!(
            "CALL\n<release>\n<create>\n<v{version}>\n<--repo>\n<civic-tech-pioneer/watane>\n<--draft>\n<--title>\n<watane {version}>\n<--notes-file>\n<release-notes.md>\n<--prerelease={}>\n<dist-out/watane-{version}-aarch64-apple-darwin.tar.gz>\n<dist-out/SHA256SUMS>\n<dist-out/install.sh>\nCALL\n<release>\n<edit>\n<v{version}>\n<--repo>\n<civic-tech-pioneer/watane>\n<--draft=false>\n",
            version.contains('-')
        );
        assert_eq!(calls, expected);
        let notes = fs::read_to_string(fixture.path().join("release-notes.md")).expect("notes");
        assert!(notes.contains(&format!("/download/v{version}/install.sh | sh")));
        assert!(notes.contains("```sh\n"));
        assert!(notes.contains("#62-the-text"));
    }
}

#[test]
fn ac2_each_required_rule_rejects_a_broken_copy() {
    let original = read(".github/workflows/release.yml");
    assert!(violations(&original).is_empty());
    for needle in TOP.iter().chain(PACKAGE).chain(RELEASE) {
        // Remove a single occurrence in its own scope, including duplicated rules.
        let scopes: Vec<_> = if TOP.contains(needle) {
            vec![original.split("\njobs:\n").next().expect("top")]
        } else {
            let jobs = original.split_once("\njobs:\n").expect("jobs").1;
            let (package, release) = jobs.split_once("\n  release:\n").expect("release");
            [package, release]
                .into_iter()
                .filter(|s| s.contains(*needle))
                .collect()
        };
        for scope in scopes {
            let broken_scope = scope.replacen(needle, "BROKEN", 1);
            let broken = original.replacen(scope, &broken_scope, 1);
            assert_ne!(original, broken, "mutation did not apply: {needle}");
            assert!(
                !violations(&broken).is_empty(),
                "accepted missing rule: {needle}"
            );
        }
    }
}

#[test]
fn ac2_unsafe_additions_and_substitutions_are_rejected() {
    let original = read(".github/workflows/release.yml");
    for (from, to) in [
        ("persist-credentials: false", "persist-credentials: true"),
        ("contents: read", "contents: write"),
        (
            "workflow_dispatch: {}",
            "workflow_dispatch:\n    inputs:\n      publish:\n        type: boolean",
        ),
        ("permissions: {}", "permissions: {contents: write}"),
        ("-- --locked", "--"),
        ("-- --locked", "--\n      - run: echo -- --locked"),
        ("environment: release", "environment: staging"),
        ("steps:\n", "steps:\n      - uses: actions/cache@v4\n"),
        ("steps:\n", "steps:\n      - uses: Swatinem/rust-cache@v2\n"),
        ("fetch-depth: 0", "fetch-depth: 0\n          cache: cargo"),
        (
            "workflow_dispatch: {}",
            "workflow_dispatch: {}\n  pull_request: {}",
        ),
        (
            "GH_TOKEN:",
            "OTHER: ${{ secrets.RELEASE_TOKEN }}\n          GH_TOKEN:",
        ),
        (
            "version=${GITHUB_REF_NAME#v}",
            "cargo build\n          version=${GITHUB_REF_NAME#v}",
        ),
        (
            "version=${GITHUB_REF_NAME#v}",
            "sh install/package.sh\n          version=${GITHUB_REF_NAME#v}",
        ),
        (
            "version=${GITHUB_REF_NAME#v}",
            "./install/package.sh\n          version=${GITHUB_REF_NAME#v}",
        ),
        ("jobs:\n", "jobs:\n  extra:\n    runs-on: ubuntu-latest\n"),
    ] {
        assert!(original.contains(from), "mutation anchor: {from}");
        assert!(
            !violations(&original.replacen(from, to, 1)).is_empty(),
            "accepted {to}"
        );
    }
    for line in original.lines().filter(|line| line.contains("- uses:")) {
        let (action, _) = line.split_once('@').expect("pin");
        let broken = original.replacen(line, &format!("{action}@v7 # v7"), 1);
        assert!(!violations(&broken).is_empty(), "accepted tag: {action}");
        let (pin, _) = line.split_once(" # ").expect("tag comment");
        let stale = original.replacen(line, &format!("{pin} # v4"), 1);
        assert!(
            !violations(&stale).is_empty(),
            "accepted stale comment: {line}"
        );
    }
    assert!(!violations(&format!("{original}\n          echo too-late\n")).is_empty());
}

#[test]
fn ac4_readme_shows_install_and_no_toolchain_commands() {
    let readme = read("README.md");
    assert!(readme.contains("Install (macOS, Apple silicon)"));
    let command = "curl -fsSL https://github.com/civic-tech-pioneer/watane/releases/latest/download/install.sh | sh";
    assert!(readme.contains(command));
    assert!(readme.contains(&format!("{command} -s -- --no-toolchain")));
    assert!(readme.contains("](install/FUNCTIONAL.md)"));
}
