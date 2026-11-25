//! The loopback transport for a checked OpenAI request in a dev preview (TECHNICAL 19.9).

use super::{Bytes, End, Full, Job, ServerName, TokioIo, failed, token_header};
use crate::Mode;

/// The ordinary HTTPS job owns request validation, registration, cancellation and response handling.
pub(super) fn run(job: &Job) -> bool {
    if !matches!(job.request.account, Some(super::RequestAccount::OpenAi))
        || !matches!(job.mode, Mode::Preview)
    {
        return false;
    }
    let Some(host) = job.weak.upgrade() else {
        return true;
    };
    let Some(fake) = host.home.scripted_openai.get() else {
        return false;
    };
    let address = fake.address;
    let connector = fake.connector.clone();
    let token = fake.token.clone();
    // Do not keep the host alive for the duration of a blocked stream.
    drop(host);

    let outcome = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(exchange(job, address, connector, token)),
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
    token: String,
) -> Result<serde_json::Value, End> {
    if job.stopped() {
        return Err(End::Stop);
    }
    let authorization = token_header(&token)?;
    let connect = async {
        let tcp = tokio::net::TcpStream::connect(address)
            .await
            .map_err(|err| format!("cannot connect to scripted OpenAI: {err}"))?;
        let _ = tcp.set_nodelay(true);
        let name = ServerName::try_from(job.request.name.clone())
            .map_err(|_| "invalid scripted OpenAI TLS name".to_owned())?;
        connector
            .connect(name, tcp)
            .await
            .map_err(|err| format!("scripted OpenAI TLS failed: {err}"))
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
