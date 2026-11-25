//! Private agent settings under the app home (TECHNICAL 19.1, 19.3 and 19.4).

use crate::CmdError;
use crate::claude_bin::ClaudeStatus;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;
#[cfg(unix)]
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use watane_host::watane_pkg::{PublisherKey, Slug};

#[derive(Deserialize, Serialize)]
struct Judge {
    version: u32,
    key: String,
    origin: String,
}

fn judge_filename(identity: &str) -> Option<String> {
    let (publisher, slug) = identity.split_once('/')?;
    if (publisher != "local" && publisher.parse::<PublisherKey>().is_err())
        || slug.parse::<Slug>().is_err()
    {
        return None;
    }
    Some(format!("{publisher}__{slug}.json"))
}

fn judge_key_valid(key: &str) -> bool {
    !key.is_empty() && key.len() <= 4096
}

#[derive(Deserialize, Serialize)]
pub(crate) struct OpenAi {
    pub version: u32,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
    pub sub: String,
    pub plan: String,
}

#[derive(Deserialize, Serialize)]
struct Claude {
    version: u32,
    path: PathBuf,
}

#[derive(Clone, Serialize)]
pub(crate) struct ClaudeSetting {
    path: PathBuf,
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl From<&ClaudeStatus> for ClaudeSetting {
    fn from(checked: &ClaudeStatus) -> Self {
        Self {
            path: checked.path.clone(),
            version: Some(checked.version.clone()),
            error: None,
        }
    }
}

/// The in-process view of the chosen Claude program and each provider's judge key (19.4).
/// ChatGPT token retrieval comes in its own stage.
pub(crate) struct Source(pub Arc<Accounts>);
impl watane_host::AccountSource for Source {
    fn openai_token(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, watane_host::AccountError>> + Send>,
    > {
        Box::pin(async { Err(watane_host::AccountError::NoAccount) })
    }
    fn judge(
        &self,
        provider: &str,
    ) -> Result<watane_host::JudgeAccount, watane_host::AccountError> {
        self.0
            .judge(provider)
            .ok_or(watane_host::AccountError::NoAccount)
    }
    /// The stored path as it is, also when its check failed (19.3): the host's spawn or its
    /// `initialize` answer decides (`no-binary`, `version`).
    fn claude_binary(&self) -> Result<PathBuf, watane_host::AccountError> {
        self.0
            .claude()
            .map(|c| c.path)
            .ok_or(watane_host::AccountError::NoAccount)
    }
}

pub(crate) struct Accounts {
    app: PathBuf,
    pub sign_in: tokio::sync::Mutex<()>,
    reported_openai_error: Mutex<Option<String>>,
    claude: Mutex<Option<Option<ClaudeSetting>>>,
}

impl Accounts {
    pub fn new(home: &Path) -> Self {
        Self {
            app: home.join("app"),
            sign_in: tokio::sync::Mutex::new(()),
            reported_openai_error: Mutex::new(None),
            claude: Mutex::new(None),
        }
    }

    fn openai_path(&self) -> PathBuf {
        self.app.join("accounts/openai.json")
    }

