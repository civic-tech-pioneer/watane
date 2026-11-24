//! Development preview state for the local scripted OpenAI account (TECHNICAL 19.9).

use crate::{Host, HostError};
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
        if !self.is_preview() {
            return Err(HostError::BadRequest(
                "scripted OpenAI requires a preview home".into(),
            ));
        }
        if *address.ip() != Ipv4Addr::LOCALHOST || address.port() == 0 {
            return Err(HostError::BadRequest(
                "scripted OpenAI requires a nonzero port on 127.0.0.1".into(),
            ));
        }
        if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(HostError::BadRequest(
                "invalid scripted OpenAI token".into(),
            ));
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
        self.home
            .scripted_openai
            .set(ScriptedOpenAi {
                address,
                connector: TlsConnector::from(Arc::new(config)),
                token,
            })
            .map_err(|_| HostError::BadRequest("scripted OpenAI is already configured".into()))
    }
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
}
