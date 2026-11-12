use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use watane_host::watane_log::{Entry, Log};
use watane_host::watane_pkg::{self as pkg, Mod, Package, RELEASE_KIND, Release};
use watane_host::{
    Answer, Caller, Confirm, Host, HostError, InstallRequest, Limits, Mode, Requester, Source,
    VerifyCheck,
};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods/counter")
}
fn release() -> (Package, Release) {
    let dir = fixture();
    let package = pkg::pack(&dir, false).unwrap();
    let body = Release::new(&Mod::read(&dir).unwrap(), &package).unwrap();
    (package, body)
}
fn host(home: &Path) -> Arc<Host> {
    Host::open(home, Mode::Preview, Limits::default()).unwrap()
}
fn assert_verify(error: HostError, check: VerifyCheck, text: &str) {
    assert!(
        matches!(&error, HostError::Verify { check: actual, .. } if *actual == check),
        "{error:?}"
    );
    assert_eq!(error.code(), "verify");
    assert_eq!(error.status(), 422);
    let value = serde_json::to_value(&error).unwrap();
    assert_eq!(value["code"], "verify");
    assert_eq!(value["check"], text);
    assert_eq!(value["message"], error.to_string());
}

#[test]
fn ac1_ac6_plan_identifies_the_signed_publisher_through_reexports() {
    let log_dir = tempfile::tempdir().unwrap();
    let log = Log::open(&log_dir.path().join("log")).unwrap();
    let (package, body) = release();
    let entry = log.append(RELEASE_KIND, &body.encode()).unwrap();
    let home = tempfile::tempdir().unwrap();
    let host = host(home.path());
    let plan = host.plan(vec![Source::Release { package, entry }]).unwrap();
    assert_eq!(plan.entries().len(), 1);
    let planned = &plan.entries()[0];
    let key = log.identity().to_string();
    assert_eq!(planned.publisher.as_ref().unwrap().to_string(), key);
    assert_eq!(planned.identity, format!("{key}/counter"));
    assert_eq!(
        planned.release.as_ref().unwrap().to_string(),
        format!("{key}/counter@0.1.0")
    );
    assert!(
        home.path()
            .join("staging")
            .read_dir()
            .unwrap()
            .next()
            .is_some()
    );
    drop(plan);
    assert_eq!(home.path().join("staging").read_dir().unwrap().count(), 0);
}

#[test]
fn ac2_changed_package_entry_and_summary_are_refused() {
    let log_dir = tempfile::tempdir().unwrap();
    let log = Log::open(&log_dir.path().join("log")).unwrap();
    let (package, mut body) = release();
    let entry = log.append(RELEASE_KIND, &body.encode()).unwrap();
    let changed = tempfile::tempdir().unwrap();
    let dir = changed.path().join("mod");
    package.unpack(&dir).unwrap();
    let file = dir.join("ui/index.html");
    let mut bytes = fs::read(&file).unwrap();
    bytes[0] ^= 1;
    fs::write(file, bytes).unwrap();
    let changed_package = pkg::pack(&dir, false).unwrap();
    let home = tempfile::tempdir().unwrap();
    let host = host(home.path());
    assert_verify(
        host.plan(vec![Source::Release {
            package: changed_package,
            entry: entry.clone(),
        }])
        .unwrap_err(),
        VerifyCheck::PackageHash,
        "package_hash",
    );
    let mut bytes = entry.bytes().to_vec();
    *bytes.last_mut().unwrap() ^= 1;
    assert!(Entry::decode(&bytes).is_err());
    body.summary = "Another summary.".into();
    let entry = log.append(RELEASE_KIND, &body.encode()).unwrap();
    let error = host
        .plan(vec![Source::Release { package, entry }])
        .unwrap_err();
    assert!(error.to_string().contains("summary"));
    assert_verify(error, VerifyCheck::Mismatch, "mismatch");
    assert_eq!(home.path().join("staging").read_dir().unwrap().count(), 0);
}

#[test]
fn ac3_kind_precedes_release_body_and_errors_carry_checks() {
    let log_dir = tempfile::tempdir().unwrap();
    let log = Log::open(&log_dir.path().join("log")).unwrap();
    let home = tempfile::tempdir().unwrap();
    let host = host(home.path());
    for (kind, check, text) in [
        ("profile", VerifyCheck::Kind, "kind"),
        (RELEASE_KIND, VerifyCheck::ReleaseBody, "release_body"),
    ] {
        let (package, _) = release();
        let entry = log.append(kind, b"not a release").unwrap();
        assert_verify(
            host.plan(vec![Source::Release { package, entry }])
                .unwrap_err(),
            check,
            text,
        );
    }
}

