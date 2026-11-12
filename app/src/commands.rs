use crate::{
    AppState, CmdError,
    consent::{Answer, AskId, Withheld},
    core::{ModView, ToolRow},
    views,
};
use serde_json::{Value, json};
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
    let mut value = json!({"home":state.home,"port":state.core.get().map_or(state.port, |c| c.port()),"mark":state.mark,"error":state.failed.get(),"identity":null,"site":null});
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
    let core = core(&state)?.clone();
    blocking(move || core.mods()).await
}
#[tauri::command]
pub(crate) async fn mod_open(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    local: String,
) -> Result<(), CmdError> {
    record(&webview, "mod_open");
    let core = core(&state)?.clone();
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
        .map(|p| p.name.unwrap_or(p.fingerprint))
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
) -> Result<(), CmdError> {
    record(&webview, "consent_answer");
    state
        .broker
        .answer(
            &id,
            if approve {
                Answer::Approve { withheld }
            } else {
                Answer::Deny
            },
        )
        .map_err(|e| CmdError::new("refused", e.to_string()))
}

#[tauri::command]
pub(crate) fn mod_enable(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    local: String,
) -> Result<(), CmdError> {
    record(&webview, "mod_enable");
    Ok(core(&state)?
        .host
        .enable(&watane_host::LocalName::parse(&local)?)?)
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
    let core = core(&state)?.clone();
    let host = core.host.clone();
    #[cfg(feature = "driver")]
    let picked = state.driver.pick.lock().unwrap().take();
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
    if paths.is_empty() {
        return Ok(json!({"installed":[]}));
    }
    let planner = host.clone();
    let plan = tauri::async_runtime::spawn_blocking(move || {
        planner.plan(paths.into_iter().map(watane_host::Source::Folder).collect())
    })
    .await
    .map_err(|e| CmdError::new("io", e.to_string()))??;
    let installed = host.install(plan, watane_host::Requester::App).await?;
    core.installed(&installed);
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
            .map(|(name, entries)| json!({"name":name, "entries":entries}))
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
    let served = core.served.lock().unwrap();
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
    let core = core(&state)?.clone();
    let planner = core.clone();
    let host = planner.host.clone();
    let plan = blocking(move || planner.plan_link(&link)).await?;
    let installed = host.install(plan, watane_host::Requester::App).await?;
    core.installed(&installed);
    Ok(json!({"installed":installed.into_iter().map(|m|m.local).collect::<Vec<_>>()}))
}
#[tauri::command]
pub(crate) async fn identity_set_name(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), CmdError> {
    record(&webview, "identity_set_name");
    let core = core(&state)?.clone();
    blocking(move || {
        core.dist.set_name(&name)?;
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
    let core = core(&state)?.clone();
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
            .unwrap()
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
    let dist = core(&state)?.dist.clone();
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
        use std::{
            io::Write,
            os::unix::fs::{OpenOptionsExt, PermissionsExt},
        };
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
    let core = core(&state)?.clone();
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
    let dist = core(&state)?.dist.clone();
    blocking(move || {
        let link = link.map(|s| s.parse::<watane_dist::Link>()).transpose()?;
        dist.confirm_own(link.as_ref())?;
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
    let core = core(&state)?.clone();
    blocking(move || core.channels()).await
}
#[tauri::command]
pub(crate) async fn channel_follow(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    link: String,
) -> Result<crate::core::ChannelRow, CmdError> {
    record(&webview, "channel_follow");
    let core = core(&state)?.clone();
    blocking(move || core.follow(&link)).await
}
#[tauri::command]
pub(crate) async fn channel_unfollow(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    channel: String,
) -> Result<(), CmdError> {
    record(&webview, "channel_unfollow");
    let core = core(&state)?.clone();
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
    let core = core(&state)?.clone();
    let refresher = core.clone();
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
    let installed = core(&state)?.install_release(release).await?;
    Ok(json!({"installed":installed.into_iter().map(|m| m.local).collect::<Vec<_>>()}))
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
    let dist = core(&state)?.dist.clone();
    blocking(move || {
        dist.withdraw(&release, &reason)?;
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
            let expected = error.to_string();
            let converted = CmdError::from(error);
            assert_eq!(converted.message, expected);
        }
    }
}
