//! Deliberately fake consent and installation for local maker-tool tests.
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tiny_http::{Response, Server};
use watane_host::{InstalledMod, LocalName, Ready, RunInfo};
use watane_pkg::Mod;

const USAGE: &str = "Usage: fake-watane --preview <dir> | --home <dir>
  --preview <dir>  Run a fake preview host in <dir>.
  --home <dir>     Run a fake home host with app consent in <dir>.
  -h, --help       Print this help.";

fn installed(folder: &Path, port: u16) -> Result<InstalledMod, Box<dyn std::error::Error>> {
    let m = Mod::read(folder)?;
    let local = LocalName::parse(m.slug().as_str())?;
    Ok(InstalledMod {
        identity: format!("local/{}", m.slug()),
        slug: m.slug().clone(),
        name: m.name().into(),
        summary: m.summary().into(),
        version: m.version().clone(),
        publisher: None,
        package: watane_pkg::pack(folder, false)?.id(),
        release: None,
        origin: m.origin().cloned(),
        enabled: true,
        has_logic: m.logic().is_some(),
        page: m
            .page()
            .map(|_| format!("http://{local}.localhost:{port}/")),
        local,
        grants: vec![],
        binds: vec![],
        links: vec![],
        error: None,
    })
}
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    if args.len() != 2 || (args[0] != "--preview" && args[0] != "--home") {
        eprintln!("{USAGE}");
        std::process::exit(2);
    }
    let mode = if args[0] == "--preview" {
        "preview"
    } else {
        "home"
    };
    let home = PathBuf::from(&args[1]);
    fs::create_dir_all(&home)?;
    let home = home.canonicalize()?;
    let server = Server::http("127.0.0.1:0")?;
    let port = server
        .server_addr()
        .to_ip()
        .ok_or("not a TCP server")?
        .port();
    fs::write(
        home.join("run.json"),
        serde_json::to_vec(&RunInfo {
            watane: 1,
            pid: std::process::id(),
            port,
            mode: mode.into(),
        })?,
    )?;
    let ready = Ready {
        watane: 1,
        mode: mode.into(),
        home: home.clone(),
        port,
        api: format!("http://localhost:{port}/api/"),
        mcp: None,
        mods: vec![],
        token: (mode == "preview").then(|| "wt_fake".into()),
    };
    println!("{}", serde_json::to_string(&ready)?);
    io::stdout().flush()?;
    let mut count = 0;
    for mut request in server.incoming_requests() {
        let path = request.url().to_owned();
        let config: Value = fs::read(home.join("fake.json"))
            .ok()
            .map(|bytes| serde_json::from_slice(&bytes))
            .transpose()?
            .unwrap_or(json!({}));
        let mut releases: Vec<Value> = if let Some(entries) = config.get("releases") {
            serde_json::from_value(entries.clone())?
        } else {
            fs::read(home.join("releases.json"))
                .ok()
                .map(|bytes| serde_json::from_slice(&bytes))
                .transpose()?
                .unwrap_or_default()
        };
        if let Some(link) = config.get("link") {
            for release in &mut releases {
                release["link"] = link.clone();
            }
        }
        if request.method() == &tiny_http::Method::Get && path == "/api/releases" {
            request.respond(Response::from_string(
                json!({"releases":releases}).to_string(),
            ))?;
            continue;
        }
        if request.method() == &tiny_http::Method::Get && path == "/api/status" {
            request.respond(Response::from_string(
                json!({"watane":1,"mode":mode,"home":home,"port":port,"app":mode=="home"})
                    .to_string(),
            ))?;
            continue;
        }
        if request.method() != &tiny_http::Method::Post
            || !matches!(
                path.as_str(),
                "/api/install" | "/api/publish" | "/api/search" | "/api/fetch"
            )
        {
            request.respond(Response::from_string("{}").with_status_code(404))?;
            continue;
        }
        let mut body = Vec::new();
        request.as_reader().read_to_end(&mut body)?;
        count += 1;
        fs::create_dir_all(home.join("requests"))?;
        fs::write(
            home.join("requests")
                .join(format!("{count}-{}", path.trim_start_matches("/api/"))),
            &body,
        )?;
        if path == "/api/fetch" {
            let asked: Value = serde_json::from_slice(&body)?;
            let key = asked["link"]
                .as_str()
                .or_else(|| asked["release"].as_str())
                .unwrap_or_default();
            if let Some(entry) = config["fetch"].get(key) {
                let file = entry["package"]
                    .as_str()
                    .unwrap_or("<missing package path>");
                let bytes = match fs::read(file) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        request.respond(Response::from_string(
                            json!({"error":{"code":"fetch_refused","message":format!("cannot read {file}: {e}")}}).to_string()
                        ).with_status_code(409))?;
                        continue;
                    }
                };
                let id = watane_pkg::PackageId::of(&bytes);
                request.respond(
                    Response::from_data(bytes)
                        .with_header(
                            tiny_http::Header::from_bytes(
                                "Content-Type",
                                "application/octet-stream",
                            )
                            .unwrap(),
                        )
                        .with_header(
                            tiny_http::Header::from_bytes(
                                "Watane-Release",
                                entry["release"].as_str().ok_or("missing release")?,
                            )
                            .unwrap(),
                        )
                        .with_header(
                            tiny_http::Header::from_bytes("Watane-Package", id.to_string())
                                .unwrap(),
                        ),
                )?;
            } else {
                request.respond(
                    Response::from_string(
                        json!({"error":{"code":"fetch_refused","message":"unknown release"}})
                            .to_string(),
                    )
                    .with_status_code(409),
                )?;
            }
            continue;
        }
        if path == "/api/publish"
            && let Some(header) = request
                .headers()
                .iter()
                .find(|h| h.field.equiv("Watane-Provider-Links"))
        {
            fs::write(
                home.join("requests")
                    .join(format!("{count}-provider-links")),
                header.value.as_str(),
            )?;
        }
        let (status, value) = if path == "/api/search" {
            (
                config["search_status"].as_u64().unwrap_or(200) as u16,
                config
                    .get("search")
                    .cloned()
                    .unwrap_or_else(|| json!({"indexes":0,"results":[]})),
            )
        } else if config["answer"] == "decline" {
            (
                403,
                json!({"error":{"code":"refused","message":"declined"}}),
            )
        } else if path == "/api/publish" {
            let unpacked = home.join(format!("publish-{count}"));
            let result =
                watane_pkg::Package::from_bytes(body).and_then(|package| package.unpack(&unpacked));
            let answer = match result {
                Ok(m) => {
                    let seq = releases
                        .iter()
                        .filter_map(|r| r["seq"].as_u64())
                        .max()
                        .unwrap_or(0)
                        + 1;
                    let release = format!("{}/{}@{}", "y".repeat(52), m.slug(), m.version());
                    let link = config
                        .get("link")
                        .cloned()
                        .unwrap_or_else(|| json!(format!("https://fake.invalid/#{}", m.slug())));
                    releases.push(json!({"slug":m.slug(), "version":m.version(), "release":release, "seq":seq, "link":link}));
                    fs::write(home.join("releases.json"), serde_json::to_vec(&releases)?)?;
                    (
                        200,
                        json!({"release":release, "seq":seq, "site":home.join("site"), "link":link, "providers":config.get("providers").cloned().unwrap_or_else(|| json!([]))}),
                    )
                }
                Err(e) => (
                    400,
                    json!({"error":{"code":"invalid_package","message":e.to_string()}}),
                ),
            };
            let _ = fs::remove_dir_all(unpacked);
            answer
        } else {
            let result = (|| -> Result<Vec<InstalledMod>, Box<dyn std::error::Error>> {
                let value: Value = serde_json::from_slice(&body)?;
                let folders: Vec<PathBuf> = serde_json::from_value(value["folders"].clone())?;
                folders
                    .iter()
                    .map(|folder| installed(folder, port))
                    .collect()
            })();
            match result {
                Ok(mods) => (200, json!({"installed":mods})),
                Err(e) => (
                    400,
                    json!({"error":{"code":"invalid","message":e.to_string()}}),
                ),
            }
        };
        request.respond(Response::from_string(value.to_string()).with_status_code(status))?;
    }
    Ok(())
}
