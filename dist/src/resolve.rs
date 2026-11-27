//! The resolver of the guarded agent (TECHNICAL 4.5, 5.9): a listed read refuses a host
//! name whose resolved addresses include a local one, at connect time.

use crate::index::local_ip;
use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

/// The cause carried inside the `ureq::Error::Io` that a refused name returns.
#[derive(Debug)]
pub(crate) struct LocalAddress;
impl std::fmt::Display for LocalAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("local address")
    }
}
impl std::error::Error for LocalAddress {}

/// True when `error` is the refusal of `GuardedResolver`.
pub(crate) fn is_local_refusal(error: &ureq::Error) -> bool {
    matches!(error, ureq::Error::Io(io)
        if io.get_ref().is_some_and(|inner| inner.is::<LocalAddress>()))
}

/// Tests only: names with fixed addresses.
#[cfg(test)]
pub(crate) type TestHosts = std::sync::Arc<std::sync::Mutex<Vec<(String, Vec<std::net::IpAddr>)>>>;

/// Resolves like ureq, then refuses when any address is local. When the request goes
/// through a proxy, ureq resolves only the proxy here and the proxy resolves the target,
/// except SOCKS4, whose target ureq resolves here too: neither is checked (TECHNICAL 6,
/// "Not protected").
#[derive(Debug, Default)]
pub(crate) struct GuardedResolver {
    /// Tests only: names with fixed addresses, in place of DNS.
    #[cfg(test)]
    pub(crate) hosts: TestHosts,
}

impl Resolver for GuardedResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let proxied = config.proxy().is_some_and(|proxy| !proxy.is_no_proxy(uri));
        #[cfg(test)]
        let fixed = {
            let host = uri.host().unwrap_or_default();
            self.hosts
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .iter()
                .find(|(name, _)| name == host)
                .map(|(_, ips)| ips.clone())
        };
        #[cfg(test)]
        let addrs = match fixed {
            Some(ips) => {
                let port = uri
                    .port_u16()
                    .unwrap_or(if uri.scheme_str() == Some("https") {
                        443
                    } else {
                        80
                    });
                let mut out = self.empty();
                for ip in ips {
                    out.push(std::net::SocketAddr::new(ip, port));
                }
                out
            }
            None => DefaultResolver::default().resolve(uri, config, timeout)?,
        };
        #[cfg(not(test))]
        let addrs = DefaultResolver::default().resolve(uri, config, timeout)?;
        if !proxied && addrs.iter().any(|addr| local_ip(addr.ip())) {
            return Err(ureq::Error::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                LocalAddress,
            )));
        }
        Ok(addrs)
    }
}