    fn read_openai(&self) -> io::Result<Option<OpenAi>> {
        let directory = self.app.join("accounts");
        match check_dir(&directory) {
            Ok(()) => read(&self.openai_path()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn chatgpt(&self) -> Value {
        let result = self.read_openai().and_then(|account| match account {
            Some(account) if account.version != 1 => {
                Err(invalid("unsupported ChatGPT account version"))
            }
            other => Ok(other),
        });
        let mut reported = self
            .reported_openai_error
            .lock()
            .expect("account status lock");
        match result {
            Ok(account) => {
                *reported = None;
                json!({"signedIn":account.is_some(),"plan":account.map(|a| a.plan),"error":null})
            }
            Err(error) => {
                let message = crate::words::chatgpt_account_read_error(&error);
                let show = reported.as_ref() != Some(&message);
                *reported = Some(message.clone());
                json!({"signedIn":false,"plan":null,"error":show.then_some(message)})
            }
        }
    }

    pub fn keep_openai(&self, account: &OpenAi) -> Result<(), CmdError> {
        private_dir(&self.app.join("accounts")).map_err(io_error)?;
        write(&self.openai_path(), account).map_err(io_error)
    }

    pub fn sign_out(&self) -> Result<(), CmdError> {
        let path = self.openai_path();
        match check_dir(&self.app.join("accounts")) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error(error)),
        }
        remove(&path)
            .and_then(|()| remove(&path.with_extension("json.tmp")))
            .map_err(io_error)
    }

    fn judge_directory(&self) -> PathBuf {
        self.app.join("accounts/judge")
    }

    fn check_judge_directory(&self) -> io::Result<()> {
        // An absent judge folder leaves unrelated account settings alone.
        fs::symlink_metadata(self.judge_directory())?;
        check_dir(&self.app.join("accounts"))?;
        check_dir(&self.judge_directory())
    }

    fn judge(&self, identity: &str) -> Option<watane_host::JudgeAccount> {
        let filename = judge_filename(identity)?;
        self.check_judge_directory().ok()?;
        let stored = read::<Judge>(&self.judge_directory().join(filename)).ok()??;
        if stored.version != 1 || !judge_key_valid(&stored.key) {
            return None;
        }
        let origin = watane_host::judge_origin(&stored.origin).ok()?;
        Some(watane_host::JudgeAccount {
            key: stored.key,
            origin,
        })
    }

    pub fn kept_judge_origin(&self, identity: &str) -> Option<String> {
        self.judge(identity).map(|account| account.origin)
    }

    pub fn keep_judge(&self, identity: &str, key: &str, origin: &str) -> Result<(), CmdError> {
        let key = key.trim();
        if !judge_key_valid(key) {
            return Err(CmdError::new("bad_input", "Enter a key of up to 4 KiB."));
        }
        let filename = judge_filename(identity)
            .ok_or_else(|| CmdError::new("bad_input", "Invalid provider identity."))?;
        let directory = self.judge_directory();
        private_dir(&self.app.join("accounts")).map_err(io_error)?;
        private_dir(&directory).map_err(io_error)?;
        write(
            &directory.join(filename),
            &Judge {
                version: 1,
                key: key.into(),
                origin: origin.into(),
            },
        )
        .map_err(io_error)
    }

    pub fn save_judge(
        &self,
        host: &watane_host::Host,
        provider: String,
        key: &str,
        origin: &str,
    ) -> Result<(), CmdError> {
        let replacing = self.kept_judge_origin(&provider).is_some();
        self.keep_judge(&provider, key, origin)?;
        if replacing {
            host.account_changed(&watane_host::Account::Judge { provider });
        }
        Ok(())
    }

    fn remove_judge(&self, identity: &str) -> Result<(), CmdError> {
        let filename = judge_filename(identity)
            .ok_or_else(|| CmdError::new("bad_input", "Invalid provider identity."))?;
        match self.check_judge_directory() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error(error)),
        }
        let path = self.judge_directory().join(filename);
        remove(&path.with_extension("json.tmp"))
            .and_then(|()| remove(&path))
            .map_err(io_error)
    }

    pub fn revoke_judge(&self, host: &watane_host::Host, provider: String) -> Result<(), CmdError> {
        self.remove_judge(&provider)?;
        host.account_changed(&watane_host::Account::Judge { provider });
        Ok(())
    }

