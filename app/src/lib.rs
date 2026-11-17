//! The app (TECHNICAL 3): Config, run, AppState, startup and exit.

mod build_on;
#[cfg(all(feature = "driver", not(debug_assertions)))]
compile_error!("the driver feature is only for debug test builds");
pub mod api;
mod commands;
mod confirm;
pub mod consent;
mod core;
#[cfg(feature = "driver")]
mod driver;
mod find;
mod machines;
mod main_mod;
pub mod mark;
pub mod views;
mod words;
use consent::{AskId, Broker};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
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

fn serve_error(error: &watane_host::HostError) -> String {
    match error {
        watane_host::HostError::PortInUse(port) => words::port_in_use(*port),
        other => cause_chain(other),
    }
}

impl From<watane_dist::DistError> for CmdError {
    fn from(e: watane_dist::DistError) -> Self {
        let message = match &e {
            watane_dist::DistError::OwnHeadUnconfirmed => {
                "Confirm the restored own head before signing: in Settings, identity, choose From your machines, or restore again.".into()
            }
            watane_dist::DistError::VersionExists { release, .. } => {
                format!("release {release} already exists")
            }
            _ => cause_chain(&e),
        };
        let code = if matches!(e, watane_dist::DistError::NoIdentity) {
            "bad_input"
        } else {
            "dist"
        };
        Self::new(code, message)
    }
}
impl From<std::io::Error> for CmdError {
    fn from(e: std::io::Error) -> Self {
        Self::new("io", cause_chain(&e))
    }
}
impl From<watane_node::NodeError> for CmdError {
    fn from(error: watane_node::NodeError) -> Self {
        Self::new("net", cause_chain(&error))
    }
}
impl From<watane_node::net::NetError> for CmdError {
    fn from(error: watane_node::net::NetError) -> Self {
        Self::new("net", cause_chain(&error))
    }
}
fn node_options(dist: watane_dist::Options) -> watane_node::Options {
    let mut options = watane_node::Options {
        dist,
        ..Default::default()
    };
    options.net.on = true; // The persisted book decides whether to start an endpoint.
    if cfg!(feature = "driver") {
        options.net.bind = std::env::var("WATANE_DRIVER_NET_BIND")
            .ok()
            .and_then(|v| v.parse().ok());
    }
    options
}

#[derive(Debug)]
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
/// TECHNICAL 10, Step 5: only a driver build started with
/// `WATANE_DRIVER_LISTED_LOCAL=1` lets dist list loopback bases.
fn dist_options(listed_local: Option<&str>) -> watane_dist::Options {
    watane_dist::Options {
        listed_local: cfg!(feature = "driver") && listed_local == Some("1"),
        ..Default::default()
    }
}

#[cfg(feature = "driver")]
fn listed_local_env() -> Option<String> {
    std::env::var("WATANE_DRIVER_LISTED_LOCAL").ok()
}
#[cfg(not(feature = "driver"))]
fn listed_local_env() -> Option<String> {
    None
}

