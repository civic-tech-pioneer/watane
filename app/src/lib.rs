#[cfg(all(feature = "driver", not(debug_assertions)))]
compile_error!("the driver feature is only for debug test builds");
pub mod api;
mod commands;
mod confirm;
pub mod consent;
mod core;
#[cfg(feature = "driver")]
mod driver;
pub mod mark;
mod views;
mod words;
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
    Host(#[from] watane_host::HostError),
    #[error(transparent)]
    Dist(#[from] watane_dist::DistError),
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
        Self::new("io", cause_chain(&e))
    }
}
impl From<watane_host::HostError> for CmdError {
    fn from(e: watane_host::HostError) -> Self {
        Self::new(e.code(), cause_chain(&e))
    }
}
pub(crate) fn cause_chain(e: &dyn std::error::Error) -> String {
    let mut message = e.to_string();
    let mut cause = e.source();
    while let Some(e) = cause {
        let text = e.to_string();
        if message != text && !message.ends_with(&format!(": {text}")) {
            message.push_str(": ");
            message.push_str(&text);
        }
        cause = e.source();
    }
    message
}

impl From<watane_dist::DistError> for CmdError {
    fn from(e: watane_dist::DistError) -> Self {
        Self::new("dist", cause_chain(&e))
    }
}
impl From<std::io::Error> for CmdError {
    fn from(e: std::io::Error) -> Self {
        Self::new("io", cause_chain(&e))
    }
}
pub struct Config {
    pub home: PathBuf,
    pub port: u16,
}
impl Config {
    pub fn from_args(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<Self, AppError> {
        let mut home = None;
        let mut port = None;
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--home") => {
                    home =
                        Some(PathBuf::from(args.next().ok_or_else(|| {
                            AppError::Args("--home needs a directory".into())
                        })?))
                }
                Some("--port") => {
                    port = Some(
                        args.next()
                            .and_then(|v| v.to_str().and_then(|s| s.parse().ok()))
                            .ok_or_else(|| AppError::Args("--port needs a u16".into()))?,
                    )
                }
                _ => {
                    return Err(AppError::Args(format!(
                        "unknown argument {}",
                        arg.to_string_lossy()
                    )));
                }
            }
        }
        let port = match (port, std::env::var("WATANE_PORT")) {
            (Some(port), _) => port,
            (None, Ok(text)) => text
                .parse()
                .map_err(|_| AppError::Args("WATANE_PORT needs a u16".into()))?,
            (None, Err(_)) => watane_host::DEFAULT_PORT,
        };
        let home = match home {
            Some(home) => home,
            None => watane_host::default_home()?,
        };
        Ok(Self { home, port })
    }
}
pub(crate) struct AppState {
    home: PathBuf,
    port: u16,
    mark: Option<String>,
    failed: OnceLock<String>,
    broker: Arc<Broker>,
    core: OnceLock<Arc<core::Core>>,
    consent_id: Mutex<Option<AskId>>,
    #[cfg(feature = "driver")]
    driver: driver::DriverState,
    #[cfg(feature = "driver")]
    screen: driver::Screen,
}
pub fn run(config: Config) -> Result<(), AppError> {
    let broker = Broker::new();
    let parties = Arc::new(words::Parties::new());
    let confirm = Arc::new(confirm::HostConfirm {
        broker: broker.clone(),
        parties: parties.clone(),
        host: OnceLock::new(),
    });
    let host = watane_host::Host::open(
        &config.home,
        watane_host::Mode::Home {
            app: Some(confirm.clone()),
        },
        watane_host::Limits::default(),
    );
    if let Ok(host) = &host {
        let _ = confirm.host.set(Arc::downgrade(host));
    }
    let mark = match &host {
        Ok(_) => mark::read_or_create(&config.home).map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    let dist = if mark.is_ok() {
        let home = config.home.clone();
        let confirm = Arc::new(confirm::DistConfirm {
            broker: broker.clone(),
            parties: parties.clone(),
        });
        tauri::async_runtime::block_on(async move {
            tauri::async_runtime::spawn_blocking(move || {
                watane_dist::Dist::open(&home, confirm, Default::default()).map(Arc::new)
            })
            .await
        })
        .map_err(|e| e.to_string())
        .and_then(|r| r.map_err(|e| cause_chain(&e)))
    } else {
        Err("home unavailable".into())
    };
    let failed = OnceLock::new();
    if let Err(e) = &mark {
        let _ = failed.set(format!("cannot open home {}: {e}", config.home.display()));
    }
    if let Err(e) = &dist {
        let _ = failed.set(e.clone());
    }
    let state = AppState {
        home: config.home,
        port: config.port,
        mark: mark.ok(),
        failed,
        broker,
        core: OnceLock::new(),
        consent_id: Mutex::new(None),
        #[cfg(feature = "driver")]
        driver: Default::default(),
        #[cfg(feature = "driver")]
        screen: driver::Screen::from_env(),
    };
    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());
    #[cfg(all(feature = "driver", target_os = "macos"))]
    let builder = builder.activate_ignoring_other_apps(state.screen == driver::Screen::Front);
    let app = builder
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::mods_list,
            commands::mod_open,
            commands::mod_disable,
            commands::mod_enable,
            commands::mod_remove,
            commands::grant_withdraw,
            commands::grant_give,
            commands::install_folders,
            commands::tools_list,
            commands::toolsets_list,
            commands::toolset_save,
            commands::tokens_list,
            commands::token_issue,
            commands::token_end,
            commands::install_link,
            commands::identity_set_name,
            commands::site_set_url,
            commands::identity_backup,
            commands::identity_restore,
            commands::identity_confirm_own,
            commands::channels_list,
            commands::channel_follow,
            commands::channel_unfollow,
            commands::channels_refresh,
            commands::consent_get,
            commands::consent_answer
        ])
        .setup(move |app| {
            #[cfg(all(feature = "driver", target_os = "macos"))]
            if app.state::<AppState>().screen != driver::Screen::Front {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            let h = app.handle().clone();
            let state = h.state::<AppState>();
            if state.failed.get().is_none()
                && let (Ok(host), Ok(dist)) = (host, dist)
            {
                let core = Arc::new(core::Core::new(
                    h.clone(),
                    host.clone(),
                    dist.clone(),
                    parties,
                ));
                let initial = core.clone();
                let result = tauri::async_runtime::block_on(async move {
                    tauri::async_runtime::spawn_blocking(move || initial.channels()).await
                });
                match result {
                    Ok(Ok(_)) => match tauri::async_runtime::block_on(
                        host.serve(config.port, api::router(Some(dist))),
                    ) {
                        Ok(served) => {
                            *core.served.lock().unwrap() = Some(served);
                            let _ = state.core.set(core.clone());
                            let weak = Arc::downgrade(&core);
                            tauri::async_runtime::spawn(async move {
                                let mut timer =
                                    tokio::time::interval(std::time::Duration::from_secs(600));
                                loop {
                                    timer.tick().await;
                                    let Some(core) = weak.upgrade() else { break };
                                    let _ = tauri::async_runtime::spawn_blocking(move || {
                                        core.refresh()
                                    })
                                    .await;
                                }
                            });
                        }
                        Err(e) => {
                            let _ = state.failed.set(cause_chain(&e));
                        }
                    },
                    Ok(Err(e)) => {
                        let _ = state.failed.set(e.message);
                    }
                    Err(e) => {
                        let _ = state.failed.set(e.to_string());
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
        if matches!(event, tauri::RunEvent::Exit)
            && let Some(core) = h.state::<AppState>().core.get()
        {
            let served = core.served.lock().unwrap().take();
            if let Some(served) = served {
                tauri::async_runtime::block_on(served.shutdown());
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    #[derive(Debug, thiserror::Error)]
    #[error("a: b")]
    struct Outer(#[source] std::io::Error);

    #[derive(Debug, thiserror::Error)]
    #[error("disk broke")]
    struct Whole(#[source] std::io::Error);

    #[test]
    fn ac1_cause_chain_appends_only_missing_suffix() {
        assert_eq!(
            super::cause_chain(&Outer(std::io::Error::other("b"))),
            "a: b"
        );
        assert_eq!(
            super::cause_chain(&Outer(std::io::Error::other("c"))),
            "a: b: c"
        );
        assert_eq!(
            super::cause_chain(&Whole(std::io::Error::other("broke"))),
            "disk broke: broke"
        );
    }
}
