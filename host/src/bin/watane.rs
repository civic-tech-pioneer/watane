use std::{io::Write, path::PathBuf};
use watane_host::Host;

struct Args {
    home: PathBuf,
    port: u16,
    folders: Vec<PathBuf>,
}

fn arguments() -> Result<Option<Args>, &'static str> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.as_slice() == ["--version"] {
        return Ok(None);
    }
    let mut args = args.into_iter();
    let mut home = None;
    let mut port = None;
    let mut folders = Vec::new();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--preview") if home.is_none() => {
                let path = args.next().ok_or("--preview needs a directory")?;
                if path.to_str().is_some_and(|s| s.starts_with("--")) {
                    return Err("--preview needs a directory, not an option");
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
            _ => {
                return Err(
                    "use watane --preview <dir> [--port <n>] [--install <folder>] or --version",
                );
            }
        }
    }
    Ok(Some(Args {
        home: home.ok_or("--preview is required in this slice")?,
        port: port.unwrap_or(0),
        folders,
    }))
}

#[tokio::main]
async fn main() {
    let args = match arguments() {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!("watane 0.1.0");
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
    let host = Host::open(&args.home)?;
    let served = host.serve(args.port).await?;
    // Initial folders share the API's catalog transaction.
    if !args.folders.is_empty() {
        host.install_folders(&args.folders, served.port)?;
    }
    println!("{}", serde_json::to_string(&served.ready())?);
    std::io::stdout().flush()?;
    tokio::select! { _ = term.recv() => (), _ = interrupt.recv() => () }
    served.shutdown().await;
    Ok(())
}