pub fn run(config: Config) -> Result<(), AppError> {
    let broker = Broker::new();
    let parties = Arc::new(words::Parties::new());
    let confirm = Arc::new(confirm::HostConfirm {
        broker: Arc::clone(&broker),
        parties: Arc::clone(&parties),
        host: OnceLock::new(),
        core: OnceLock::new(),
        main: OnceLock::new(),
        notices: Arc::new(confirm::ManagerNotices::default()),
    });
    let host = watane_host::Host::open(
        &config.home,
        watane_host::Mode::Home {
            app: Some(Arc::<confirm::HostConfirm>::clone(&confirm)),
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
        let options = dist_options(listed_local_env().as_deref());
        let confirm = Arc::new(confirm::DistConfirm {
            broker: Arc::clone(&broker),
            parties: Arc::clone(&parties),
            dist: OnceLock::new(),
        });
        tauri::async_runtime::block_on(async move {
            tauri::async_runtime::spawn_blocking(move || {
                let node = Arc::new(watane_node::Node::open(
                    &home,
                    Arc::<confirm::DistConfirm>::clone(&confirm),
                    node_options(options),
                )?);
                let dist = Arc::clone(node.dist());
                let _ = confirm.dist.set(Arc::downgrade(&dist));
                Ok::<_, watane_node::NodeError>(node)
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
            commands::install_paths,
            commands::main_set,
            commands::updates_done,
            commands::tools_list,
            commands::toolsets_list,
            commands::toolset_save,
            commands::toolset_remove,
            commands::tokens_list,
            commands::token_issue,
            commands::token_end,
            commands::install_link,
            commands::release_install,
            commands::release_withdraw,
            commands::identity_set_name,
            commands::site_set_url,
            commands::identity_backup,
            commands::identity_restore,
            commands::identity_confirm_own,
            commands::channels_list,
            commands::channel_follow,
            commands::channel_unfollow,
            commands::channels_refresh,
            commands::indexes_list,
            commands::index_choose,
            commands::index_drop,
            commands::search,
            commands::install_release,
            commands::index_list,
            commands::index_include,
            commands::index_unlist,
            commands::machines,
            commands::machines_on,
            commands::pair_code,
            commands::machine_join,
            commands::machine_remove,
            commands::machine_role,
            commands::machines_expiry,
            commands::relay_add,
            commands::relay_remove,
            commands::dht_set,
            commands::own_hints,
            commands::identity_confirm_hint,
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
                && let (Ok(host), Ok(node)) = (host, dist)
            {
                'startup: {
                    let core = match core::Core::new(
                        h.clone(),
                        Arc::clone(&host),
                        Arc::clone(&node),
                        Arc::clone(&parties),
                        Arc::clone(&state.broker),
                        Arc::clone(&confirm.notices),
                    ) {
                        Ok(core) => Arc::new(core),
                        Err(error) => {
                            let _ = state.failed.set(error.message);
                            break 'startup;
                        }
                    };
                    let _ = confirm.main.set(Arc::clone(&core.main));
                    let _ = confirm.core.set(Arc::downgrade(&core));
                    let initial = Arc::clone(&core);
                    let result = tauri::async_runtime::block_on(async move {
                        tauri::async_runtime::spawn_blocking(move || initial.channels()).await
                    });
                    match result {
                        Ok(Ok(_)) => match tauri::async_runtime::block_on(host.serve(
                            config.port,
                            api::router_node(Some(node), Arc::clone(&parties)),
                        )) {
                            Ok(served) => {
                                *core.served.lock().expect(
                                    "lock poisoned: another thread panicked while holding it",
                                ) = Some(served);
                                let _ = state.core.set(Arc::clone(&core));
                                let weak = Arc::downgrade(&core);
                                tauri::async_runtime::spawn(async move {
                                    let mut timer =
                                        tokio::time::interval(std::time::Duration::from_secs(600));
                                    loop {
                                        timer.tick().await;
                                        let Some(core) = weak.upgrade() else { break };
                                        let _ = tauri::async_runtime::spawn_blocking(move || {
                                            let (rows, updates) = core.refresh()?;
                                            core.spawn_updates(updates);
                                            Ok::<_, CmdError>(rows)
                                        })
                                        .await;
                                    }
                                });
                            }
                            Err(e) => {
                                let message = serve_error(&e);
                                eprintln!("{message}");
                                let _ = state.failed.set(message);
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
            }
            let handle = h.clone();
            state.broker.set_listener(Box::new(move |_| {
                let h = handle.clone();
                let _ = handle.run_on_main_thread(move || views::sync_consent(&h));
            }));
            views::install_menu(&h)?;
            let initial = state
                .core
                .get()
                .and_then(|core| core.main.startup(&core.host));
            if let Some(m) = initial {
                let party = m
                    .publisher
                    .map(|p| words::party_label(&words::party(&p.to_string(), &parties)))
                    .unwrap_or_else(|| "local".into());
                views::open_mod(
                    &h,
                    m.local.as_ref(),
                    m.page
                        .as_deref()
                        .expect("startup returns only mods with a page"),
                    &format!("{} — {party}", m.name),
                )
                .map_err(|e| e.message)?;
            } else {
                views::main_window(&h)?;
            }
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
            let served = core
                .served
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .take();
            if let Some(served) = served {
                tauri::async_runtime::block_on(served.shutdown());
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(feature = "driver")]
    fn ac1_driver_dist_listed_local_only_with_env() {
        assert!(super::dist_options(Some("1")).listed_local);
        assert!(!super::dist_options(None).listed_local);
        assert!(!super::dist_options(Some("0")).listed_local);
        assert!(!super::dist_options(Some("")).listed_local);
    }

    #[test]
    #[cfg(not(feature = "driver"))]
    fn ac1_default_dist_refuses_listed_local() {
        assert!(!super::dist_options(Some("1")).listed_local);
        assert!(!super::dist_options(None).listed_local);
        assert!(super::listed_local_env().is_none());
    }

    #[derive(Debug, thiserror::Error)]
    #[error("a: b")]
    struct Outer(#[source] std::io::Error);

    #[derive(Debug, thiserror::Error)]
    #[error("disk broke")]
    struct Whole(#[source] std::io::Error);

    #[test]
    fn ac1_serve_error_names_busy_port_and_preserves_other_causes() {
        assert_eq!(
            super::serve_error(&watane_host::HostError::PortInUse(47341)),
            "Port 47341 is in use, maybe by another watane app. Start this one with --port <n> or WATANE_PORT=<n> to use another port, or --port 0 for any free port."
        );
        let other = watane_host::HostError::AlreadyServing;
        assert_eq!(super::serve_error(&other), super::cause_chain(&other));
    }
    #[test]
    fn ac5_production_identity_reads_do_not_unwrap_or_expect() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(src).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let mut test_attribute = false;
            let mut test_module = false;
            let mut production = String::new();
            for line in source.lines() {
                if test_attribute && line.starts_with("mod ") {
                    test_module = true;
                }
                if !test_module {
                    production.extend(line.chars().filter(|c| !c.is_whitespace()));
                } else if line == "}" {
                    test_module = false;
                }
                test_attribute = line == "#[cfg(test)]";
            }
            for method in ["identity", "fingerprint"] {
                for panic in ["unwrap", "expect"] {
                    assert!(
                        !production.contains(&format!(".{method}().{panic}(")),
                        "{}: {method}().{panic}() in production",
                        path.display()
                    );
                }
            }
        }
    }
    #[test]
    fn ac3_no_identity_command_is_bad_input_with_dist_text() {
        let error = watane_dist::DistError::NoIdentity;
        let text = error.to_string();
        let mapped = super::CmdError::from(error);
        assert_eq!(mapped.code, "bad_input");
        assert_eq!(mapped.message, text);
    }
    #[test]
    fn ac3_duplicate_version_omits_sequence() {
        let release = format!("{}/ratings@1.0.0", "y".repeat(52)).parse().unwrap();
        assert_eq!(
            super::CmdError::from(watane_dist::DistError::VersionExists { release, seq: 0 })
                .message,
            format!("release {}/ratings@1.0.0 already exists", "y".repeat(52))
        );
    }
    #[test]
    fn ac3_real_dist_causes_print_once_and_urls_are_plain() {
        use watane_dist::DistError;
        let repeated = DistError::Io {
            path: "/site".into(),
            source: std::io::Error::other(DistError::Package(
                watane_host::watane_pkg::PackageError::Io {
                    path: "/cache".into(),
                    source: std::io::Error::other("disk broke"),
                },
            )),
        };
        assert_eq!(super::cause_chain(&repeated), "/site: /cache: disk broke");
        for error in [
            DistError::Package(watane_host::watane_pkg::PackageError::Io {
                path: "/file".into(),
                source: std::io::Error::other("disk broke"),
            }),
            DistError::Network {
                url: "https://example.org/head".into(),
                cause: "connection refused".into(),
            },
            DistError::NotFound {
                url: "https://example.org/head".into(),
            },
        ] {
            let text = super::cause_chain(&error);
            assert_eq!(text, error.to_string());
            assert!(!text.contains(['"', '{', '}']), "{text}");
        }
    }
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
