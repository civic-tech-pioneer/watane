use crate::{Host, HostError, LocalName, Ready, ReadyMod, RunInfo, api, home, pages};
use axum::{Router, body::Body, extract::State, http::Request, response::Response};
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::{net::TcpListener, sync::watch, task::JoinHandle};

#[derive(Clone)]
pub(crate) struct ServerState {
    pub host: Arc<Host>,
    pub port: u16,
}

#[derive(Debug)]
pub struct Served {
    pub port: u16,
    host: Arc<Host>,
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<std::io::Result<()>>>,
}

impl Host {
    pub async fn serve(self: &Arc<Self>, port: u16) -> Result<Served, HostError> {
        if self.serving.swap(true, Ordering::AcqRel) {
            return Err(HostError::AlreadyServing);
        }
        let result = self.bind_and_serve(port).await;
        if result.is_err() {
            self.serving.store(false, Ordering::Release);
        }
        result
    }

    async fn bind_and_serve(self: &Arc<Self>, requested: u16) -> Result<Served, HostError> {
        let mut listeners = None;
        for _ in 0..8 {
            let v4 = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, requested))
                .await
                .map_err(|_| HostError::PortInUse(requested))?;
            let port = v4.local_addr()?.port();
            match TcpListener::bind((std::net::Ipv6Addr::LOCALHOST, port)).await {
                Ok(v6) => {
                    listeners = Some((v4, v6, port));
                    break;
                }
                Err(_) if requested == 0 => continue,
                Err(_) => return Err(HostError::PortInUse(requested)),
            }
        }
        let (v4, v6, port) = listeners.ok_or(HostError::PortInUse(requested))?;
        let router = Router::new().fallback(dispatch).with_state(ServerState {
            host: self.clone(),
            port,
        });
        home::write_json(
            &self.home().join("run.json"),
            &RunInfo {
                watane: 1,
                pid: std::process::id(),
                port,
                mode: "preview".into(),
            },
        )?;
        let (shutdown, rx) = watch::channel(false);
        let mut tasks = Vec::new();
        for listener in [v4, v6] {
            let router = router.clone();
            let mut rx = rx.clone();
            tasks.push(tokio::spawn(async move {
                axum::serve(listener, router)
                    .with_graceful_shutdown(async move {
                        let _ = rx.changed().await;
                    })
                    .await
            }));
        }
        Ok(Served {
            port,
            host: self.clone(),
            shutdown,
            tasks,
        })
    }
}

impl Served {
    pub fn page_url(&self, local: &LocalName) -> String {
        format!("http://{local}.localhost:{}/", self.port)
    }
    pub fn api_url(&self) -> String {
        format!("http://localhost:{}/api/", self.port)
    }
    pub fn ready(&self) -> Ready {
        let catalog = self
            .host
            .catalog
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ready {
            watane: 1,
            mode: "preview".into(),
            home: self.host.home().into(),
            port: self.port,
            api: self.api_url(),
            mcp: None,
            token: None,
            mods: catalog
                .mods
                .iter()
                .map(|m| ReadyMod {
                    local: m.local.clone(),
                    page: m.page.as_ref().map(|_| self.page_url(&m.local)),
                })
                .collect(),
        }
    }
    pub async fn shutdown(mut self) {
        let _ = self.shutdown.send(true);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
        for task in &mut self.tasks {
            if tokio::time::timeout_at(deadline, &mut *task).await.is_err() {
                task.abort();
                let _ = task.await;
            }
        }
        self.tasks.clear();
    }
    fn clean_run(&self) {
        if let Err(e) = std::fs::remove_file(self.host.home().join("run.json"))
            && e.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("watane: remove run.json: {e}");
        }
        self.host.serving.store(false, Ordering::Release);
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        for task in &self.tasks {
            task.abort();
        }
        self.clean_run();
    }
}

async fn dispatch(State(state): State<ServerState>, request: Request<Body>) -> Response {
    let host = request
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let api_host = [
        format!("localhost:{}", state.port),
        format!("127.0.0.1:{}", state.port),
        format!("[::1]:{}", state.port),
    ]
    .iter()
    .any(|s| s == host);
    if api_host {
        return api::respond(state, request).await;
    }
    if let Some(label) = host.strip_suffix(&format!(".localhost:{}", state.port))
        && let Ok(local) = LocalName::parse(label)
    {
        let method = request.method().clone();
        let path = request.uri().path().to_owned();
        return match tokio::task::spawn_blocking(move || {
            pages::respond(&state.host, &local, &method, &path)
        })
        .await
        {
            Ok(response) => response,
            Err(_) => pages::not_found(),
        };
    }
    pages::plain(421, "unknown host")
}
