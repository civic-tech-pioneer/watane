#[cfg(all(feature = "driver", not(debug_assertions)))]
compile_error!("the driver feature is only for debug test builds");
mod commands;
pub mod consent;
mod core;
#[cfg(feature = "driver")]
mod driver;
pub mod mark;
mod views;
use consent::{AskId, Broker};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};
use tauri::Manager;
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("bad argument: {0}")]
    Args(String),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}
#[derive(Debug, serde::Serialize)]
pub struct CmdError {
    pub code: &'static str,
    pub message: String,
}
impl CmdError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl From<tauri::Error> for CmdError {
    fn from(e: tauri::Error) -> Self {
        let mut message = e.to_string();
        let mut cause = std::error::Error::source(&e);
        while let Some(error) = cause {
            message.push_str(": ");
            message.push_str(&error.to_string());
            cause = error.source();
        }
        Self::new("io", message)
    }
}
pub struct Config {
    pub home: PathBuf,
    pub port: u16,
}
impl Config {
    pub fn from_args(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<Self, AppError> {
        let mut home = None;
        let mut port = 0;
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--home") => {
                    home =
                        Some(PathBuf::from(args.next().ok_or_else(|| {
                            AppError::Args("--home needs a directory".into())
                        })?))
                }
                Some("--port") => {
                    port = args
                        .next()
                        .and_then(|v| v.to_str().and_then(|s| s.parse().ok()))
                        .ok_or_else(|| AppError::Args("--port needs a u16".into()))?
                }
                _ => {
                    return Err(AppError::Args(format!(
                        "unknown argument {}",
                        arg.to_string_lossy()
                    )));
                }
            }
        }
        Ok(Self {
            home: home.ok_or_else(|| AppError::Args("--home is required".into()))?,
            port,
        })
    }
}
pub(crate) struct AppState {
    home: PathBuf,
    port: u16,
    mark: Option<String>,
    failed: OnceLock<String>,
    broker: Arc<Broker>,
    core: OnceLock<core::Core>,
    consent_id: Mutex<Option<AskId>>,
    #[cfg(feature = "driver")]
    driver: driver::DriverState,
    #[cfg(feature = "driver")]
    screen: driver::Screen,
}
pub fn run(config: Config) -> Result<(), AppError> {
    let mark = mark::read_or_create(&config.home);
    let failed = OnceLock::new();
    if let Err(e) = &mark {
        let _ = failed.set(format!("cannot open home {}: {e}", config.home.display()));
    }
    let state = AppState {
        home: config.home,
        port: config.port,
        mark: mark.ok(),
        failed,
        broker: Broker::new(),
        core: OnceLock::new(),
        consent_id: Mutex::new(None),
        #[cfg(feature = "driver")]
        driver: Default::default(),
        #[cfg(feature = "driver")]
        screen: driver::Screen::from_env(),
    };
    let builder = tauri::Builder::default();
    #[cfg(all(feature = "driver", target_os = "macos"))]
    let builder = builder.activate_ignoring_other_apps(state.screen == driver::Screen::Front);
    let app = builder
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::mods_list,
            commands::mod_open,
            commands::mod_disable,
            commands::consent_get,
            commands::consent_answer
        ])
        .setup(|app| {
            #[cfg(all(feature = "driver", target_os = "macos"))]
            if app.state::<AppState>().screen != driver::Screen::Front {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            let h = app.handle().clone();
            let state = h.state::<AppState>();
            if state.failed.get().is_none() {
                match core::Core::new(h.clone()) {
                    Ok(core) => {
                        let _ = state.core.set(core);
                    }
                    Err(e) => {
                        let _ = state.failed.set(e.message);
                    }
                }
            }
            let handle = h.clone();
            state.broker.set_listener(Box::new(move |_| {
                let h = handle.clone();
                let _ = handle.run_on_main_thread(move || views::sync_consent(&h));
            }));
            views::main_window(&h)?;
            #[cfg(feature = "driver")]
            driver::start(h);
            Ok(())
        })
        .build(tauri::generate_context!())?;
    app.run(|h, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            h.state::<AppState>().broker.deny_all();
        }
    });
    Ok(())
}