struct Approve;
impl Confirm for Approve {
    fn install(
        &self,
        request: InstallRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Answer> + Send>> {
        assert_eq!(request.entries.len(), 1);
        assert!(request.entries[0].publisher.is_some());
        assert!(request.entries[0].release.is_some());
        Box::pin(async { Answer::Approve { withhold: vec![] } })
    }
}
#[tokio::test]
async fn ac4_release_install_survives_restart_with_page_and_calls() {
    let log_dir = tempfile::tempdir().unwrap();
    let log = Log::open(&log_dir.path().join("log")).unwrap();
    let (package, body) = release();
    let entry = log.append(RELEASE_KIND, &body.encode()).unwrap();
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(
        home.path(),
        Mode::Home {
            app: Some(Arc::new(Approve)),
        },
        Limits::default(),
    )
    .unwrap();
    let plan = host.plan(vec![Source::Release { package, entry }]).unwrap();
    assert!(plan.needs_confirmation());
    let installed = host.install(plan, Requester::App).await.unwrap();
    let local = installed[0].local.clone();
    let key = log.identity().to_string();
    let expected_release = format!("{key}/counter@0.1.0");
    assert_eq!(
        installed[0].release.as_ref().unwrap().to_string(),
        expected_release
    );
    assert_eq!(installed[0].publisher.as_ref().unwrap().to_string(), key);
    assert_eq!(
        host.call_async(Caller::Page, local.clone(), "api.add".into(), json!(null))
            .await
            .unwrap(),
        json!(1)
    );
    drop(host);
    let host = Host::open(home.path(), Mode::Home { app: None }, Limits::default()).unwrap();
    let recovered = host.get(&local).unwrap();
    assert_eq!(
        recovered.release.as_ref().unwrap().to_string(),
        expected_release
    );
    assert_eq!(recovered.publisher.as_ref().unwrap().to_string(), key);
    assert_eq!(recovered.identity, format!("{key}/counter"));
    assert!(recovered.error.is_none(), "{:?}", recovered.error);
    assert_eq!(
        host.call_async(Caller::Page, local.clone(), "api.get".into(), json!(null))
            .await
            .unwrap(),
        json!(1)
    );
    let served = host.serve(0, axum::Router::new()).await.unwrap();
    let response = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{}/", served.port))
        .header("Host", format!("{local}.localhost:{}", served.port))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        b"<button id=\"add\">Add</button><script src=\"app.js\"></script>\n"
    );
    served.shutdown().await;
}

#[tokio::test]
async fn ac4_recovery_restores_committed_release_bytes_including_source() {
    let log_dir = tempfile::tempdir().unwrap();
    let log = Log::open(&log_dir.path().join("log")).unwrap();
    for include_source in [false, true] {
        let input = tempfile::tempdir().unwrap();
        let dir = input.path().join("mod");
        let (package, _) = release();
        package.unpack(&dir).unwrap();
        fs::create_dir(dir.join("tests")).unwrap();
        fs::write(dir.join("tests/source.txt"), b"published source").unwrap();
        let package = pkg::pack(&dir, include_source).unwrap();
        let body = Release::new(&Mod::read(&dir).unwrap(), &package).unwrap();
        let entry = log.append(RELEASE_KIND, &body.encode()).unwrap();
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(
            home.path(),
            Mode::Home {
                app: Some(Arc::new(Approve)),
            },
            Limits::default(),
        )
        .unwrap();
        let plan = host.plan(vec![Source::Release { package, entry }]).unwrap();
        host.install(plan, Requester::App).await.unwrap();
        drop(host);
        // A crash after the folder swap but before the catalog commit.
        let current = home.path().join("mods/counter");
        fs::rename(&current, current.with_extension("old")).unwrap();
        let (package, _) = release();
        package.unpack(&current).unwrap();
        fs::write(current.join("ui/index.html"), b"uncommitted page").unwrap();
        let host = Host::open(home.path(), Mode::Home { app: None }, Limits::default()).unwrap();
        assert_eq!(
            fs::read(current.join("ui/index.html")).unwrap(),
            b"<button id=\"add\">Add</button><script src=\"app.js\"></script>\n"
        );
        assert_eq!(current.join("tests/source.txt").exists(), include_source);
        assert!(!current.with_extension("old").exists());
        assert_eq!(
            host.mods()[0].release.as_ref().unwrap().to_string(),
            format!("{}/counter@0.1.0", log.identity())
        );
    }
}
