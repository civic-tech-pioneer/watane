//! Loopback issuer and browser for the ChatGPT sign-in tests (TECHNICAL 19.6).
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use axum::extract::{Form, Query, State};
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::rand::SystemRandom;
use ring::{digest, signature};
use serde::Deserialize;
use serde_json::{Value, json};
use std::net::{Ipv4Addr, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::oneshot;

const MODULUS: &str = concat!(
    "1rGdNxU4-i3yWiSIMY6cbor36wAm1QMF7u_E8skbgu4ddGAC0tdqhg7chBB_CIwvF",
    "FfjV_9qNZp1Mws30O63TL99_ojndiH-9MfipuSeSNetTt1gY72qEQq34N8Tw1Pn",
    "351HdkYRJbwoY3F_q0rsI6Ro4cBUMw03hB6haSmaIKptICry_tLR-ncneLq8aE0",
    "nZE1S1bElksCFIbh0_sLK-lFklgZIaDoF5oZNlqXr2ie6WYzRMUbTiouhpdp8Hu",
    "ICsAKJcCqZkW_4q2ukWcUXBWIedC_NJZw94YmnO2wvpgT0TZYdOYvIkTe_WgDO",
    "tGRHJ0j_mbw57ERaXDRKjypR_w",
);

#[derive(Clone, Copy, Debug, Default)]
pub enum TokenFault {
    #[default]
    None,
    Signature,
    Issuer,
    Audience,
    Nonce,
    Expired,
    Algorithm,
}

#[derive(Debug, Deserialize)]
struct Authorization {
    response_type: String,
    client_id: String,
    redirect_uri: String,
    scope: String,
    code_challenge: String,
    code_challenge_method: String,
    state: String,
    nonce: String,
}

#[derive(Debug, Deserialize)]
struct Exchange {
    grant_type: String,
    client_id: String,
    redirect_uri: String,
    code_verifier: String,
    code: String,
}

#[derive(Debug)]
struct Inner {
    issuer: String,
    authorization: Mutex<Option<Authorization>>,
    fault: TokenFault,
    jwks_hits: AtomicUsize,
}

#[derive(Debug)]
pub struct Issuer {
    pub url: String,
    inner: Arc<Inner>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Issuer {
    pub fn start() -> Self {
        Self::with_fault(TokenFault::None)
    }

    pub fn with_fault(fault: TokenFault) -> Self {
        let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        socket.set_nonblocking(true).unwrap();
        let url = format!("http://{}", socket.local_addr().unwrap());
        let inner = Arc::new(Inner {
            issuer: url.clone(),
            authorization: Mutex::new(None),
            fault,
            jwks_hits: AtomicUsize::new(0),
        });
        let app = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/authorize", get(authorize))
            .route("/token", post(token))
            .route("/keys", get(keys))
            .with_state(Arc::clone(&inner));
        let (stop, stopped) = oneshot::channel();
        let thread = thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    axum::serve(tokio::net::TcpListener::from_std(socket).unwrap(), app)
                        .with_graceful_shutdown(async move {
                            let _ = stopped.await;
                        })
                        .await
                        .unwrap();
                });
        });
        Self {
            url,
            inner,
            stop: Some(stop),
            thread: Some(thread),
        }
    }

    pub fn follow(&self, url: &str) {
        assert!(url.starts_with(&format!("{}/authorize?", self.url)));
        let body = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap()
            .get(url)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(body, "You can return to watane.");
    }

    pub fn jwks_hits(&self) -> usize {
        self.inner.jwks_hits.load(Ordering::Relaxed)
    }
}

impl Drop for Issuer {
    fn drop(&mut self) {
        let _ = self.stop.take().unwrap().send(());
        self.thread.take().unwrap().join().unwrap();
    }
}

async fn discovery(State(inner): State<Arc<Inner>>) -> Json<Value> {
    let issuer = &inner.issuer;
    Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "jwks_uri": format!("{issuer}/keys"),
    }))
}

