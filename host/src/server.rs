//! The server (TECHNICAL 8.1): binds the port, routes each request by its `Host`
//! header to a page, the API or MCP, and owns `Served` and `run.json`.

use crate::{Host, HostError, LocalName, Ready, ReadyMod, RunInfo, api, home, pages};
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::response::Response;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub(crate) struct ServerState {
    pub host: Arc<ServingHost>,
    pub port: u16,
    pub shutdown: watch::Receiver<bool>,
}

// All router, request and blocking-task references share this owner. Axum's
// graceful shutdown signal can precede the drop of a connection's router.
#[derive(Debug)]
pub(crate) struct ServingHost {
    // Field order matters: release the host before notifying shutdown.
    host: Arc<Host>,
    _finished: watch::Sender<()>,
}
impl std::ops::Deref for ServingHost {
    type Target = Arc<Host>;
    fn deref(&self) -> &Self::Target {
        &self.host
    }
}

#[derive(Debug)]
pub struct Served {
    pub port: u16,
    host: Arc<Host>,
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<std::io::Result<()>>>,
    finished: watch::Receiver<()>,
}

impl Host {
    pub async fn serve(
        self: &Arc<Self>,
        port: u16,
        extra_api: Router,
    ) -> Result<Served, HostError> {
        if self.serving.swap(true, Ordering::AcqRel) {
            return Err(HostError::AlreadyServing);
        }
        let result = self.bind_and_serve(port, extra_api).await;
        if result.is_err() {
            self.serving.store(false, Ordering::Release);
        }
        result
    }

    async fn bind_and_serve(
        self: &Arc<Self>,
        requested: u16,
        extra_api: Router,
    ) -> Result<Served, HostError> {
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
        let (shutdown, rx) = watch::channel(false);
        let (finished_tx, finished) = watch::channel(());
        let host = Arc::new(ServingHost {
            host: Arc::clone(self),
            _finished: finished_tx,
        });
        let api_state = api::ApiState {
            host: Arc::clone(&host),
            port,
        };
        use axum::routing::MethodFilter as M;
        let methods = M::CONNECT
            .or(M::DELETE)
            .or(M::GET)
            .or(M::HEAD)
            .or(M::OPTIONS)
            .or(M::PATCH)
            .or(M::POST)
            .or(M::PUT)
            .or(M::TRACE);
        let builtin = axum::routing::on(methods, move |request: Request<Body>| {
            let state = api_state.clone();
            async move { api::respond(state, request).await }
        });
        let api = Router::new()
            .route("/api/status", builtin.clone())
            .route("/api/mods", builtin.clone())
            .route("/api/plan", builtin.clone())
            .route("/api/install", builtin)
            .merge(extra_api)
            .fallback(|| async { api::not_found() });
        let router = api.layer(axum::middleware::from_fn_with_state(
            ServerState {
                host,
                port,
                shutdown: rx.clone(),
            },
            dispatch,
        ));
        home::write_json(
            &self.home().join("run.json"),
            &RunInfo {
                watane: 1,
                pid: std::process::id(),
                port,
                mode: self.home.mode.as_str().into(),
            },
        )?;
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
        self.port.store(port, Ordering::Release);
        Ok(Served {
            port,
            host: Arc::clone(self),
            shutdown,
            tasks,
            finished,
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
    pub fn mcp_url(&self, toolset: &crate::ToolsetName) -> String {
        format!("http://localhost:{}/mcp/{toolset}", self.port)
    }
    pub fn ready(&self) -> Ready {
        let catalog = self
            .host
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        Ready {
            watane: 1,
            mode: self.host.home.mode.as_str().into(),
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
        let started = tokio::time::Instant::now();
        let _ = self.shutdown.send(true);
        let deadline = started + Duration::from_secs(4);
        for task in &mut self.tasks {
            if tokio::time::timeout_at(deadline, &mut *task).await.is_err() {
                task.abort();
                let _ = task.await;
            }
        }
        self.tasks.clear();
        // Also wait for detached connection futures and spawn_blocking work.
        // Their shared owner notifies only after releasing its host clone.
        // Active work may retain the home after the whole shutdown bound.
        let _ = tokio::time::timeout_at(started + Duration::from_secs(5), self.finished.changed())
            .await;
    }
    fn clean_run(&self) {
        if let Err(e) = std::fs::remove_file(self.host.home().join("run.json"))
            && e.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("watane: remove run.json: {e}");
        }
        self.host.serving.store(false, Ordering::Release);
        self.host.port.store(0, Ordering::Release);
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

async fn dispatch(
    State(state): State<ServerState>,
    request: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    let mut hosts = request.headers().get_all("host").iter();
    let host = hosts.next().and_then(|v| v.to_str().ok()).unwrap_or("");
    if hosts.next().is_some() {
        return pages::plain(421, "unknown host");
    }
    let api_host = [
        format!("localhost:{}", state.port),
        format!("127.0.0.1:{}", state.port),
        format!("[::1]:{}", state.port),
    ]
    .iter()
    .any(|s| s == host);
    if api_host {
        if request.uri().path().starts_with("/mcp/") {
            return crate::tools::respond(&state.host, request).await;
        }
        if request.headers().contains_key(axum::http::header::ORIGIN) {
            return api::error(403, "forbidden", "Origin is forbidden on the local API");
        }
        if !request.uri().path().starts_with("/api/") {
            return api::not_found();
        }
        return next.run(request).await;
    }
    if let Some(label) = host.strip_suffix(&format!(".localhost:{}", state.port))
        && let Ok(local) = LocalName::parse(label)
    {
        if request.uri().path() == "/call" {
            return pages::call(&state.host, local, state.port, request).await;
        }
        if request.uri().path() == "/events" {
            return pages::events(&state.host, local, state.port, request, state.shutdown);
        }
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
