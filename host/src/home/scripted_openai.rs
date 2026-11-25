//! Development preview state for the local scripted Responses and judge accounts (TECHNICAL 19.9).

use crate::{Host, HostError, JudgeAccount, judge_origin};
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;
use rustls::{ClientConfig, RootCertStore, crypto};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use tokio_rustls::TlsConnector;

pub(crate) struct ScriptedOpenAi {
    pub(crate) address: SocketAddrV4,
    pub(crate) connector: TlsConnector,
    pub(crate) token: String,
    pub(crate) judge: Option<(SocketAddrV4, JudgeAccount)>,
}

impl std::fmt::Debug for ScriptedOpenAi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptedOpenAi")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl Host {
    /// Bind the fake account for this preview only (19.9).
    #[doc(hidden)]
    pub fn set_scripted_openai(
        &self,
        address: SocketAddrV4,
        root_pem: &str,
        token: String,
    ) -> Result<(), HostError> {
        self.set_scripted_responses(address, root_pem, token, None)
            .map(|_| ())
    }

    /// Bind both accounts before serving. Return the canonical judge origin when configured (19.9).
    #[doc(hidden)]
    pub fn set_scripted_responses(
        &self,
        address: SocketAddrV4,
        root_pem: &str,
        token: String,
        mut judge: Option<(SocketAddrV4, JudgeAccount)>,
    ) -> Result<Option<String>, HostError> {
        if !self.is_preview() {
            return Err(HostError::BadRequest(
                "scripted OpenAI requires a preview home".into(),
            ));
        }
        if !is_loopback(address) {
            return Err(HostError::BadRequest(
                "scripted OpenAI requires a nonzero port on 127.0.0.1".into(),
            ));
        }
        if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(HostError::BadRequest(
                "invalid scripted OpenAI token".into(),
            ));
        }
        if let Some((address, account)) = &mut judge {
            if !is_loopback(*address) {
                return Err(HostError::BadRequest(
                    "scripted judge requires a nonzero port on 127.0.0.1".into(),
                ));
            }
            account.origin = judge_origin(&account.origin)
                .ok()
                .filter(|origin| origin == "https://judge.test")
                .ok_or_else(|| HostError::BadRequest("invalid scripted judge origin".into()))?;
        }
        let invalid_root = |_| HostError::BadRequest("invalid scripted OpenAI TLS root".into());
        let root = CertificateDer::from_pem_slice(root_pem.as_bytes()).map_err(invalid_root)?;
        let mut roots = RootCertStore::empty();
        roots
            .add(root)
            .map_err(|_| HostError::BadRequest("invalid scripted OpenAI TLS root".into()))?;
        let config =
            ClientConfig::builder_with_provider(Arc::new(crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .map_err(|err| HostError::Internal(err.to_string()))?
                .with_root_certificates(roots)
                .with_no_client_auth();
        let origin = judge.as_ref().map(|(_, account)| account.origin.clone());
        self.home
            .scripted_openai
            .set(ScriptedOpenAi {
                address,
                connector: TlsConnector::from(Arc::new(config)),
                token,
                judge,
            })
            .map_err(|_| HostError::BadRequest("scripted OpenAI is already configured".into()))?;
        Ok(origin)
    }
}

fn is_loopback(address: SocketAddrV4) -> bool {
    *address.ip() == Ipv4Addr::LOCALHOST && address.port() != 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_private_address_outside_loopback_is_refused() {
        let dir = tempfile::tempdir().expect("temporary preview");
        let host = crate::Host::open(dir.path(), crate::Mode::Preview, crate::Limits::default())
            .expect("open preview");
        let address = "10.0.0.1:1234".parse().expect("private socket address");
        let error = host
            .set_scripted_openai(address, "", "fixture".into())
            .expect_err("non-loopback endpoint");
        assert_eq!(
            error.to_string(),
            "scripted OpenAI requires a nonzero port on 127.0.0.1"
        );
    }

    #[test]
    fn scripted_judge_refuses_non_loopback_sockets_and_other_origins() {
        let dir = tempfile::tempdir().expect("temporary preview");
        let host = crate::Host::open(dir.path(), crate::Mode::Preview, crate::Limits::default())
            .expect("open preview");
        let responses = "127.0.0.1:1234".parse().expect("responses socket");
        for (address, origin, expected) in [
            (
                "10.0.0.1:2345",
                "https://judge.test",
                "scripted judge requires a nonzero port on 127.0.0.1",
            ),
            (
                "127.0.0.1:0",
                "https://judge.test",
                "scripted judge requires a nonzero port on 127.0.0.1",
            ),
            (
                "127.0.0.1:2345",
                "https://api.openai.com",
                "invalid scripted judge origin",
            ),
            (
                "127.0.0.1:2345",
                "https://judge.test/other",
                "invalid scripted judge origin",
            ),
        ] {
            let judge = crate::JudgeAccount {
                origin: origin.into(),
                key: "fixture".into(),
            };
            let error = host
                .set_scripted_responses(
                    responses,
                    "",
                    "fixture".into(),
                    Some((address.parse().expect("judge socket"), judge)),
                )
                .expect_err("invalid scripted judge");
            assert_eq!(error.to_string(), expected);
            assert!(host.home.scripted_openai.get().is_none());
        }
    }
}
