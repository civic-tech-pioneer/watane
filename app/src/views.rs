use crate::{AppState, CmdError, consent::Answer};
use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
fn loaded(window: tauri::WebviewWindow, event: tauri::webview::PageLoadPayload<'_>) {
    #[cfg(feature = "driver")]
    window.state::<AppState>().driver.loaded(
        window.label(),
        matches!(event.event(), tauri::webview::PageLoadEvent::Finished),
    );
    #[cfg(not(feature = "driver"))]
    let _ = (window, event);
}
pub(crate) fn main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("watane")
        .inner_size(1000., 700.)
        .on_page_load(loaded)
        .build()?;
    let handle = app.clone();
    w.on_window_event(move |event| {
        if matches!(event, WindowEvent::CloseRequested { .. }) {
            handle.state::<AppState>().broker.deny_all();
            handle.exit(0);
        }
    });
    Ok(())
}
pub(crate) fn open_mod(
    app: &tauri::AppHandle,
    local: &str,
    page: &str,
    title: &str,
) -> Result<(), CmdError> {
    let label = format!("mod-{local}");
    if let Some(w) = app.get_webview_window(&label) {
        w.set_focus()?;
        return Ok(());
    }
    let url: tauri::Url = page
        .parse()
        .map_err(|e| CmdError::new("bad_input", format!("invalid page URL: {e}")))?;
    let origin = url.origin();
    let a = app.clone();
    let l = label.clone();
    let b = app.clone();
    let m = label.clone();
    let c = app.clone();
    let n = label.clone();
    let w = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title(title)
        .inner_size(900., 680.)
        .incognito(true)
        .disable_drag_drop_handler()
        .on_navigation(move |u| {
            let allowed = u.origin() == origin;
            if !allowed {
                blocked(&a, &l, "navigation", u.as_str());
            }
            allowed
        })
        .on_new_window(move |u, _| {
            blocked(&b, &m, "new_window", u.as_str());
            NewWindowResponse::Deny
        })
        .on_download(move |_, event| {
            if let DownloadEvent::Requested { url, .. } = event {
                blocked(&c, &n, "download", url.as_str());
            }
            false
        })
        .on_page_load(loaded)
        .build()?;
    let h = app.clone();
    w.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            unloaded(&h, &label);
        }
    });
    Ok(())
}
fn blocked(app: &tauri::AppHandle, label: &str, kind: &str, url: &str) {
    #[cfg(feature = "driver")]
    app.state::<AppState>().driver.blocked(label, kind, url);
    #[cfg(not(feature = "driver"))]
    let _ = (app, label, kind, url);
}
fn unloaded(app: &tauri::AppHandle, label: &str) {
    #[cfg(feature = "driver")]
    app.state::<AppState>().driver.loaded(label, false);
    #[cfg(not(feature = "driver"))]
    let _ = (app, label);
}
pub(crate) fn close_mod(app: &tauri::AppHandle, local: &str) -> Result<(), CmdError> {
    if let Some(w) = app.get_webview_window(&format!("mod-{local}")) {
        w.close()?;
    }
    Ok(())
}
// Runs on the event loop. A successor is created only after Destroyed, so an
// old window cannot deny its successor or collide with its label.
pub(crate) fn sync_consent(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    if state.failed.get().is_some() {
        state.broker.deny_all();
        return;
    }
    let current = state.broker.current().map(|p| p.0);
    if let Some(w) = app.get_webview_window("consent") {
        if *state.consent_id.lock().unwrap() != current {
            let _ = w.close();
        }
        return;
    }
    let Some(id) = current else {
        return;
    };
    *state.consent_id.lock().unwrap() = Some(id.clone());
    let result = WebviewWindowBuilder::new(app, "consent", WebviewUrl::App("consent.html".into()))
        .title("watane — confirm")
        .inner_size(560., 640.)
        .resizable(false)
        .minimizable(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .focused(true)
        .center()
        .on_page_load(loaded)
        .build();
    match result {
        Ok(w) => {
            let h = app.clone();
            w.on_window_event(move |event| {
                if matches!(event, WindowEvent::Destroyed) {
                    unloaded(&h, "consent");
                    let state = h.state::<AppState>();
                    let _ = state.broker.answer(&id, Answer::Deny);
                    *state.consent_id.lock().unwrap() = None;
                    let handle = h.clone();
                    let _ = h.run_on_main_thread(move || sync_consent(&handle));
                }
            });
            let _ = w.set_focus();
            let _ = w.request_user_attention(Some(tauri::UserAttentionType::Critical));
        }
        Err(e) => {
            eprintln!("cannot open consent: {e}");
            state.broker.deny_all();
        }
    }
}