    pub fn prune_judges<'a>(
        &self,
        installed: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), CmdError> {
        match self.check_judge_directory() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error(error)),
        }
        let kept: HashSet<_> = installed.into_iter().filter_map(judge_filename).collect();
        for entry in fs::read_dir(self.judge_directory()).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name();
            if let Some(name) = name.to_str()
                && (name.ends_with(".json") || name.ends_with(".json.tmp"))
                && !kept.contains(name)
                && !entry.file_type().map_err(io_error)?.is_dir()
            {
                remove(&entry.path()).map_err(io_error)?;
            }
        }
        Ok(())
    }

    fn claude_path(&self) -> PathBuf {
        self.app.join("claude.json")
    }

    pub fn claude(&self) -> Option<ClaudeSetting> {
        let mut cached = self.claude.lock().expect("Claude setting lock");
        cached
            .get_or_insert_with(|| {
                let stored = read::<Claude>(&self.claude_path()).ok().flatten()?;
                if stored.version != 1 || !stored.path.is_absolute() {
                    return None;
                }
                Some(match crate::claude_bin::validate(&stored.path) {
                    Ok(checked) => ClaudeSetting::from(&checked),
                    Err(error) => ClaudeSetting {
                        path: stored.path,
                        version: None,
                        error: Some(error.message),
                    },
                })
            })
            .clone()
    }

    pub fn choose_claude(&self, path: &Path) -> Result<ClaudeStatus, CmdError> {
        let checked = crate::claude_bin::validate(path)?;
        let mut cached = self.claude.lock().expect("Claude setting lock");
        write(
            &self.claude_path(),
            &Claude {
                version: 1,
                path: checked.path.clone(),
            },
        )
        .map_err(io_error)?;
        *cached = Some(Some(ClaudeSetting::from(&checked)));
        Ok(checked)
    }

    pub fn clear_claude(&self) -> Result<(), CmdError> {
        let mut cached = self.claude.lock().expect("Claude setting lock");
        remove(&self.claude_path()).map_err(io_error)?;
        *cached = Some(None);
        Ok(())
    }
}

