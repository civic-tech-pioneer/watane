//! Private agent settings under the app home (TECHNICAL 19.1 and 19.3).

use crate::CmdError;
use crate::claude_bin::ClaudeStatus;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

/// What the host reads of the accounts (19.4): the chosen `claude` program. The ChatGPT token and
/// the judge key come with their own stages and are `NoAccount` until then.
pub(crate) struct Source(pub Arc<Accounts>);
impl watane_host::AccountSource for Source {
    fn openai_token(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, watane_host::AccountError>> + Send>,
    > {
        Box::pin(async { Err(watane_host::AccountError::NoAccount) })
    }
    fn judge(&self, _: &str) -> Result<watane_host::JudgeAccount, watane_host::AccountError> {
        Err(watane_host::AccountError::NoAccount)
    }
    fn claude_binary(&self) -> Result<PathBuf, watane_host::AccountError> {
        self.0
            .claude()
            .filter(|c| c.error.is_none())
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
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() {
        return Err(invalid("expected a regular file"));
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
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
    #[cfg(not(unix))]
    let created = fs::create_dir(path);
    match created {
        Ok(()) => sync_parent(path),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => check_dir(path),
        Err(error) => Err(error),
    }
}

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

fn write(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let temp = path.with_extension("json.tmp");
    // An interrupted write can leave a temporary file. Remove the entry
    // before create_new so a link is never followed.
    match fs::remove_file(&temp) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600)
    };
    let mut file = options.open(&temp)?;
    let result = (|| {
        serde_json::to_writer(&mut file, value)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        sync_parent(path)
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
