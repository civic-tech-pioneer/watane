use crate::{
    AppState, CmdError,
    consent::{Answer, AskId, Withheld},
    core::ModView,
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
fn core(state: &AppState) -> Result<&crate::core::Core, CmdError> {
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
pub(crate) fn status(webview: tauri::Webview, state: tauri::State<'_, AppState>) -> Value {
    record(&webview, "status");
    json!({"home":state.home,"port":state.core.get().map_or(state.port, |c| c.port()),"mark":state.mark,"error":state.failed.get(),"identity":null,"site":null})
}
#[tauri::command]
pub(crate) fn mods_list(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModView>, CmdError> {
    record(&webview, "mods_list");
    Ok(core(&state)?.mods())
}
#[tauri::command]
pub(crate) async fn mod_open(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
    local: String,
) -> Result<(), CmdError> {
    record(&webview, "mod_open");
    let m = core(&state)?
        .get(&local)?
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
    let host = core(&state)?.host.clone();
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
    Ok(json!({"installed": installed.into_iter().map(|m| m.local).collect::<Vec<_>>()}))
}
#[tauri::command]
pub(crate) fn tools_list(
    webview: tauri::Webview,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, CmdError> {
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
