use crate::consent::{AskId, Withheld};
use crate::core::{ModView, ToolRow};
use crate::{AppState, CmdError, views};
use serde_json::{Value, json};
use std::sync::Arc;
use tauri::Manager;
fn record(webview: &tauri::Webview, cmd: &str) {
    #[cfg(feature = "driver")]
    webview
        .state::<AppState>()
        .driver
        .command(webview.label(), cmd);
    #[cfg(not(feature = "driver"))]
    let _ = (webview, cmd);
}
fn core(state: &AppState) -> Result<&std::sync::Arc<crate::core::Core>, CmdError> {
    state.core.get().ok_or_else(|| {
        CmdError::new(
            "unavailable",
            state
                .failed
                .get()
                .map(String::as_str)
                .unwrap_or("home is unavailable"),
        )
    })
}
#[tauri::command]
pub(crate) async fn status(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Value, CmdError> {
    record(&webview, "status");
    let mut value = json!({"home":state.home,"port":state.core.get().map_or(state.port, |c| c.port()),"mark":state.mark,"error":state.failed.get(),"identity":null,"site":null,"checkedAt":state.core.get().and_then(|c| c.checked_at())});
    if let Some(core) = state.core.get().cloned() {
        match blocking(move || core.status()).await {
            Ok((identity, site)) => {
                value["identity"] = identity;
                value["site"] = site;
            }
            Err(e) => value["error"] = json!(e.message),
        }
    }
    Ok(value)
}
#[tauri::command]
pub(crate) async fn mods_list(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModView>, CmdError> {
    record(&webview, "mods_list");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.mods()).await
}
#[tauri::command]
pub(crate) async fn mod_open(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    local: String,
) -> Result<(), CmdError> {
    record(&webview, "mod_open");
    let core = Arc::clone(core(&state)?);
    let name = local.clone();
    let m = blocking(move || core.get(&name))
        .await?
        .filter(|m| m.enabled)
        .ok_or_else(|| CmdError::new("not_found", "mod not found or disabled"))?;
    let page = m
        .page
        .ok_or_else(|| CmdError::new("not_found", "mod has no page"))?;
    let party = m
        .publisher
        .map(|p| crate::words::party_label(&p))
        .unwrap_or_else(|| "local".into());
    views::open_mod(
        webview.app_handle(),
        &local,
        &page,
        &format!("{} — {party}", m.name),
    )
}
#[tauri::command]
pub(crate) async fn mod_disable(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    local: String,
) -> Result<(), CmdError> {
    record(&webview, "mod_disable");
    core(&state)?.disable(&local)
}
#[tauri::command]
pub(crate) fn consent_get(webview: tauri::Webview, state: tauri::State<'_, AppState>) -> Value {
    record(&webview, "consent_get");
    state
        .broker
        .current()
        .map(|(id, request)| json!({"id":id,"mark":state.mark,"request":request}))
        .unwrap_or(Value::Null)
}
#[tauri::command]
pub(crate) fn consent_answer(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    id: AskId,
    approve: bool,
    withheld: Vec<Withheld>,
    choice: Option<String>,
    stop: Option<bool>,
) -> Result<(), CmdError> {
    record(&webview, "consent_answer");
    let stop = stop.unwrap_or(false);
    let result = (|| {
        let (head, request) = state
            .broker
            .current()
            .ok_or(crate::consent::ConsentError::Stale)?;
        if head != id {
            return Err(crate::consent::ConsentError::Stale);
        }
        let answer = request.answer_with_stop(approve, choice, withheld, stop)?;
        state.broker.answer(&id, answer)?;
        Ok::<_, crate::consent::ConsentError>(request)
    })();
    let request = result.map_err(|e| CmdError::new("refused", e.to_string()))?;
    if stop && let Some(asker) = request.asked_by() {
        core(&state)?.grant(&asker.local, "manage", false)?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn mod_enable(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    local: String,
) -> Result<(), CmdError> {
    record(&webview, "mod_enable");
    core(&state)?.enable(&local)
}
#[tauri::command]
pub(crate) fn mod_remove(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    local: String,
    delete_data: bool,
) -> Result<(), CmdError> {
    record(&webview, "mod_remove");
    core(&state)?.remove(&local, delete_data)
}
#[tauri::command]
pub(crate) fn grant_withdraw(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    local: String,
    key: String,
) -> Result<(), CmdError> {
    record(&webview, "grant_withdraw");
    core(&state)?.grant(&local, &key, false)
}
#[tauri::command]
pub(crate) fn grant_give(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    local: String,
    key: String,
) -> Result<(), CmdError> {
    record(&webview, "grant_give");
    core(&state)?.grant(&local, &key, true)
}
#[tauri::command]
pub(crate) async fn install_folders(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Value, CmdError> {
    record(&webview, "install_folders");
    let core = Arc::clone(core(&state)?);
    #[cfg(feature = "driver")]
    let picked = state
        .driver
        .pick
        .lock()
        .expect("lock poisoned: another thread panicked while holding it")
        .take();
    #[cfg(not(feature = "driver"))]
    let picked: Option<Vec<std::path::PathBuf>> = None;
    let paths = if let Some(paths) = picked {
        paths
    } else {
        use tauri_plugin_dialog::DialogExt;
        let app = webview.app_handle().clone();
        tauri::async_runtime::spawn_blocking(move || {
            app.dialog()
                .file()
                .blocking_pick_folders()
                .unwrap_or_default()
                .into_iter()
                .map(|p| {
                    p.into_path()
                        .map_err(|e| CmdError::new("bad_input", e.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|e| CmdError::new("io", e.to_string()))??
    };
    install_selected(core, paths).await
}
#[tauri::command]
pub(crate) async fn install_paths(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    paths: Vec<std::path::PathBuf>,
) -> Result<Value, CmdError> {
    record(&webview, "install_paths");
    install_selected(Arc::clone(core(&state)?), paths).await
}
async fn install_selected(
    core: std::sync::Arc<crate::core::Core>,
    paths: Vec<std::path::PathBuf>,
) -> Result<Value, CmdError> {
    let host = Arc::clone(&core.host);
    if paths.iter().any(|path| !path.is_dir()) {
        return Err(CmdError::new(
            "bad_request",
            "each install path must be a directory",
        ));
    }
    if paths.is_empty() {
        return Ok(json!({"installed":[]}));
    }
    let planner = Arc::clone(&core);
    let plan = tauri::async_runtime::spawn_blocking(move || {
        let plan = planner
            .host
            .plan(paths.into_iter().map(watane_host::Source::Folder).collect())?;
        planner.cache_plan_parties(&plan)?;
        Ok::<_, CmdError>(plan)
    })
    .await
    .map_err(|e| CmdError::new("io", e.to_string()))??;
    let installed = host.install(plan, watane_host::Requester::App).await?;
    core.clear_update_failures(&installed);
    Ok(json!({"installed": installed.into_iter().map(|m| m.local).collect::<Vec<_>>()}))
}
#[tauri::command]
pub(crate) fn tools_list(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ToolRow>, CmdError> {
    record(&webview, "tools_list");
    core(&state)?.tools()
}
#[tauri::command]
pub(crate) fn toolsets_list(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Value, CmdError> {
    record(&webview, "toolsets_list");
    Ok(json!(
        core(&state)?
            .host
            .toolsets()
            .into_iter()
            .map(|(name, entries)| crate::core::toolset_row(&name.to_string(), entries))
            .collect::<Vec<_>>()
    ))
}
#[tauri::command]
pub(crate) fn toolset_save(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    name: String,
    entries: Vec<String>,
) -> Result<(), CmdError> {
    record(&webview, "toolset_save");
    Ok(core(&state)?
        .host
        .set_toolset(&watane_host::ToolsetName::parse(&name)?, entries)?)
}
fn remove_toolset(host: &watane_host::Host, name: &str) -> Result<(), CmdError> {
    Ok(host.remove_toolset(&watane_host::ToolsetName::parse(name)?)?)
}
#[tauri::command]
pub(crate) fn toolset_remove(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), CmdError> {
    record(&webview, "toolset_remove");
    remove_toolset(&core(&state)?.host, &name)
}
#[tauri::command]
pub(crate) fn tokens_list(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Value, CmdError> {
    record(&webview, "tokens_list");
    Ok(json!(core(&state)?.host.tokens()))
}
#[tauri::command]
pub(crate) fn token_issue(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    toolset: String,
    handle: String,
) -> Result<Value, CmdError> {
    record(&webview, "token_issue");
    let core = core(&state)?;
    let toolset = watane_host::ToolsetName::parse(&toolset)?;
    let token = core
        .host
        .issue_token(&toolset, &watane_host::Handle::parse(&handle)?)?;
    let served = core
        .served
        .lock()
        .expect("lock poisoned: another thread panicked while holding it");
    let served = served
        .as_ref()
        .ok_or_else(|| CmdError::new("unavailable", "server stopped"))?;
    Ok(json!({"token":token.as_str(), "mcpUrl":served.mcp_url(&toolset)}))
}
#[tauri::command]
pub(crate) fn token_end(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), CmdError> {
    record(&webview, "token_end");
    let host = &core(&state)?.host;
    let token = host
        .tokens()
        .into_iter()
        .find(|t| serde_json::to_value(&t.id).ok().as_ref() == Some(&json!(id)))
        .ok_or_else(|| CmdError::new("not_found", "token not found"))?;
    if host.end_token(&token.id) {
        Ok(())
    } else {
        Err(CmdError::new("not_found", "token not found"))
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CmdError> + Send + 'static,
) -> Result<T, CmdError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CmdError::new("io", e.to_string()))?
}
#[tauri::command]
pub(crate) async fn install_link(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    link: String,
) -> Result<Value, CmdError> {
    record(&webview, "install_link");
    let core = Arc::clone(core(&state)?);
    let planner = Arc::clone(&core);
    let host = Arc::clone(&planner.host);
    let (plan, fetched) = blocking(move || planner.plan_link(&link)).await?;
    let installed = host.install(plan, watane_host::Requester::App).await?;
    core.clear_update_failures(&installed);
    blocking(move || {
        core.follow_fetched(&fetched);
        Ok(())
    })
    .await?;
    Ok(json!({"installed":installed.into_iter().map(|m|m.local).collect::<Vec<_>>()}))
}
#[tauri::command]
pub(crate) async fn identity_set_name(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), CmdError> {
    record(&webview, "identity_set_name");
    let core = Arc::clone(core(&state)?);
    blocking(move || {
        core.node.set_name(&name)?;
        core.channels()?;
        Ok(())
    })
    .await
}
#[tauri::command]
pub(crate) async fn site_set_url(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<Value, CmdError> {
    record(&webview, "site_set_url");
    let core = Arc::clone(core(&state)?);
    blocking(move || {
        core.dist.set_site_url(&url)?;
        Ok(())
    })
    .await?;
    status(webview, state).await
}
fn picked_file(state: &AppState) -> Option<Option<std::path::PathBuf>> {
    #[cfg(feature = "driver")]
    {
        state
            .driver
            .pick
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .take()
            .map(|p| p.into_iter().next())
    }
    #[cfg(not(feature = "driver"))]
    {
        let _ = state;
        None
    }
}
#[tauri::command]
pub(crate) async fn identity_backup(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    passphrase: String,
) -> Result<Value, CmdError> {
    record(&webview, "identity_backup");
    if passphrase.chars().count() < 12 {
        return Err(CmdError::new(
            "bad_input",
            "passphrase needs at least 12 characters",
        ));
    }
    let dist = Arc::clone(&core(&state)?.dist);
    let picked = picked_file(&state);
    let app = webview.app_handle().clone();
    blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        let path = match picked {
            Some(p) => p,
            None => app
                .dialog()
                .file()
                .set_file_name("watane-identity.backup")
                .blocking_save_file()
                .map(|p| p.into_path())
                .transpose()
                .map_err(|e| CmdError::new("bad_input", e.to_string()))?,
        };
        let Some(path) = path else {
            return Ok(Value::Null);
        };
        let bytes = dist.backup(&passphrase)?;
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&path)?;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        f.write_all(&bytes)?;
        Ok(json!({"path":path}))
    })
    .await
}
#[tauri::command]
pub(crate) async fn identity_restore(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    passphrase: String,
) -> Result<Value, CmdError> {
    record(&webview, "identity_restore");
    let core = Arc::clone(core(&state)?);
    let picked = picked_file(&state);
    let app = webview.app_handle().clone();
    blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        let path = match picked {
            Some(p) => p,
            None => app
                .dialog()
                .file()
                .blocking_pick_file()
                .map(|p| p.into_path())
                .transpose()
                .map_err(|e| CmdError::new("bad_input", e.to_string()))?,
        };
        let Some(path) = path else {
            return Ok(Value::Null);
        };
        let key = core.dist.restore(&std::fs::read(path)?, &passphrase)?;
        core.channels()?;
        Ok(json!({"fingerprint":key.fingerprint()}))
    })
    .await
}
#[tauri::command]
pub(crate) async fn identity_confirm_own(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    link: Option<String>,
) -> Result<Value, CmdError> {
    record(&webview, "identity_confirm_own");
    let node = Arc::clone(&core(&state)?.node);
    blocking(move || {
        let link = link.map(|s| s.parse::<watane_dist::Link>()).transpose()?;
        node.confirm_own(link.as_ref())?;
        Ok(())
    })
    .await?;
    status(webview, state).await
}
#[tauri::command]
pub(crate) async fn channels_list(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::core::ChannelRow>, CmdError> {
    record(&webview, "channels_list");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.channels()).await
}
#[tauri::command]
pub(crate) async fn channel_follow(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    link: String,
) -> Result<crate::core::ChannelRow, CmdError> {
    record(&webview, "channel_follow");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.follow(&link)).await
}
#[tauri::command]
pub(crate) async fn channel_unfollow(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    channel: String,
) -> Result<(), CmdError> {
    record(&webview, "channel_unfollow");
    let core = Arc::clone(core(&state)?);
    blocking(move || {
        let key = channel
            .parse::<watane_dist::PublicKey>()
            .map_err(|e| CmdError::new("bad_input", e.to_string()))?;
        core.dist.unfollow(&key)?;
        core.channels()?;
        Ok(())
    })
    .await
}
#[tauri::command]
pub(crate) async fn channels_refresh(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::core::ChannelRow>, CmdError> {
    record(&webview, "channels_refresh");
    let core = Arc::clone(core(&state)?);
    let refresher = Arc::clone(&core);
    let (rows, updates) = blocking(move || refresher.refresh()).await?;
    core.spawn_updates(updates);
    Ok(rows)
}

fn parse_release(text: &str) -> Result<watane_dist::ReleaseRef, CmdError> {
    text.parse::<watane_dist::ReleaseRef>()
        .map_err(|e| CmdError::new("bad_input", e.to_string()))
}
#[tauri::command]
pub(crate) async fn release_install(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    release: String,
) -> Result<Value, CmdError> {
    record(&webview, "release_install");
    let release = parse_release(&release)?;
    let installed = core(&state)?.install_release(release, None).await?;
    Ok(json!({"installed":installed.into_iter().map(|m| m.local).collect::<Vec<_>>()}))
}
fn withdraw_error(error: watane_dist::DistError) -> CmdError {
    match error {
        watane_dist::DistError::BadReason { cause } => {
            CmdError::new("bad_input", crate::words::bad_reason(cause))
        }
        other => other.into(),
    }
}
#[tauri::command]
pub(crate) async fn release_withdraw(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    release: String,
    reason: String,
) -> Result<(), CmdError> {
    record(&webview, "release_withdraw");
    let release = parse_release(&release)?;
    let node = Arc::clone(&core(&state)?.node);
    blocking(move || {
        node.withdraw(&release, &reason).map_err(withdraw_error)?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod stage_d_tests {
    use super::*;
    #[test]
    fn ac5_release_commands_reject_bad_reference_as_bad_input() {
        for text in ["", "bad", "watane:bad", "publisher/reading-list@invalid"] {
            assert_eq!(parse_release(text).unwrap_err().code, "bad_input");
        }
    }
    #[test]
    fn ac3_withdraw_reason_mapping_and_refusal() {
        struct Refuse;
        impl watane_dist::Confirm for Refuse {
            fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
                panic!("invalid reasons must never ask for consent")
            }
        }
        let home = tempfile::tempdir().unwrap();
        let dist =
            watane_dist::Dist::open(home.path(), std::sync::Arc::new(Refuse), Default::default())
                .unwrap();
        let reference = format!("{}/reading-list@0.1.0", "y".repeat(52))
            .parse()
            .unwrap();
        for (reason, expected) in [
            (String::new(), "Give a reason of 1 to 280 characters."),
            ("x".repeat(281), "Give a reason of 1 to 280 characters."),
            (
                " leading".into(),
                "This reason cannot be used: leading or trailing whitespace refused.",
            ),
        ] {
            let error = withdraw_error(dist.withdraw(&reference, &reason).unwrap_err());
            assert_eq!(error.code, "bad_input");
            assert_eq!(error.message, expected);
        }
        assert_eq!(
            withdraw_error(watane_dist::DistError::BadReason {
                cause: "other cause"
            })
            .message,
            "This reason cannot be used: other cause."
        );
        let error = watane_dist::DistError::Refused;
        let expected = error.to_string();
        assert_eq!(withdraw_error(error).message, expected);
    }
    #[test]
    fn ac6_withdraw_errors_preserve_dist_cause() {
        let reference: watane_dist::ReleaseRef = format!("{}/reading-list@0.1.0", "y".repeat(52))
            .parse()
            .unwrap();
        for error in [
            watane_dist::DistError::BadReason { cause: "empty" },
            watane_dist::DistError::NotOwnRelease {
                release: reference.clone(),
            },
            watane_dist::DistError::AlreadyWithdrawn {
                release: reference,
                seq: 2,
            },
        ] {
            let expected = if matches!(error, watane_dist::DistError::BadReason { .. }) {
                "This reason cannot be used: empty.".to_string()
            } else {
                error.to_string()
            };
            let bad_reason = matches!(error, watane_dist::DistError::BadReason { .. });
            let converted = withdraw_error(error);
            assert_eq!(
                converted.code,
                if bad_reason { "bad_input" } else { "dist" }
            );
            assert_eq!(converted.message, expected);
        }
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    use watane_dist::*;
    #[test]
    fn ac4_index_errors_and_bad_text() {
        for text in ["", "bad", "watane:bad"] {
            assert_eq!(parse_link(text).unwrap_err().code, "bad_input");
            assert_eq!(parse_entry(text).unwrap_err().code, "bad_input");
            assert_eq!(parse_release(text).unwrap_err().code, "bad_input");
        }
        for (publisher, interface) in [(Some("bad"), None), (None, Some("bad"))] {
            let query = crate::find::SearchQuery {
                text: String::new(),
                origin: None,
                publisher: publisher.map(Into::into),
                interface: interface.map(Into::into),
            };
            assert_eq!(query.parse().unwrap_err().code, "bad_input");
        }
        for (cause, expected) in [
            (
                "invalid character count",
                "Give a note of at most 280 characters.",
            ),
            ("other cause", "This note cannot be used: other cause."),
        ] {
            let error = index_error(DistError::BadNote { cause });
            assert_eq!(error.code, "bad_input");
            assert_eq!(error.message, expected);
        }
        let channel: PublicKey = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy"
            .parse()
            .unwrap();
        for error in [
            DistError::OwnListing,
            DistError::AlreadyListed { seq: 2 },
            DistError::NoRelease {
                channel,
                slug: "reading-list".parse().unwrap(),
            },
            DistError::NotIndex { channel },
        ] {
            let expected = if matches!(error, DistError::AlreadyListed { .. }) {
                "already listed".into()
            } else {
                error.to_string()
            };
            let error = index_error(error);
            assert_eq!(error.message, expected);
        }
        struct Refuse;
        impl Confirm for Refuse {
            fn confirm(&self, _: &SignRequest) -> Decision {
                panic!("must not sign")
            }
        }
        let home = tempfile::tempdir().unwrap();
        let dist =
            Dist::open(home.path(), std::sync::Arc::new(Refuse), Default::default()).unwrap();
        dist.set_site_url("https://example.org/").unwrap();
        let error = crate::core::choose_index_with(
            &dist,
            &dist.own_link(None).unwrap().to_string(),
            |_| panic!("own index must be refused before choosing"),
        )
        .unwrap_err();
        assert_eq!(error.code, "bad_input");
        assert_eq!(error.message, "Your own channel is always your index.");
    }
}

pub(crate) fn parse_link(text: &str) -> Result<watane_dist::Link, CmdError> {
    text.parse::<watane_dist::Link>()
        .map_err(|e| CmdError::new("bad_input", e.to_string()))
}
fn parse_entry(text: &str) -> Result<watane_dist::EntryId, CmdError> {
    text.parse::<watane_dist::EntryId>()
        .map_err(|e| CmdError::new("bad_input", e.to_string()))
}
fn index_error(error: watane_dist::DistError) -> CmdError {
    match error {
        watane_dist::DistError::BadNote { cause } => {
            CmdError::new("bad_input", crate::words::bad_note(cause))
        }
        watane_dist::DistError::AlreadyListed { .. } => {
            CmdError::new("dist", crate::words::already_listed(false))
        }
        other => other.into(),
    }
}
#[tauri::command]
pub(crate) async fn indexes_list(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Value>, CmdError> {
    record(&webview, "indexes_list");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.indexes()).await
}
#[tauri::command]
pub(crate) async fn index_choose(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    link: String,
) -> Result<Vec<Value>, CmdError> {
    record(&webview, "index_choose");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.choose_index(&link)).await
}
#[tauri::command]
pub(crate) async fn index_drop(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    channel: String,
) -> Result<(), CmdError> {
    record(&webview, "index_drop");
    let core = Arc::clone(core(&state)?);
    let key = channel
        .parse::<watane_dist::PublicKey>()
        .map_err(|e| CmdError::new("bad_input", e.to_string()))?;
    blocking(move || {
        core.dist.drop_index(&key)?;
        core.channels()?;
        Ok(())
    })
    .await
}
#[tauri::command]
pub(crate) async fn search(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    text: String,
    publisher: Option<String>,
    interface: Option<String>,
    origin: Option<String>,
) -> Result<Vec<Value>, CmdError> {
    record(&webview, "search");
    let query = crate::find::SearchQuery {
        text,
        publisher,
        interface,
        origin,
    }
    .parse()?;
    let core = Arc::clone(core(&state)?);
    blocking(move || core.search(&query)).await
}
#[tauri::command]
pub(crate) async fn install_release(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    release: String,
    link: Option<String>,
) -> Result<Value, CmdError> {
    record(&webview, "install_release");
    let release = parse_release(&release)?;
    let link = link.as_deref().map(parse_link).transpose()?;
    let installed = core(&state)?.install_release(release, link).await?;
    Ok(json!({"installed":installed.into_iter().map(|m|m.local).collect::<Vec<_>>()}))
}
#[tauri::command]
pub(crate) async fn index_list(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    link: String,
    note: String,
) -> Result<(), CmdError> {
    record(&webview, "index_list");
    let link = parse_link(&link)?;
    let core = Arc::clone(core(&state)?);
    blocking(move || {
        core.node.list(&link, &note).map_err(index_error)?;
        core.channels()?;
        Ok(())
    })
    .await
}
#[tauri::command]
pub(crate) async fn index_include(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    link: String,
    note: String,
) -> Result<(), CmdError> {
    record(&webview, "index_include");
    let link = parse_link(&link)?;
    let core = Arc::clone(core(&state)?);
    blocking(move || {
        core.node.include(&link, &note).map_err(|e| match e {
            watane_dist::DistError::AlreadyListed { .. } => {
                CmdError::new("dist", crate::words::already_listed(true))
            }
            other => index_error(other),
        })?;
        core.channels()?;
        Ok(())
    })
    .await
}
#[tauri::command]
pub(crate) async fn index_unlist(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    entry: String,
) -> Result<(), CmdError> {
    record(&webview, "index_unlist");
    let entry = parse_entry(&entry)?;
    let core = Arc::clone(core(&state)?);
    blocking(move || {
        core.node.unlist(&entry)?;
        core.channels()?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod toolset_tests {
    use super::*;
    #[test]
    fn ac1_toolset_remove_unknown_name_keeps_host_code() {
        let home = tempfile::tempdir().unwrap();
        let host =
            watane_host::Host::open(home.path(), watane_host::Mode::Preview, Default::default())
                .unwrap();
        assert_eq!(
            remove_toolset(&host, "missing").unwrap_err().code,
            "no_toolset"
        );
        let name = watane_host::ToolsetName::parse("reader").unwrap();
        host.set_toolset(&name, vec![]).unwrap();
        let token = host
            .issue_token(&name, &watane_host::Handle::parse("agent").unwrap())
            .unwrap();
        remove_toolset(&host, "reader").unwrap();
        assert!(host.toolsets().is_empty());
        assert_eq!(host.tokens().len(), 1);
        drop(token);
    }
}

#[tauri::command]
pub(crate) fn main_set(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    local: Option<String>,
) -> Result<(), CmdError> {
    record(&webview, "main_set");
    let core = core(&state)?;
    core.main.set(&core.host, local.as_deref())
}
#[tauri::command]
pub(crate) fn updates_done(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::core::UpdateDone>, CmdError> {
    record(&webview, "updates_done");
    Ok(core(&state)?.updates_done())
}

#[tauri::command]
pub(crate) async fn machines(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Value, CmdError> {
    record(&webview, "machines");
    let core = Arc::clone(core(&state)?);
    blocking(move || Ok(core.machines_view())).await
}

#[tauri::command]
pub(crate) async fn machines_on(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    on: bool,
) -> Result<Value, CmdError> {
    record(&webview, "machines_on");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.machines_on(on)).await
}

#[tauri::command]
pub(crate) async fn pair_code(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    role: crate::consent::Role,
) -> Result<Value, CmdError> {
    record(&webview, "pair_code");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.pair_code(role)).await
}

#[tauri::command]
pub(crate) async fn machine_join(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    code: String,
    replace: bool,
) -> Result<Value, CmdError> {
    record(&webview, "machine_join");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.machine_join(&code, replace)).await
}

#[tauri::command]
pub(crate) async fn machine_remove(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    node: String,
) -> Result<Value, CmdError> {
    record(&webview, "machine_remove");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.machine_remove(&node)).await
}

#[tauri::command]
pub(crate) async fn machine_role(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    node: String,
    role: crate::consent::Role,
) -> Result<Value, CmdError> {
    record(&webview, "machine_role");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.machine_role(&node, role)).await
}

#[tauri::command]
pub(crate) async fn machines_expiry(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    days: u32,
) -> Result<Value, CmdError> {
    record(&webview, "machines_expiry");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.machines_expiry(days)).await
}

#[tauri::command]
pub(crate) async fn relay_add(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<Value, CmdError> {
    record(&webview, "relay_add");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.relay(&url, true)).await
}

#[tauri::command]
pub(crate) async fn relay_remove(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<Value, CmdError> {
    record(&webview, "relay_remove");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.relay(&url, false)).await
}

#[tauri::command]
pub(crate) async fn dht_set(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    on: bool,
) -> Result<Value, CmdError> {
    record(&webview, "dht_set");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.dht_set(on)).await
}

#[tauri::command]
pub(crate) async fn own_hints(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Value, CmdError> {
    record(&webview, "own_hints");
    let core = Arc::clone(core(&state)?);
    blocking(move || Ok(core.own_hints())).await
}

#[tauri::command]
pub(crate) async fn identity_confirm_hint(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    node: String,
    seq: u64,
    id: String,
) -> Result<Value, CmdError> {
    record(&webview, "identity_confirm_hint");
    let core = Arc::clone(core(&state)?);
    blocking(move || core.identity_confirm_hint(&node, seq, &id)).await?;
    status(webview, state).await
}
