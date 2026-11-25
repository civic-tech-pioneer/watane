//! The test driver (TECHNICAL 10): drives the windows from the Rust side for
//! window tests. Only with feature `driver`, never in a release build.

use crate::AppState;
use crate::consent::{Answer, Request};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::task::Poll;
use std::time::Duration;
use tauri::Manager;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Screen {
    #[default]
    Hidden,
    Background,
    Front,
}
impl Screen {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("background") => Self::Background,
            Some("front") => Self::Front,
            _ => Self::Hidden,
        }
    }
    pub fn from_env() -> Self {
        Self::parse(std::env::var("WATANE_DRIVER_SCREEN").ok().as_deref())
    }
}
#[derive(Default)]
pub(crate) struct DriverState {
    pub pick: Mutex<Option<Vec<std::path::PathBuf>>>,
    pages: Mutex<HashSet<String>>,
    changed: Condvar,
    commands: Mutex<Vec<Value>>,
    opened_urls: Mutex<std::collections::VecDeque<String>>,
    blocked: Mutex<Vec<Value>>,
}
impl DriverState {
    pub fn loaded(&self, label: &str, loaded: bool) {
        let mut pages = self
            .pages
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if loaded {
            pages.insert(label.into());
        } else {
            pages.remove(label);
        }
        self.changed.notify_all();
    }
    pub fn command(&self, label: &str, cmd: &str) {
        self.commands
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .push(json!({"label":label,"cmd":cmd}));
    }
    pub fn judge_set(&self, label: &str) {
        self.commands
            .lock()
            .expect("driver command lock")
            .push(json!({"label":label,"cmd":"judge_set","args":{"key":"<redacted>"}}));
    }
    pub fn open_url(&self, url: &tauri::Url) {
        self.opened_urls
            .lock()
            .expect("driver browser lock")
            .push_back(url.to_string());
    }
    pub fn blocked(&self, label: &str, kind: &str, url: &str) {
        self.blocked
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .push(json!({"label":label,"kind":kind,"url":url}));
    }
    fn wait(&self, label: &str, timeout: Duration) -> bool {
        let pages = self
            .pages
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let (pages, _) = self
            .changed
            .wait_timeout_while(pages, timeout, |pages| !pages.contains(label))
            .expect("lock poisoned: another thread panicked while holding it");
        pages.contains(label)
    }
}
fn output(value: Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}
fn reply(id: Value, result: Result<Value, String>) {
    output(match result {
        Ok(value) => json!({"id":id,"ok":value}),
        Err(e) => json!({"id":id,"err":e}),
    });
}
fn field<'a>(v: &'a Value, name: &str) -> Result<&'a str, String> {
    v[name].as_str().ok_or_else(|| format!("missing {name}"))
}
// Tauri exposes a policy setter but no getter. Read AppKit's actual policy on
// the main thread, rather than reporting the policy we intended to apply.
#[cfg(target_os = "macos")]
fn accessory(app: &tauri::AppHandle) -> Result<bool, String> {
    use std::ffi::{c_char, c_void};
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        static NSApp: *mut c_void;
    }
    #[link(name = "objc")]
    unsafe extern "C" {
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend(receiver: *mut c_void, selector: *mut c_void) -> isize;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        // SAFETY: setup has initialized NSApp. activationPolicy takes no
        // arguments and returns NSInteger (isize); Accessory's AppKit value is 1.
        let value = unsafe { objc_msgSend(NSApp, sel_registerName(c"activationPolicy".as_ptr())) };
        let _ = tx.send(value == 1);
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(30))
        .map_err(|e| e.to_string())
}
#[cfg(not(target_os = "macos"))]
fn accessory(_: &tauri::AppHandle) -> Result<bool, String> {
    Ok(false)
}
pub(crate) fn start(app: tauri::AppHandle) {
    let ready = app.clone();
    std::thread::spawn(move || {
        let state = ready.state::<AppState>();
        let label = ready
            .webview_windows()
            .into_keys()
            .find(|l| l != "consent")
            .unwrap_or_else(|| "main".into());
        if state.driver.wait(&label, Duration::from_secs(30)) {
            if let Some(cause) = state.failed.get() {
                output(json!({"event":"failed","cause":cause}));
            } else {
                output(
                    json!({"event":"ready","port":state.core.get().map_or(state.port, |c| c.port())}),
                );
            }
        } else {
            output(json!({"event":"failed","cause":"main page load timed out"}));
        }
    });
    std::thread::spawn(move || {
        let pending = Arc::new((Mutex::new(0usize), Condvar::new()));
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else {
                break;
            };
            let v: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    reply(Value::Null, Err(e.to_string()));
                    continue;
                }
            };
            let id = v["id"].clone();
            if v["op"] == "ask" {
                let request: Request = match serde_json::from_value(v["request"].clone()) {
                    Ok(r) => r,
                    Err(e) => {
                        reply(id, Err(e.to_string()));
                        continue;
                    }
                };
                let state = app.state::<AppState>();
                if let Some(cause) = state.failed.get() {
                    reply(id, Err(cause.clone()));
                    continue;
                }
                let broker = Arc::clone(&state.broker);
                let mut ask = Box::pin(async move { broker.ask(request).await });
                // Poll once before reading the next line to preserve stdin FIFO.
                tauri::async_runtime::block_on(std::future::poll_fn(|cx| {
                    assert!(ask.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                }));
                *pending
                    .0
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it") += 1;
                let pending = Arc::clone(&pending);
                tauri::async_runtime::spawn(async move {
                    let answer: Answer = ask.await;
                    reply(
                        id,
                        Ok(serde_json::to_value(answer).expect("a driver answer serializes")),
                    );
                    *pending
                        .0
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it") -= 1;
                    pending.1.notify_all();
                });
            } else if v["op"] == "quit" {
                app.state::<AppState>().broker.deny_all();
                let count = pending
                    .0
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it");
                let _guard = pending
                    .1
                    .wait_while(count, |n| *n > 0)
                    .expect("lock poisoned: another thread panicked while holding it");
                app.exit(0);
                return;
            } else {
                let h = app.clone();
                std::thread::spawn(move || reply(id, dispatch(&h, &v)));
            }
        }
        app.state::<AppState>().broker.deny_all();
        app.exit(0);
    });
}
fn dispatch(app: &tauri::AppHandle, v: &Value) -> Result<Value, String> {
    let state = app.state::<AppState>();
    match field(v, "op")? {
        "theme" => {
            let theme = match v.get("theme") {
                Some(Value::Null) => None,
                Some(Value::String(s)) if s == "light" => Some(tauri::Theme::Light),
                Some(Value::String(s)) if s == "dark" => Some(tauri::Theme::Dark),
                _ => return Err("theme must be light, dark or null".into()),
            };
            for window in app.webview_windows().into_values() {
                window.set_theme(theme).map_err(|e| e.to_string())?;
            }
            Ok(json!(true))
        }
        "menu_items" => {
            use tauri::menu::MenuItemKind;
            let menu = app.menu().ok_or("menu not found")?;
            let rows = menu
                .items()
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter_map(|item| {
                    let MenuItemKind::Submenu(submenu) = item else {
                        return None;
                    };
                    Some((|| -> tauri::Result<Value> {
                        let items = submenu
                            .items()?
                            .into_iter()
                            .map(|item| {
                                let label = match item {
                                    MenuItemKind::MenuItem(i) => i.text()?,
                                    MenuItemKind::Predefined(i) => i.text()?,
                                    MenuItemKind::Submenu(i) => i.text()?,
                                    MenuItemKind::Check(i) => i.text()?,
                                    MenuItemKind::Icon(i) => i.text()?,
                                };
                                Ok(if label.is_empty() {
                                    "-".to_owned()
                                } else {
                                    label
                                })
                            })
                            .collect::<tauri::Result<Vec<_>>>()?;
                        Ok(json!({"menu":submenu.text()?, "items":items}))
                    })())
                })
                .collect::<tauri::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?;
            Ok(json!(rows))
        }
        "menu" => {
            let item = field(v, "item")?.to_owned();
            if !["mods", "settings"].contains(&item.as_str()) {
                return Err("unknown menu item".into());
            }
            let handle = app.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            app.run_on_main_thread(move || {
                let _ = tx.send(crate::views::menu(&handle, &item).map_err(|e| e.to_string()));
            })
            .map_err(|e| e.to_string())?;
            rx.recv_timeout(Duration::from_secs(30))
                .map_err(|e| e.to_string())??;
            Ok(json!(true))
        }
        "open_url" => Ok(json!(
            state
                .driver
                .opened_urls
                .lock()
                .expect("driver browser lock")
                .pop_front()
        )),
        "pick" => {
            let paths: Vec<std::path::PathBuf> =
                serde_json::from_value(v["paths"].clone()).map_err(|e| e.to_string())?;
            *state
                .driver
                .pick
                .lock()
                .expect("lock poisoned: another thread panicked while holding it") = Some(paths);
            Ok(json!(true))
        }
        "wait" => {
            let label = field(v, "label")?;
            let timeout = Duration::from_millis(v["timeoutMs"].as_u64().unwrap_or(30000));
            Ok(json!(
                state.driver.wait(label, timeout) && app.get_webview_window(label).is_some()
            ))
        }
        "eval" => {
            let w = app
                .get_webview_window(field(v, "label")?)
                .ok_or("window not found")?;
            let (tx, rx) = std::sync::mpsc::channel();
            w.eval_with_callback(field(v, "js")?, move |r| {
                let _ = tx.send(r);
            })
            .map_err(|e| e.to_string())?;
            rx.recv_timeout(Duration::from_secs(30))
                .map(Value::String)
                .map_err(|e| e.to_string())
        }
        "screen" => Ok(json!({"mode":state.screen, "accessory": accessory(app)?})),
        "windows" => {
            let windows: Vec<_> = app.webview_windows().into_values().map(|w| {
                json!({
                    "label": w.label(), "url": w.url().ok().map(|u| u.to_string()),
                    "title": w.title().ok(), "alwaysOnTop": w.is_always_on_top().unwrap_or(false),
                    "focused": w.is_focused().unwrap_or(false), "visible": w.is_visible().unwrap_or(false)
                })
            }).collect();
            Ok(json!(windows))
        }
        "close" => {
            let label = field(v, "label")?;
            let w = app.get_webview_window(label).ok_or("window not found")?;
            // Wait for this instance, since its label can immediately be reused
            // by the next consent request.
            let (tx, rx) = std::sync::mpsc::channel();
            w.on_window_event(move |event| {
                if matches!(event, tauri::WindowEvent::Destroyed) {
                    let _ = tx.send(());
                }
            });
            w.close().map_err(|e| e.to_string())?;
            rx.recv_timeout(Duration::from_secs(30))
                .map_err(|e| e.to_string())?;
            Ok(json!(true))
        }
        "log" => Ok(
            json!({"commands":*state.driver.commands.lock().expect("lock poisoned: another thread panicked while holding it"),"blocked":*state.driver.blocked.lock().expect("lock poisoned: another thread panicked while holding it")}),
        ),
        op => Err(format!("unknown driver operation: {op}")),
    }
}

#[cfg(test)]
mod screen_tests {
    use super::*;
    #[test]
    fn ac1_unknown_or_unset_screen_is_hidden() {
        for value in [
            None,
            Some(""),
            Some("unknown"),
            Some("HIDDEN"),
            Some("hidden"),
        ] {
            assert_eq!(Screen::parse(value), Screen::Hidden);
        }
        assert_eq!(Screen::parse(Some("background")), Screen::Background);
        assert_eq!(Screen::parse(Some("front")), Screen::Front);
    }
}
