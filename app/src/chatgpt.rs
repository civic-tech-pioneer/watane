//! Sign in with ChatGPT through a one-shot native OpenID Connect flow (TECHNICAL 19.1).

use crate::CmdError;
use crate::accounts::OpenAi;
use axum::Router;
use axum::extract::{Query, Request, State};
use axum::http::{Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::{digest, signature};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH};
use subtle::ConstantTimeEq;
use tauri::Url;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

// Q12: the client ID and plan scope await Watane's registration with OpenAI.
const CLIENT_ID: &str = "";
const ISSUER: &str = "https://auth.openai.com";
const SCOPES: &[&str] = &["openid", "offline_access"];
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const ID_TOKEN_LEEWAY: u64 = 60;

struct Provider {
    issuer: Url,
    issuer_id: String,
    client_id: &'static str,
    scopes: &'static [&'static str],
    allow_loopback_http: bool,
}

impl Provider {
    fn configured() -> Result<Self, CmdError> {
        #[cfg(feature = "driver")]
        if let Ok(issuer) = std::env::var("WATANE_APP_OPENAI_ISSUER") {
            return Self::stand_in(&issuer);
        }
        if CLIENT_ID.is_empty() {
            return Err(CmdError::new(
                "unavailable",
                "Sign in with ChatGPT needs Watane’s OpenAI client registration.",
            ));
        }
        Self::new(ISSUER, CLIENT_ID, SCOPES, false)
    }

    fn new(
        issuer: &str,
        client_id: &'static str,
        scopes: &'static [&'static str],
        allow_loopback_http: bool,
    ) -> Result<Self, CmdError> {
        let parsed = Url::parse(issuer)
            .map_err(|_| CmdError::new("bad_input", "The OpenID issuer has an invalid URL."))?;
        let provider = Self {
            issuer: parsed,
            issuer_id: issuer.to_owned(),
            client_id,
            scopes,
            allow_loopback_http,
        };
        if provider.issuer.query().is_some() || !provider.valid_endpoint(&provider.issuer) {
            return Err(CmdError::new(
                "bad_input",
                "The OpenID issuer has an invalid URL.",
            ));
        }
        Ok(provider)
    }

    #[cfg(any(test, feature = "driver"))]
    fn stand_in(issuer: &str) -> Result<Self, CmdError> {
        Self::new(
            issuer,
            "watane-test",
            &["openid", "offline_access", "plan.read"],
            true,
        )
    }

    fn valid_endpoint(&self, url: &Url) -> bool {
        let transport_ok = if self.allow_loopback_http {
            url.scheme() == "http"
                && url.host_str() == Some("127.0.0.1")
                && url.origin() == self.issuer.origin()
        } else {
            url.scheme() == "https" && url.host().is_some()
        };
        transport_ok
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    }

    fn endpoint(&self, text: &str) -> Result<Url, CmdError> {
        Url::parse(text)
            .ok()
            .filter(|url| self.valid_endpoint(url))
            .ok_or_else(|| {
                CmdError::new(
                    "bad_input",
                    "The OpenID issuer supplied an invalid endpoint.",
                )
            })
    }
}

struct Attempt {
    redirect: Url,
    verifier: String,
    state: String,
    nonce: String,
}

impl Attempt {
    fn new(port: u16) -> Result<Self, CmdError> {
        let path = random_hex()?;
        let redirect = Url::parse(&format!("http://127.0.0.1:{port}/{path}"))
            .map_err(|e| CmdError::new("io", e.to_string()))?;
        Ok(Self {
            redirect,
            verifier: URL_SAFE_NO_PAD.encode(random::<32>()?),
            state: random_hex()?,
            nonce: random_hex()?,
        })
    }

    fn authorization_url(&self, provider: &Provider, mut url: Url) -> Result<Url, CmdError> {
        let challenge =
            URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, self.verifier.as_bytes()));
        let scopes = provider.scopes.join(" ");
        let pairs = [
            ("response_type", "code"),
            ("client_id", provider.client_id),
            ("redirect_uri", self.redirect.as_str()),
            ("scope", scopes.as_str()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", self.state.as_str()),
            ("nonce", self.nonce.as_str()),
        ];
        if url
            .query_pairs()
            .any(|(name, _)| pairs.iter().any(|(key, _)| name == *key))
        {
            return Err(CmdError::new(
                "bad_input",
                "The OpenID authorization URL repeats a sign-in parameter.",
            ));
        }
        url.query_pairs_mut().extend_pairs(pairs);
        Ok(url)
    }
}