fn io_error(error: io::Error) -> CmdError {
    CmdError::new("io", error.to_string())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    #[cfg(unix)]
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    #[cfg(unix)]
    if !metadata.is_file() {
        return Err(invalid("expected a regular file"));
    }
    #[cfg(unix)]
    let opened = File::open(path);
    #[cfg(windows)]
    let opened = watane_log_windows_fs::open_private_file(path, false);
    let file = match opened {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    #[cfg(unix)]
    let metadata = file.metadata()?;
    #[cfg(unix)]
    if !metadata.is_file() {
        return Err(invalid("expected a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o7177 != 0 {
            return Err(invalid(
                "account settings must have permissions no wider than 0600",
            ));
        }
    }
    // Serde's full error can include the value of a malformed field, including tokens.
    serde_json::from_reader(file)
        .map(Some)
        .map_err(|_| invalid("invalid account settings JSON"))
}

fn private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    let created = {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)
    };
    #[cfg(windows)]
    let created = watane_log_windows_fs::create_dir(path);
    match created {
        Ok(()) => sync_parent(path),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => check_dir(path),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn check_dir(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(invalid("expected the accounts directory"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o7077 != 0 {
            return Err(invalid(
                "accounts directory must have permissions no wider than 0700",
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn check_dir(path: &Path) -> io::Result<()> {
    use watane_log_windows_fs::{self as winfs, FileKind};
    let directory = winfs::open_path(path, false)?;
    if !matches!(winfs::kind(&directory)?, FileKind::Directory) {
        return Err(invalid("expected a plain Windows accounts directory"));
    }
    winfs::check_owner_only(&directory)
}

fn write(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let temp = path.with_extension("json.tmp");
    // An interrupted write can leave a temporary file. Remove the entry
    // before create_new so a link is never followed.
    match fs::remove_file(&temp) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&temp)?
    };
    #[cfg(windows)]
    let mut file = watane_log_windows_fs::create_file(&temp)?;
    let result = (|| {
        serde_json::to_writer(&mut file, value)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        #[cfg(unix)]
        {
            fs::rename(&temp, path)?;
            sync_parent(path)
        }
        #[cfg(windows)]
        watane_log_windows_fs::publish(&temp, path, true)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => sync_parent(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn sync_parent(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(path.parent().expect("account settings have a parent"))?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test fixtures")]
mod tests {
    use super::*;

    #[test]
    fn ac2_judge_storage_uses_exact_identity_and_private_atomic_files() {
        use watane_host::AccountSource;

        let home = tempfile::tempdir().unwrap();
        fs::create_dir(home.path().join("app")).unwrap();
        let accounts = Arc::new(Accounts::new(home.path()));
        let source = Source(Arc::clone(&accounts));
        let path = home.path().join("app/accounts/judge/local__checked.json");
        accounts
            .keep_judge("local/checked", "  test key  ", "https://judge.test")
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            json!({"version":1,"key":"test key","origin":"https://judge.test"})
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            )
        };
        fs::write(path.with_extension("json.tmp"), "interrupted").unwrap();
        assert_eq!(source.judge("local/checked").unwrap().key, "test key");
        for other in [
            "checked",
            "local/Checked",
            "local/check",
            "local/checked-more",
        ] {
            assert!(matches!(
                source.judge(other),
                Err(watane_host::AccountError::NoAccount)
            ));
        }
        let publisher = PublisherKey::from_bytes([1; 32]);
        let other_publisher = PublisherKey::from_bytes([2; 32]);
        let published = format!("{publisher}/checked");
        let other = format!("{other_publisher}/checked");
        accounts
            .keep_judge(&published, "publisher key", "https://judge.test")
            .unwrap();
        assert!(matches!(
            source.judge(&other),
            Err(watane_host::AccountError::NoAccount)
        ));
        accounts
            .keep_judge(&other, "other publisher key", "https://judge.test")
            .unwrap();
        assert_eq!(source.judge(&published).unwrap().key, "publisher key");
        assert_eq!(source.judge(&other).unwrap().key, "other publisher key");
        assert_eq!(source.judge("local/checked").unwrap().key, "test key");
        accounts
            .keep_judge("local/checked", "second key", "https://judge.test:8443")
            .unwrap();
        let replacement = source.judge("local/checked").unwrap();
        assert_eq!(replacement.key, "second key");
        assert_eq!(replacement.origin, "https://judge.test:8443");
        assert!(!path.with_extension("json.tmp").exists());
        assert!(
            accounts
                .keep_judge("local/checked", "  ", "https://judge.test")
                .is_err()
        );
        assert!(
            accounts
                .keep_judge("local/checked", &"x".repeat(4097), "https://judge.test")
                .is_err()
        );
        assert_eq!(source.judge("local/checked").unwrap().key, "second key");
        accounts.remove_judge("local/checked").unwrap();
        accounts.remove_judge("local/checked").unwrap();
        assert!(!path.exists());
        assert!(matches!(
            source.judge("local/checked"),
            Err(watane_host::AccountError::NoAccount)
        ));
    }

    #[test]
    fn ac2_judge_source_rejects_invalid_records_without_a_secret_error() {
        use watane_host::{AccountError, AccountSource};

        let home = tempfile::tempdir().unwrap();
        fs::create_dir(home.path().join("app")).unwrap();
        let accounts = Arc::new(Accounts::new(home.path()));
        accounts
            .keep_judge("local/checked", "test key", "https://judge.test")
            .unwrap();
        let account_path = home.path().join("app/accounts/judge/local__checked.json");
        let source = Source(accounts);
        for invalid in [
            json!({"version":2,"key":"test key","origin":"https://judge.test"}),
            json!({"version":1,"key":"test key","origin":"https://judge.test/path"}),
            json!({"version":1,"key":{"test key":true},"origin":"https://judge.test"}),
        ] {
            fs::write(&account_path, serde_json::to_vec(&invalid).unwrap()).unwrap();
            assert!(matches!(
                source.judge("local/checked"),
                Err(AccountError::NoAccount)
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::write(
                &account_path,
                br#"{"version":1,"key":"test key","origin":"https://judge.test"}"#,
            )
            .unwrap();
            fs::set_permissions(&account_path, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(matches!(
                source.judge("local/checked"),
                Err(AccountError::NoAccount)
            ));
        }
    }

    #[test]
    fn ac4_startup_prunes_absent_identities_and_incomplete_writes() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir(home.path().join("app")).unwrap();
        let accounts = Accounts::new(home.path());
        accounts
            .keep_judge("local/installed", "test key", "https://judge.test")
            .unwrap();
        accounts
            .keep_judge("local/stale", "test key", "https://judge.test")
            .unwrap();
        let directory = home.path().join("app/accounts/judge");
        fs::write(directory.join("local__installed.json.tmp"), b"incomplete").unwrap();
        accounts.prune_judges(["local/installed"]).unwrap();
        assert!(directory.join("local__installed.json").exists());
        assert!(!directory.join("local__stale.json").exists());
        assert!(!directory.join("local__installed.json.tmp").exists());
    }

    #[tokio::test]
    async fn ac3_replacing_and_removing_the_app_key_ends_live_scripted_judge_streams() {
        use std::pin::Pin;
        use std::time::{Duration, Instant};
        use watane_dev::scripted::{Play, ScriptedResponses};
        use watane_host::{
            AccountSource, Answer, Caller, Confirm, Host, InstallRequest, Limits, LocalName, Mode,
            Requester, TestNet,
        };

        struct JudgeConfirm(Arc<Accounts>);
        impl Confirm for JudgeConfirm {
            fn install(&self, _: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
                Box::pin(async { Answer::Approve { withhold: vec![] } })
            }
            fn accounts(&self) -> Option<Arc<dyn AccountSource>> {
                Some(Arc::new(Source(Arc::clone(&self.0))))
            }
        }

        let home = tempfile::tempdir().unwrap();
        let accounts = Arc::new(Accounts::new(home.path()));
        let workspace = tempfile::tempdir().unwrap();
        let source = workspace.path().join("https");
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../host/tests/fixtures/mods/https");
        let local = LocalName::parse("https").unwrap();
        fs::create_dir_all(source.join("wit/deps/watane-host")).unwrap();
        for file in [
            "logic.wasm",
            "wit/world.wit",
            "wit/deps/watane-host/host.wit",
        ] {
            fs::copy(fixture.join(file), source.join(file)).unwrap();
        }
        fs::write(
            source.join("mod.toml"),
            r#"name = "https"
summary = "A scripted judge client."

[http]
hosts = ["judge.test"]
"#,
        )
        .unwrap();
        let host = Host::open(
            home.path(),
            Mode::Home {
                app: Some(Arc::new(JudgeConfirm(Arc::clone(&accounts)))),
            },
            Limits::default(),
        )
        .unwrap();
        fs::create_dir(home.path().join("app")).unwrap();
        let plan = host
            .plan(vec![watane_host::Source::Folder(source)])
            .unwrap();
        host.install(plan, Requester::App).await.unwrap();
        let identity = host.get(&local).unwrap().identity;
        let scripted = ScriptedResponses::start().unwrap();
        scripted.judge().expect_key("test judge key");
        scripted.judge().script(vec![Play::Slow {
            ms: 60_000,
            then: Box::new(Play::Safe(vec![])),
        }]);
        host.set_test_net(TestNet {
            routes: vec![("judge.test".into(), scripted.judge_addr())],
            root_pem: scripted.ca_pem(),
        });
        accounts
            .keep_judge(&identity, "test judge key", &scripted.judge_origin())
            .unwrap();
        let start = || {
            host.call_async(
            Caller::Page, local.clone(), "api.start".into(),
            json!({"request":json!({"url":"https://judge.test/verdict","method":"POST","account":"judge"}).to_string()})
        )
        };
        let until_request = async |index: usize, bearer: &str| {
            let deadline = Instant::now() + Duration::from_secs(20);
            while scripted.judge().count() <= index {
                assert!(
                    Instant::now() < deadline,
                    "judge never received request {index}"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(
                scripted.judge().requests()[index]
                    .headers
                    .iter()
                    .any(|(name, value)| name.eq_ignore_ascii_case("authorization")
                        && value == bearer)
            );
        };
        let until_ended = async |expected: String| {
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                let ended = host
                    .call_async(Caller::Page, local.clone(), "api.ended".into(), Value::Null)
                    .await
                    .unwrap();
                if ended.as_array().unwrap().contains(&json!(&expected)) {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "judge stream outcomes: {ended:?}; wanted {expected}"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };

        let stream = start().await.unwrap();
        until_request(0, "Bearer test judge key").await;
        accounts
            .save_judge(
                &host,
                identity.clone(),
                "new judge key",
                &scripted.judge_origin(),
            )
            .unwrap();
        until_ended(format!("{} gone", stream.as_str().unwrap())).await;

        scripted.judge().expect_key("new judge key");
        let stream = start().await.unwrap();
        until_request(1, "Bearer new judge key").await;
        accounts.revoke_judge(&host, identity.clone()).unwrap();
        assert!(accounts.kept_judge_origin(&identity).is_none());
        until_ended(format!("{} gone", stream.as_str().unwrap())).await;

        let no_key = start().await.unwrap();
        until_ended(format!("{} no-account", no_key.as_str().unwrap())).await;
        assert_eq!(scripted.judge().count(), 2);
    }

    #[tokio::test]
    async fn source_gives_the_stored_claude_path_and_no_token_or_key() {
        use watane_host::{AccountError, AccountSource};
        let home = tempfile::tempdir().unwrap();
        fs::create_dir(home.path().join("app")).unwrap();
        let accounts = Arc::new(Accounts::new(home.path()));
        let source = Source(Arc::clone(&accounts));
        assert_eq!(source.claude_binary(), Err(AccountError::NoAccount));
        accounts
            .keep_openai(&OpenAi {
                version: 1,
                access_token: "private-access".into(),
                refresh_token: "private-refresh".into(),
                expires_at: u64::MAX,
                sub: "alice".into(),
                plan: "plus".into(),
            })
            .unwrap();
        // A path whose file is gone is returned as it is (19.3); the host's spawn fails.
        let gone = home.path().join("gone/claude");
        let stored = Claude {
            version: 1,
            path: gone.clone(),
        };
        write(&accounts.claude_path(), &stored).unwrap();
        let source = Source(Arc::new(Accounts::new(home.path())));
        assert_eq!(source.claude_binary(), Ok(gone));
        assert_eq!(source.openai_token().await, Err(AccountError::NoAccount));
        assert!(matches!(
            source.judge("local/claude"),
            Err(AccountError::NoAccount)
        ));
    }

    #[test]
    fn ac5_account_is_private_and_status_redacts_tokens() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir(home.path().join("app")).unwrap();
        let accounts = Accounts::new(home.path());
        accounts
            .keep_openai(&OpenAi {
                version: 1,
                access_token: "private-access".into(),
                refresh_token: "private-refresh".into(),
                expires_at: 1,
                sub: "alice".into(),
                plan: "plus".into(),
            })
            .unwrap();
        let path = accounts.openai_path();
        fs::write(path.with_extension("json.tmp"), "{").unwrap();
        assert_eq!(
            accounts.chatgpt(),
            json!({"signedIn":true,"plan":"plus","error":null})
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(accounts.chatgpt()["error"].is_string());
            assert_eq!(
                accounts.chatgpt(),
                json!({"signedIn":false,"plan":null,"error":null})
            )
        };
        accounts.sign_out().unwrap();
        assert!(!path.exists());
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn ac5_malformed_or_unknown_account_is_reported_once_without_its_contents() {
        let home = tempfile::tempdir().unwrap();
        let accounts = Accounts::new(home.path());
        fs::create_dir(home.path().join("app")).unwrap();
        private_dir(&accounts.app.join("accounts")).unwrap();
        let path = accounts.openai_path();
        write(
            &path,
            &json!({"version":1,"access_token":{"secret":"never-display"}}),
        )
        .unwrap();
        let status = accounts.chatgpt();
        assert_eq!(status["signedIn"], false);
        assert_eq!(
            status["error"],
            "Could not read the ChatGPT account: invalid account settings JSON"
        );
        assert!(accounts.chatgpt()["error"].is_null());
        write(
            &path,
            &OpenAi {
                version: 2,
                access_token: "secret".into(),
                refresh_token: "secret".into(),
                expires_at: 1,
                sub: "alice".into(),
                plan: "plus".into(),
            },
        )
        .unwrap();
        let status = accounts.chatgpt();
        assert_eq!(status["signedIn"], false);
        assert_eq!(
            status["error"],
            "Could not read the ChatGPT account: unsupported ChatGPT account version"
        );
        assert!(accounts.chatgpt()["error"].is_null());
    }

    #[cfg(unix)]
    #[test]
    fn ac5_absent_or_wide_folder_and_unknown_claude_version() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let accounts = Accounts::new(home.path());
        fs::create_dir(&accounts.app).unwrap();
        let dir = accounts.app.join("accounts");
        let signed_out = json!({"signedIn":false,"plan":null,"error":null});

        assert_eq!(accounts.chatgpt(), signed_out);
        accounts.sign_out().unwrap();
        private_dir(&dir).unwrap();
        assert_eq!(accounts.chatgpt(), signed_out);
        accounts.sign_out().unwrap();

        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(accounts.chatgpt()["error"].is_string());
        assert_eq!(accounts.sign_out().unwrap_err().code, "io");

        write(
            &accounts.claude_path(),
            &Claude {
                version: 2,
                path: home.path().join("claude"),
            },
        )
        .unwrap();
        assert!(accounts.claude().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn ac5_replaces_account_without_following_a_temporary_symlink() {
        use std::os::unix::fs::symlink;
        let home = tempfile::tempdir().unwrap();
        let accounts = Accounts::new(home.path());
        fs::create_dir(&accounts.app).unwrap();
        let mut account = OpenAi {
            version: 1,
            access_token: "first-access".into(),
            refresh_token: "first-refresh".into(),
            expires_at: 1,
            sub: "alice".into(),
            plan: "plus".into(),
        };
        accounts.keep_openai(&account).unwrap();
        assert_eq!(accounts.chatgpt()["plan"], "plus");

        let outside = home.path().join("outside");
        fs::write(&outside, "leave this alone").unwrap();
        let tmp = accounts.openai_path().with_extension("json.tmp");
        symlink(&outside, &tmp).unwrap();

        account.plan = "pro".into();
        accounts.keep_openai(&account).unwrap();
        assert_eq!(accounts.chatgpt()["plan"], "pro");
        assert_eq!(fs::read_to_string(outside).unwrap(), "leave this alone");
        assert!(!tmp.is_symlink());
        assert!(!tmp.exists());
    }

    #[test]
    fn ac3_missing_saved_program_keeps_its_path_and_can_be_cleared() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir(home.path().join("app")).unwrap();
        let accounts = Accounts::new(home.path());
        let missing = home.path().join("removed-claude");
        write(
            &accounts.claude_path(),
            &Claude {
                version: 1,
                path: missing.clone(),
            },
        )
        .unwrap();
        let status = serde_json::to_value(accounts.claude()).unwrap();
        assert_eq!(status["path"], json!(missing));
        assert!(status["version"].is_null());
        assert_eq!(
            status["error"],
            format!("{} is not a program you can run.", missing.display())
        );
        accounts.clear_claude().unwrap();
        assert!(accounts.claude().is_none());
        assert!(!accounts.claude_path().exists());
    }
}
