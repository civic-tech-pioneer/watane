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
    json!({"home":state.home,"port":state.port,"mark":state.mark,"error":state.failed.get(),"identity":null,"site":null})
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
        .get(&local)
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
