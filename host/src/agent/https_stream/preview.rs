//! The loopback transport for a checked scripted request in a dev preview (TECHNICAL 19.9).

use super::{Bytes, End, Full, Job, RequestAccount, ServerName, TokioIo, failed, token_header};
use crate::Mode;
use hyper::header::HeaderValue;

/// The ordinary HTTPS job owns request validation, registration, cancellation and response handling.
pub(super) fn run(job: &Job) -> bool {
    if job.request.account.is_none() || !matches!(job.mode, Mode::Preview) {
        return false;
    }
    let Some(host) = job.weak.upgrade() else {
        return true;
    };
    let Some(fake) = host.home.scripted_openai.get() else {
        return false;
    };
    let (address, authorization) = match job.request.account {
        Some(RequestAccount::OpenAi) => (fake.address, token_header(&fake.token)),
        Some(RequestAccount::Judge(_)) => {
            let Some((address, _)) = &fake.judge else {
                return false;
            };
            let authorization = job
                .judge_authorization
                .clone()
                .unwrap_or_else(|| Err(failed("scripted judge has no checked authorization")));
            (*address, authorization.map_err(End::With))
        }
        None => return false,
    };
    let connector = fake.connector.clone();
    // Do not keep the host alive for the duration of a blocked stream.
    drop(host);

    let outcome = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(exchange(job, address, connector, authorization)),
        Err(_) => Err(End::With(failed("cannot start a runtime for https-stream"))),
    };
    match outcome {
        Ok(value) | Err(End::With(value)) => job.hub.finish(&job.id, value),
        Err(End::Stop) => {}
    }
    true
}

async fn exchange(
    job: &Job,
    address: std::net::SocketAddrV4,
    connector: tokio_rustls::TlsConnector,
    authorization: Result<HeaderValue, End>,
) -> Result<serde_json::Value, End> {
    if job.stopped() {
        return Err(End::Stop);
    }
    let authorization = authorization?;
    let account = match job.request.account {
        Some(RequestAccount::Judge(_)) => "scripted judge",
        _ => "scripted OpenAI",
    };
    let connect = async {
        let tcp = tokio::net::TcpStream::connect(address)
            .await
            .map_err(|err| format!("cannot connect to {account}: {err}"))?;
        let _ = tcp.set_nodelay(true);
        let name = ServerName::try_from(job.request.name.clone())
            .map_err(|_| format!("invalid {account} TLS name"))?;
        connector
            .connect(name, tcp)
            .await
            .map_err(|err| format!("{account} TLS failed: {err}"))
    };
    let tls = job
        .within(connect)
        .await?
        .map_err(|err| End::With(failed(err)))?;
    let (mut sender, connection) = job
        .within(hyper::client::conn::http1::handshake::<_, Full<Bytes>>(
            TokioIo::new(tls),
        ))
        .await?
        .map_err(|err| End::With(failed(format!("HTTP: {err}"))))?;
    let driver = tokio::spawn(async move {
        let _ = connection.await;
    });
    let result = job.exchange(&mut sender, Some(authorization)).await;
    driver.abort();
    result
}