async fn authorize(
    State(inner): State<Arc<Inner>>,
    Query(query): Query<Authorization>,
) -> Redirect {
    assert_eq!(query.response_type, "code");
    assert_eq!(query.client_id, "watane-test");
    assert_eq!(query.scope, "openid offline_access plan.read");
    assert_eq!(query.code_challenge_method, "S256");
    assert_eq!(query.state.len(), 32);
    assert_eq!(query.nonce.len(), 32);
    let mut redirect = tauri::Url::parse(&query.redirect_uri).unwrap();
    assert_eq!(redirect.scheme(), "http");
    assert_eq!(redirect.host_str(), Some("127.0.0.1"));
    assert_eq!(redirect.path().len(), 33);
    redirect
        .query_pairs_mut()
        .append_pair("state", &query.state)
        .append_pair("code", "test-code");
    *inner.authorization.lock().unwrap() = Some(query);
    Redirect::temporary(redirect.as_str())
}

async fn token(State(inner): State<Arc<Inner>>, Form(exchange): Form<Exchange>) -> Json<Value> {
    let auth = inner.authorization.lock().unwrap().take().unwrap();
    assert_eq!(exchange.grant_type, "authorization_code");
    assert_eq!(exchange.client_id, auth.client_id);
    assert_eq!(exchange.redirect_uri, auth.redirect_uri);
    assert_eq!(exchange.code, "test-code");
    assert_eq!(
        URL_SAFE_NO_PAD
            .decode(&exchange.code_verifier)
            .unwrap()
            .len(),
        32
    );
    assert_eq!(
        auth.code_challenge,
        URL_SAFE_NO_PAD.encode(digest::digest(
            &digest::SHA256,
            exchange.code_verifier.as_bytes()
        )),
    );
    Json(json!({
        "access_token": "stand-in-access",
        "refresh_token": "stand-in-refresh",
        "expires_in": 3600,
        "id_token": signed_token(&inner.issuer, &auth, inner.fault),
        "plan": "unverified-plan",
    }))
}

fn signed_token(issuer: &str, auth: &Authorization, fault: TokenFault) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let (mut issuer, mut audience, mut nonce, mut expiry, mut algorithm) = (
        issuer,
        auth.client_id.as_str(),
        auth.nonce.as_str(),
        now + 300,
        "RS256",
    );
    match fault {
        TokenFault::None | TokenFault::Signature => {}
        TokenFault::Issuer => issuer = "different-issuer",
        TokenFault::Audience => audience = "different-client",
        TokenFault::Nonce => nonce = "different-nonce",
        TokenFault::Expired => expiry = now - 300,
        TokenFault::Algorithm => algorithm = "HS256",
    }
    let header = URL_SAFE_NO_PAD.encode(json!({"alg": algorithm, "kid": "test-key"}).to_string());
    let claims = URL_SAFE_NO_PAD.encode(
        json!({
            "iss": issuer, "aud": audience, "nonce": nonce, "exp": expiry,
            "sub": "stand-in-alice", "plan": "plus",
        })
        .to_string(),
    );
    let signed = format!("{header}.{claims}");
    // Publicly committed test fixture key, generated for this stand-in only.
    let key = signature::RsaKeyPair::from_pkcs8(include_bytes!("issuer-test-key.der")).unwrap();
    let mut bytes = vec![0; key.public().modulus_len()];
    key.sign(
        &signature::RSA_PKCS1_SHA256,
        &SystemRandom::new(),
        signed.as_bytes(),
        &mut bytes,
    )
    .unwrap();
    if matches!(fault, TokenFault::Signature) {
        bytes[0] ^= 1;
    }
    format!("{signed}.{}", URL_SAFE_NO_PAD.encode(bytes))
}

async fn keys(State(inner): State<Arc<Inner>>) -> Json<Value> {
    // Simulate rotation: the verifier must fetch the JWKS again for an unknown kid.
    if inner.jwks_hits.fetch_add(1, Ordering::Relaxed) == 0 {
        return Json(json!({"keys": []}));
    }
    Json(json!({"keys": [{
        "kid": "test-key",
        "kty": "RSA",
        "alg": "RS256",
        "use": "sig",
        "n": MODULUS,
        "e": "AQAB",
    }]}))
}
