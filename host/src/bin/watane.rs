//! The program `watane` (TECHNICAL 12.2): parses the arguments, opens the home
//! or a preview, and serves it until shutdown.

use std::io::Write;
use std::path::PathBuf;
use watane_host::{DEFAULT_PORT, Host, Mode, default_home};

struct Args {
    home: Option<PathBuf>,
    mode: Mode,
    port: u16,
    folders: Vec<PathBuf>,
    fake_claude: Option<PathBuf>,
}

fn arguments() -> Result<Option<Args>, &'static str> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.as_slice() == ["--version"] {
        return Ok(None);
    }
    let mut args = args.into_iter();
    let mut home = None;
    let mut mode = Mode::Home { app: None };
    let mut port = None;
    let mut folders = Vec::new();
    let mut fake_claude = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some(option @ ("--preview" | "--home")) if home.is_none() => {
                mode = if option == "--preview" {
                    Mode::Preview
                } else {
                    Mode::Home { app: None }
                };
                let path = args.next().ok_or("home option needs a directory")?;
                if path.to_str().is_some_and(|s| s.starts_with("--")) {
                    return Err("home option needs a directory, not an option");
                }
                home = Some(PathBuf::from(path));
            }
            Some("--port") if port.is_none() => {
                let value = args.next().ok_or("--port needs a number")?;
                let value = value.to_str().ok_or("port is not UTF-8")?;
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err("port must be 0 to 65535");
                }
                port = Some(
                    value
                        .parse::<u16>()
                        .map_err(|_| "port must be 0 to 65535")?,
                );
            }
            Some("--install") => {
                let path = args.next().ok_or("--install needs a folder")?;
                if path.to_str().is_some_and(|s| s.starts_with("--")) {
                    return Err("--install needs a folder, not an option");
                }
                folders.push(PathBuf::from(path));
            }
            Some("--fake-claude") if fake_claude.is_none() => {
                let path = args.next().ok_or("--fake-claude needs a program")?;
                if path.to_str().is_some_and(|s| s.starts_with("--")) {
                    return Err("--fake-claude needs a program, not an option");
                }
                fake_claude = Some(PathBuf::from(path));
            }
            _ => {
                return Err(
                    "use watane [--home <dir>] [--port <n>], --preview <dir> [--port <n>] [--install <folder>] [--fake-claude <path>], or --version",
                );
            }
        }
    }
    if matches!(mode, Mode::Home { .. }) && !folders.is_empty() {
        return Err("--install requires --preview");
    }
    if matches!(mode, Mode::Home { .. }) && fake_claude.is_some() {
        return Err("--fake-claude requires --preview");
    }
    Ok(Some(Args {
        home,
        mode: mode.clone(),
        port: port.unwrap_or(if matches!(mode, Mode::Preview) {
            0
        } else {
            DEFAULT_PORT
        }),
        folders,
        fake_claude,
    }))
}

#[tokio::main]
async fn main() {
    let args = match arguments() {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!("watane {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Err(message) => {
            println!(
                "{}",
                serde_json::json!({"watane":1,"error":{"code":"usage","message":message}})
            );
            std::process::exit(2);
        }
    };
    if let Err(e) = run(args).await {
        println!(
            "{}",
            serde_json::json!({"watane":1,"error":{"code":e.code(),"message":e.to_string()}})
        );
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<(), watane_host::HostError> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    let home = match args.home {
        Some(home) => home,
        None => default_home()?,
    };
    let host = Host::open(&home, args.mode, watane_host::Limits::default())?;
    if let Some(program) = &args.fake_claude {
        host.set_fake_claude(program)?;
    }
    let served = host.serve(args.port, axum::Router::new()).await?;
    // Initial folders share the API's catalog transaction.
    if !args.folders.is_empty() {
        host.install_folders(&args.folders, served.port)?;
    }
    let mut ready = served.ready();
    if host.is_preview() {
        let preview = watane_host::ToolsetName::parse("preview")?;
        let token = host.issue_token(&preview, &watane_host::Handle::parse("preview")?)?;
        ready.mcp = Some(served.mcp_url(&preview));
        ready.token = Some(token.as_str().into());
    }
    println!("{}", serde_json::to_string(&ready)?);
    std::io::stdout().flush()?;
    tokio::select! { _ = term.recv() => (), _ = interrupt.recv() => () }
    served.shutdown().await;
    Ok(())
}
