//! Stage A fixture origins and an independent TCP connection oracle.
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::net::{Ipv4Addr, Ipv6Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use tokio::sync::oneshot;
use tower::ServiceExt;
use tower_http::services::ServeDir;

const CSP: &str = "default-src 'self'; script-src 'self'; connect-src 'self'; frame-src 'none'; object-src 'none'";

struct Worker {
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.stop.take().unwrap().send(());
        self.thread
            .take()
            .unwrap()
            .join()
            .expect("fixture worker panicked");
    }
}

pub struct Pages {
    pub port: u16,
    _worker: Worker,
}

impl Pages {
    pub fn start(dir: &Path) -> Pages {
        let ipv4 = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind fixture IPv4");
        let port = ipv4.local_addr().unwrap().port();
        let ipv6 = TcpListener::bind((Ipv6Addr::LOCALHOST, port)).expect("bind fixture IPv6");
        ipv4.set_nonblocking(true).unwrap();
        ipv6.set_nonblocking(true).unwrap();
        let dir = dir.canonicalize().expect("fixture root");
        let (stop, stopped) = oneshot::channel();
        let thread = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let app = Router::new().fallback(page).with_state((dir, port));
                let ipv4 = tokio::net::TcpListener::from_std(ipv4).unwrap();
                let ipv6 = tokio::net::TcpListener::from_std(ipv6).unwrap();
                tokio::select! {
                    result = axum::serve(ipv4, app.clone()) => result.expect("serve fixture IPv4"),
                    result = axum::serve(ipv6, app) => result.expect("serve fixture IPv6"),
                    _ = stopped => {},
                }
            });
        });
        Pages {
            port,
            _worker: Worker {
                stop: Some(stop),
                thread: Some(thread),
            },
        }
    }

    pub fn url(&self, local: &str) -> String {
        format!("http://{local}.localhost:{}/", self.port)
    }
}

async fn page(State((dir, port)): State<(PathBuf, u16)>, request: Request) -> Response {
    let local = request
        .headers()
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .and_then(|host| host.strip_suffix(&format!(".localhost:{port}")))
        .filter(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        });
    let mut response = if let Some(local) = local {
        let folder = dir.join(local);
        if folder.is_dir() {
            let folder = if folder.join("ui").is_dir() {
                folder.join("ui")
            } else {
                folder
            };
            ServeDir::new(folder)
                .oneshot(request)
                .await
                .unwrap()
                .map(Body::new)
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    };
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    response
}

pub struct Listener {
    pub port: u16,
    count: Arc<AtomicUsize>,
    _worker: Worker,
}

impl Listener {
    pub fn start() -> Listener {
        let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind test listener");
        let port = socket.local_addr().unwrap().port();
        socket.set_nonblocking(true).unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let accepted = Arc::clone(&count);
        let (stop, mut stopped) = oneshot::channel();
        let thread = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(socket).unwrap();
                loop {
                    tokio::select! {
                        _ = &mut stopped => break,
                        connection = listener.accept() => {
                            let (mut stream, _) = connection.expect("accept test connection");
                            accepted.fetch_add(1, Ordering::SeqCst);
                            tokio::spawn(async move {
                                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                                // Drain the HTTP headers before closing, avoiding a TCP reset
                                // from unread request bytes. Idle sockets must not block accept.
                                let reply = async {
                                    let mut end = [0; 4];
                                    loop {
                                        end.rotate_left(1);
                                        if stream.read_exact(&mut end[3..]).await.is_err() { return; }
                                        if end == *b"\r\n\r\n" { break; }
                                    }
                                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                                    let _ = stream.shutdown().await;
                                };
                                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), reply).await;
                            });
                        }
                    }
                }
            });
        });
        Listener {
            port,
            count,
            _worker: Worker {
                stop: Some(stop),
                thread: Some(thread),
            },
        }
    }

    pub fn count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}