fn random<const N: usize>() -> Result<[u8; N], CmdError> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|e| CmdError::new("io", e.to_string()))?;
    Ok(bytes)
}

fn random_hex() -> Result<String, CmdError> {
    Ok(random::<16>()?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[derive(Deserialize)]
struct DiscoveryDocument {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
}

struct Discovery {
    authorization: Url,
    token: Url,
    jwks: Url,
}

fn discover(agent: &ureq::Agent, provider: &Provider) -> Result<Discovery, CmdError> {
    let url = format!(
        "{}/.well-known/openid-configuration",
        provider.issuer_id.trim_end_matches('/')
    );
    let document: DiscoveryDocument =
        http_json(agent.get(url.as_str()).call(), "read the OpenID issuer")?;
    if document.issuer != provider.issuer_id {
        return Err(CmdError::new(
            "bad_input",
            "The OpenID discovery issuer does not match.",
        ));
    }
    Ok(Discovery {
        authorization: provider.endpoint(&document.authorization_endpoint)?,
        token: provider.endpoint(&document.token_endpoint)?,
        jwks: provider.endpoint(&document.jwks_uri)?,
    })
}

fn http_json<T: DeserializeOwned>(
    response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    action: &str,
) -> Result<T, CmdError> {
    // The response and transport details can carry credentials. Show a bounded, fixed cause.
    let mut response =
        response.map_err(|_| CmdError::new("unavailable", format!("Could not {action}.")))?;
    if !response.status().is_success() {
        return Err(CmdError::new("unavailable", format!("Could not {action}.")));
    }
    response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_json()
        .map_err(|_| {
            CmdError::new(
                "bad_input",
                format!("The response could not be read to {action}."),
            )
        })
}

#[derive(Deserialize)]
struct CallbackQuery {
    state: String,
    code: Option<String>,
    error: Option<String>,
}

fn callback_code(uri: &Uri, expected: &str) -> Result<String, CmdError> {
    let Query(answer) = Query::<CallbackQuery>::try_from_uri(uri)
        .map_err(|_| CmdError::new("bad_input", "The sign-in answer could not be read."))?;
    if !bool::from(answer.state.as_bytes().ct_eq(expected.as_bytes())) {
        return Err(CmdError::new(
            "bad_input",
            "The sign-in state did not match.",
        ));
    }
    if let Some(error) = answer.error {
        let error = match error.as_str() {
            "invalid_request"
            | "unauthorized_client"
            | "access_denied"
            | "unsupported_response_type"
            | "invalid_scope"
            | "server_error"
            | "temporarily_unavailable"
            | "interaction_required"
            | "login_required"
            | "account_selection_required"
            | "consent_required"
            | "invalid_request_uri"
            | "invalid_request_object"
            | "request_not_supported"
            | "request_uri_not_supported"
            | "registration_not_supported" => error.as_str(),
            _ => "unknown error",
        };
        return Err(CmdError::new(
            "bad_input",
            format!("sign-in refused: {error}"),
        ));
    }
    answer
        .code
        .filter(|code| !code.is_empty())
        .ok_or_else(|| CmdError::new("bad_input", "The sign-in answer had no authorization code."))
}

struct ListenerState {
    path: String,
    expected_state: String,
    answer: Mutex<Option<oneshot::Sender<Result<String, CmdError>>>>,
}

struct CloseCallbackOnDrop(Arc<ListenerState>);

impl Drop for CloseCallbackOnDrop {
    fn drop(&mut self) {
        // A timed-out caller must close already accepted connections as well as the listener.
        if let Ok(mut answer) = self.0.answer.lock() {
            let _ = answer.take();
        }
    }
}

async fn callback(State(state): State<Arc<ListenerState>>, request: Request) -> Response {
    let reject = || (StatusCode::NOT_FOUND, [(header::CONNECTION, "close")]).into_response();
    if request.uri().path() != state.path {
        return reject();
    }
    let Some(answer) = state
        .answer
        .lock()
        .ok()
        .and_then(|mut answer| answer.take())
    else {
        return reject();
    };
    let result = if request.method() == Method::GET {
        callback_code(request.uri(), &state.expected_state)
    } else {
        Err(CmdError::new(
            "bad_input",
            "The sign-in answer was not a GET request.",
        ))
    };
    let _ = answer.send(result);
    (
        [
            (header::CONNECTION, "close"),
            (header::CACHE_CONTROL, "no-store"),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        "You can return to watane.",
    )
        .into_response()
}

async fn wait_for_callback(listener: TcpListener, attempt: &Attempt) -> Result<String, CmdError> {
    let (answer, receive) = oneshot::channel();
    let (finished, result) = oneshot::channel();
    let state = Arc::new(ListenerState {
        path: attempt.redirect.path().to_owned(),
        expected_state: attempt.state.clone(),
        answer: Mutex::new(Some(answer)),
    });
    let _close = CloseCallbackOnDrop(Arc::clone(&state));
    let app = Router::new().fallback(callback).with_state(state);
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        if let Ok(code) = receive.await {
            let _ = finished.send(code);
        }
    });
    // Drop the listener as soon as a matching request is handled. Axum's connection
    // tasks flush the response and shut down independently of the code exchange.
    tokio::select! {
        biased;
        result = result => result.map_err(|_| listener_closed())?,
        served = server => {
            served.map_err(|e| CmdError::new("io", e.to_string()))?;
            Err(listener_closed())
        }
    }
}

fn listener_closed() -> CmdError {
    CmdError::new("unavailable", "The sign-in listener closed.")
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    id_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct KeySet {
    keys: Vec<Key>,
}

#[derive(Deserialize)]
struct Key {
    kid: Option<String>,
    kty: String,
    alg: Option<String>,
    #[serde(rename = "use")]
    usage: Option<String>,
    key_ops: Option<Vec<String>>,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Deserialize)]
struct JwtHeader {
    alg: String,
    kid: String,
    crit: Option<Vec<String>>,
    b64: Option<bool>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct Claims {
    iss: String,
    aud: Audience,
    azp: Option<String>,
    exp: u64,
    nonce: String,
    sub: String,
    plan: String,
}

fn id_token_error(cause: &str) -> CmdError {
    CmdError::new("bad_input", format!("the ID token did not check: {cause}"))
}

fn decode_claim<T: DeserializeOwned>(encoded: &str, cause: &str) -> Result<T, CmdError> {
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| id_token_error(cause))?;
    serde_json::from_slice(&decoded).map_err(|_| id_token_error(cause))
}

fn get_key(agent: &ureq::Agent, jwks: &Url, kid: &str) -> Result<Key, CmdError> {
    for _ in 0..2 {
        let keys: KeySet = http_json(
            agent.get(jwks.as_str()).call(),
            "read the OpenID signing keys",
        )
        .map_err(|error| {
            CmdError::new(
                error.code,
                format!("the ID token did not check: {}", error.message),
            )
        })?;
        let mut matches = keys
            .keys
            .into_iter()
            .filter(|key| key.kid.as_deref() == Some(kid));
        if let Some(key) = matches.next() {
            if matches.next().is_some() {
                return Err(id_token_error(
                    "more than one signing key has the requested ID",
                ));
            }
            return Ok(key);
        }
    }
    Err(id_token_error("the signing key was not found"))
}

fn check_token(
    agent: &ureq::Agent,
    provider: &Provider,
    jwks: &Url,
    nonce: &str,
    token: &str,
    now: Duration,
) -> Result<Claims, CmdError> {
    let (signed, encoded_signature) = token
        .rsplit_once('.')
        .ok_or_else(|| id_token_error("invalid format"))?;
    let (encoded_header, encoded_claims) = signed
        .split_once('.')
        .ok_or_else(|| id_token_error("invalid format"))?;
    let header: JwtHeader = decode_claim(encoded_header, "invalid header")?;
    if header.alg != "RS256" || header.kid.is_empty() {
        return Err(id_token_error(
            "the token did not name an RS256 signing key",
        ));
    }
    if header.crit.is_some() || header.b64 == Some(false) {
        return Err(id_token_error("the token used an unsupported header"));
    }
    let key = get_key(agent, jwks, &header.kid)?;
    if key.kty != "RSA"
        || key.alg.is_some_and(|alg| alg != "RS256")
        || key.usage.is_some_and(|usage| usage != "sig")
        || key
            .key_ops
            .is_some_and(|ops| !ops.iter().any(|op| op == "verify"))
    {
        return Err(id_token_error(
            "the key is unsuitable for RS256 verification",
        ));
    }
    let decode_key = |value: Option<String>| {
        value
            .and_then(|value| URL_SAFE_NO_PAD.decode(value).ok())
            .ok_or_else(|| id_token_error("invalid signing key"))
    };
    let key = signature::RsaPublicKeyComponents {
        n: decode_key(key.n)?,
        e: decode_key(key.e)?,
    };
    let sig = URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .map_err(|_| id_token_error("invalid signature"))?;
    key.verify(
        &signature::RSA_PKCS1_2048_8192_SHA256,
        signed.as_bytes(),
        &sig,
    )
    .map_err(|_| id_token_error("invalid signature"))?;

    let claims: Claims = decode_claim(encoded_claims, "invalid claims")?;
    if claims.iss != provider.issuer_id {
        return Err(id_token_error("the issuer did not match"));
    }
    let audience_ok = match &claims.aud {
        Audience::One(audience) => audience == provider.client_id,
        // Watane registers a single audience. There are no other trusted audiences.
        Audience::Many(audiences) => audiences.len() == 1 && audiences[0] == provider.client_id,
    };
    if !audience_ok
        || claims
            .azp
            .as_deref()
            .is_some_and(|azp| azp != provider.client_id)
    {
        return Err(id_token_error("the audience did not match"));
    }
    if claims.exp <= now.as_secs().saturating_sub(ID_TOKEN_LEEWAY) {
        return Err(id_token_error("the token expired"));
    }
    if !bool::from(claims.nonce.as_bytes().ct_eq(nonce.as_bytes())) {
        return Err(id_token_error("the nonce did not match"));
    }
    if claims.sub.is_empty() || claims.plan.is_empty() {
        return Err(id_token_error("the subject or plan was empty"));
    }
    Ok(claims)
}

fn clock_error(error: SystemTimeError) -> CmdError {
    CmdError::new("io", error.to_string())
}

fn exchange(
    agent: &ureq::Agent,
    provider: &Provider,
    discovery: &Discovery,
    attempt: &Attempt,
    code: &str,
) -> Result<OpenAi, CmdError> {
    let token: TokenResponse = http_json(
        agent.post(discovery.token.as_str()).send_form([
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", attempt.redirect.as_str()),
            ("client_id", provider.client_id),
            ("code_verifier", attempt.verifier.as_str()),
        ]),
        "exchange the OpenID authorization code",
    )?;
    if token.access_token.is_empty() || token.refresh_token.is_empty() || token.expires_in == 0 {
        return Err(CmdError::new(
            "bad_input",
            "The OpenID token response had missing or empty fields.",
        ));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(clock_error)?;
    let claims = check_token(
        agent,
        provider,
        &discovery.jwks,
        &attempt.nonce,
        &token.id_token,
        now,
    )?;
    let expires_at = now
        .checked_add(Duration::from_secs(token.expires_in))
        .and_then(|expiry| u64::try_from(expiry.as_millis()).ok())
        .ok_or_else(|| CmdError::new("bad_input", "The OpenID token expiry is out of range."))?;
    Ok(OpenAi {
        version: 1,
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at,
        sub: claims.sub,
        plan: claims.plan,
    })
}

async fn run(
    provider: Provider,
    open_browser: impl FnOnce(&Url) -> Result<(), CmdError> + Send + 'static,
) -> Result<OpenAi, CmdError> {
    let (agent, provider, discovery) = tokio::task::spawn_blocking(move || {
        let mut config = ureq::Agent::config_builder()
            .max_redirects(0)
            .https_only(!provider.allow_loopback_http)
            .timeout_global(Some(HTTP_TIMEOUT));
        if provider.allow_loopback_http {
            config = config.proxy(None);
        }
        let agent = config.build().new_agent();
        let discovery = discover(&agent, &provider)?;
        Ok::<_, CmdError>((agent, provider, discovery))
    })
    .await
    .map_err(|e| CmdError::new("io", e.to_string()))??;

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|e| CmdError::new("io", e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| CmdError::new("io", e.to_string()))?
        .port();
    let attempt = Attempt::new(port)?;
    let url = attempt.authorization_url(&provider, discovery.authorization.clone())?;
    // Poll the listener while opening the browser, including test drivers that follow the URL immediately.
    let (opened, code) = tokio::try_join!(
        async {
            tokio::task::spawn_blocking(move || open_browser(&url))
                .await
                .map_err(|e| CmdError::new("io", e.to_string()))?
        },
        wait_for_callback(listener, &attempt),
    )?;
    let () = opened;
    tokio::task::spawn_blocking(move || exchange(&agent, &provider, &discovery, &attempt, &code))
        .await
        .map_err(|e| CmdError::new("io", e.to_string()))?
}

async fn with_timeout<T>(
    duration: Duration,
    future: impl Future<Output = Result<T, CmdError>>,
) -> Result<T, CmdError> {
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| CmdError::new("unavailable", "Sign in with ChatGPT timed out."))?
}

pub(crate) async fn sign_in(
    open_browser: impl FnOnce(&Url) -> Result<(), CmdError> + Send + 'static,
) -> Result<OpenAi, CmdError> {
    with_timeout(ATTEMPT_TIMEOUT, run(Provider::configured()?, open_browser)).await
}

#[cfg(test)]
#[path = "../tests/support/issuer.rs"]
mod issuer_fixture;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests may panic")]

    use super::issuer_fixture::{Issuer, TokenFault};
    use super::{ATTEMPT_TIMEOUT, Attempt, Provider, run, wait_for_callback, with_timeout};
    use reqwest::{Client, StatusCode};
    use std::io::Write;
    use std::net::Ipv4Addr;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::net::{TcpListener, TcpStream};

    #[tokio::test]
    async fn ac1_flow_checks_pkce_and_signed_plan_with_rotating_keys() {
        let issuer = Arc::new(Issuer::start());
        let browser = Arc::clone(&issuer);
        let result = run(Provider::stand_in(&issuer.url).unwrap(), move |url| {
            browser.follow(url.as_str());
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(result.sub, "stand-in-alice");
        assert_eq!(result.plan, "plus");
        assert_eq!(result.access_token, "stand-in-access");
        assert_eq!(result.refresh_token, "stand-in-refresh");
        assert_eq!(issuer.jwks_hits(), 2);
    }

    #[tokio::test]
    async fn ac4_listener_ignores_other_paths_and_closes_after_the_first_match() {
        for valid_state in [true, false] {
            let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let addr = socket.local_addr().unwrap();
            let attempt = Attempt::new(addr.port()).unwrap();
            let mut url = attempt.redirect.clone();
            let state = if valid_state {
                &attempt.state
            } else {
                "wrong-state"
            };
            url.query_pairs_mut()
                .append_pair("state", state)
                .append_pair("code", "test-code");
            let listener = tokio::spawn(async move { wait_for_callback(socket, &attempt).await });
            let client = Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();

            let different_path = format!("http://{addr}/different");
            let wrong = client.get(different_path).send().await.unwrap();
            assert_eq!(wrong.status(), StatusCode::NOT_FOUND);

            // An unrelated browser connection can stall before finishing its request.
            let mut unrelated = std::net::TcpStream::connect(addr).unwrap();
            unrelated
                .write_all(b"GET /unrelated HTTP/1.1\r\nHost: localhost\r\n")
                .unwrap();
            let answer = client.get(url.clone()).send().await.unwrap();
            assert_eq!(answer.text().await.unwrap(), "You can return to watane.");
            let result = tokio::time::timeout(Duration::from_secs(1), listener)
                .await
                .unwrap()
                .unwrap();
            if valid_state {
                assert_eq!(result.unwrap(), "test-code");
            } else {
                assert_eq!(
                    result.err().unwrap().message,
                    "The sign-in state did not match."
                );
            }
            assert!(client.get(url).send().await.unwrap_err().is_connect());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn ac4_listener_closes_at_five_minutes_without_an_answer() {
        let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = socket.local_addr().unwrap();
        let attempt = Attempt::new(addr.port()).unwrap();
        let before = tokio::time::Instant::now();
        let error = with_timeout(ATTEMPT_TIMEOUT, wait_for_callback(socket, &attempt))
            .await
            .unwrap_err();
        assert_eq!(before.elapsed(), Duration::from_secs(300));
        assert_eq!(error.code, "unavailable");
        assert_eq!(
            TcpStream::connect(addr).await.unwrap_err().kind(),
            std::io::ErrorKind::ConnectionRefused,
        );
    }

    #[tokio::test]
    async fn ac5_invalid_tokens_never_produce_account_material() {
        for (fault, cause) in [
            (TokenFault::Signature, "invalid signature"),
            (TokenFault::Issuer, "the issuer did not match"),
            (TokenFault::Audience, "the audience did not match"),
            (TokenFault::Nonce, "the nonce did not match"),
            (TokenFault::Expired, "the token expired"),
            (
                TokenFault::Algorithm,
                "the token did not name an RS256 signing key",
            ),
        ] {
            let issuer = Arc::new(Issuer::with_fault(fault));
            let browser = Arc::clone(&issuer);
            let error = run(Provider::stand_in(&issuer.url).unwrap(), move |url| {
                browser.follow(url.as_str());
                Ok(())
            })
            .await
            .err()
            .unwrap();
            assert_eq!(error.code, "bad_input", "{fault:?}");
            assert_eq!(
                error.message,
                format!("the ID token did not check: {cause}")
            );
        }
    }
}
