use crate::consent::Answer;
use crate::{AppState, CmdError};
use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
pub const DROP_ENTER_JS: &str = "document.body.classList.add('dropping')";
pub const DROP_LEAVE_JS: &str = "document.body.classList.remove('dropping')";

// Apply last, so a consent builder's focused(true) cannot override quiet mode.
fn screen_builder<'a>(
    app: &tauri::AppHandle,
    builder: WebviewWindowBuilder<'a, tauri::Wry, tauri::AppHandle>,
) -> WebviewWindowBuilder<'a, tauri::Wry, tauri::AppHandle> {
    #[cfg(feature = "driver")]
    match app.state::<AppState>().screen {
        crate::driver::Screen::Hidden => return builder.visible(false).focused(false),
        crate::driver::Screen::Background => return builder.visible(true).focused(false),
        crate::driver::Screen::Front => {}
    }
    #[cfg(not(feature = "driver"))]
    let _ = app;
    builder
}
fn focus(window: &tauri::WebviewWindow, attention: bool) -> tauri::Result<()> {
    #[cfg(feature = "driver")]
    if window.state::<AppState>().screen != crate::driver::Screen::Front {
        return Ok(());
    }
    let result = window.set_focus();
    if attention {
        let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
    }
    result
}
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
    main_window_at(app, "#mods")
}
fn main_window_at(app: &tauri::AppHandle, hash: &str) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(
        app,
        "main",
        WebviewUrl::App(format!("index.html{hash}").into()),
    )
    .title("watane")
    .inner_size(1000., 700.)
    .on_page_load(loaded);
    let w = screen_builder(app, builder).build()?;
    let drop_window = w.clone();
    let handle = app.clone();
    w.on_window_event(move |event| {
        match event {
            WindowEvent::DragDrop(tauri::DragDropEvent::Enter { .. }) => {
                let _ = drop_window.eval(DROP_ENTER_JS);
            }
            WindowEvent::DragDrop(tauri::DragDropEvent::Leave) => {
                let _ = drop_window.eval(DROP_LEAVE_JS);
            }
            WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                let _ = drop_window.eval(DROP_LEAVE_JS);
                let _ = drop_window.eval(format!(
                    "window.wataneDropped({})",
                    serde_json::json!(paths)
                ));
            }
            _ => {}
        }
        if matches!(event, WindowEvent::Destroyed) {
            unloaded(&handle, "main");
            quit_if_last(&handle, "main");
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
        if app.state::<AppState>().broker.current().is_none() {
            focus(&w, false)?;
        }
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
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title(title)
        .inner_size(900., 680.)
        .incognito(true)
        // tauri-runtime-wry registers Linux protocols once per data_directory key;
        // keep ephemeral mod contexts separate from main/consent's default key.
        .data_directory(app.state::<AppState>().home.join("app/mod-webview-context"))
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
        .on_page_load(loaded);
    let w = screen_builder(app, builder).build()?;
    // WebKitGTK blocks script popups before the create signal by default.
    // In driver builds, exercise our deny hook instead (NOTES S18). The hook
    // above still denies every request; release settings remain untouched.
    #[cfg(all(feature = "driver", target_os = "linux"))]
    w.with_webview(|view| {
        use webkit2gtk::{SettingsExt, WebViewExt};
        view.inner()
            .settings()
            .expect("WebKitGTK settings")
            .set_javascript_can_open_windows_automatically(true);
    })?;
    let h = app.clone();
    w.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            unloaded(&h, &label);
            quit_if_last(&h, &label);
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
        if *state
            .consent_id
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            != current
        {
            let _ = w.close();
        }
        return;
    }
    let Some(id) = current else {
        return;
    };
    *state
        .consent_id
        .lock()
        .expect("lock poisoned: another thread panicked while holding it") = Some(id.clone());
    let builder = WebviewWindowBuilder::new(app, "consent", WebviewUrl::App("consent.html".into()))
        .title("watane — confirm")
        .inner_size(560., 640.)
        .resizable(false)
        .minimizable(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .focused(true)
        .center()
        .on_page_load(loaded);
    let result = screen_builder(app, builder).build();
    match result {
        Ok(w) => {
            let h = app.clone();
            w.on_window_event(move |event| {
                if matches!(event, WindowEvent::Destroyed) {
                    unloaded(&h, "consent");
                    *h.state::<AppState>()
                        .consent_id
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it") = None;
                    // Answer off the main thread: run_on_main_thread called on
                    // the main thread runs at once, which would open the
                    // successor while this window still owns the label.
                    let h = h.clone();
                    let id = id.clone();
                    std::thread::spawn(move || {
                        let _ = h.state::<AppState>().broker.answer(&id, Answer::Deny);
                        let handle = h.clone();
                        let _ = h.run_on_main_thread(move || sync_consent(&handle));
                    });
                }
            });
            let _ = focus(&w, true);
        }
        Err(e) => {
            eprintln!("cannot open consent: {e}");
            state.broker.deny_all();
        }
    }
}

fn quit_if_last(app: &tauri::AppHandle, closed: &str) {
    if !app
        .webview_windows()
        .keys()
        .any(|label| label != "consent" && label != closed)
    {
        app.state::<AppState>().broker.deny_all();
        app.exit(0);
    }
}
pub(crate) fn menu(app: &tauri::AppHandle, id: &str) -> tauri::Result<()> {
    let hash = match id {
        "mods" => "#mods",
        "settings" => "#settings",
        _ => return Ok(()),
    };
    if app.get_webview_window("main").is_none() {
        return main_window_at(app, hash);
    }
    if let Some(window) = app.get_webview_window("main") {
        window.eval(format!("location.hash = {}", serde_json::json!(hash)))?;
        #[cfg(feature = "driver")]
        if app.state::<AppState>().screen == crate::driver::Screen::Front {
            window.show()?;
        }
        #[cfg(not(feature = "driver"))]
        window.show()?;
        focus(&window, false)?;
    }
    Ok(())
}
pub(crate) fn install_menu(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, SubmenuBuilder};
    let submenu = SubmenuBuilder::new(app, "watane")
        .about_with_text("About watane", None)
        .separator()
        .text("mods", "Mods…")
        .text("settings", "Settings…")
        .separator()
        .quit_with_text("Quit watane")
        .build()?;
    for (id, accelerator) in [("mods", "CmdOrCtrl+Shift+M"), ("settings", "CmdOrCtrl+,")] {
        if let Some(item) = submenu.get(id).and_then(|i| i.as_menuitem().cloned()) {
            item.set_accelerator(Some(accelerator))?;
        }
    }
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut_with_text("Cut")
        .copy_with_text("Copy")
        .paste_with_text("Paste")
        .select_all_with_text("Select All")
        .build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize_with_text("Minimize")
        .close_window_with_text("Close Window")
        .build()?;
    app.set_menu(Menu::with_items(app, &[&submenu, &edit, &window])?)?;
    app.on_menu_event(|app, event| {
        if let Err(error) = menu(app, event.id().as_ref()) {
            eprintln!("{error}");
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ac6_drop_scripts_are_constant_class_changes_without_path_formatting() {
        assert_eq!(
            super::DROP_ENTER_JS,
            "document.body.classList.add('dropping')"
        );
        assert_eq!(
            super::DROP_LEAVE_JS,
            "document.body.classList.remove('dropping')"
        );
        for script in [super::DROP_ENTER_JS, super::DROP_LEAVE_JS] {
            assert!(!script.contains(['{', '}']));
        }
    }
}
