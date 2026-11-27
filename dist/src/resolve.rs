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

/// Resolves like ureq, then refuses when any address is local. Every target address ureq
/// asks for is checked; only the resolution of the configured proxy endpoint itself is
/// not. A name that an HTTP CONNECT proxy from the environment resolves never reaches
/// this resolver, so it stays unchecked (TECHNICAL 6, "Not protected").
#[derive(Debug, Default)]
pub(crate) struct GuardedResolver {
    /// Tests only: names with fixed addresses, in place of DNS.
    #[cfg(test)]
    pub(crate) hosts: TestHosts,
}

/// `uri` is the configured proxy's own host and port (ureq's CONNECT and SOCKS connectors
/// resolve it through the same resolver as a target).
fn is_proxy_endpoint(uri: &Uri, config: &Config) -> bool {
    config.proxy().is_some_and(|proxy| {
        let port = uri.port_u16().or(match uri.scheme_str() {
            Some("https") => Some(443),
            Some("http") => Some(80),
            _ => None,
        });
        uri.host() == Some(proxy.host()) && port == Some(proxy.port())
    })
}

impl Resolver for GuardedResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let proxy_endpoint = is_proxy_endpoint(uri, config);
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
        if !proxy_endpoint && addrs.iter().any(|addr| local_ip(addr.ip())) {
            return Err(ureq::Error::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                LocalAddress,
            )));
        }
        Ok(addrs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ureq::Proxy;

    fn config(proxy: Option<Proxy>) -> Config {
        Config::builder().proxy(proxy).build()
    }
    fn resolver(name: &str, ips: &[&str]) -> GuardedResolver {
        let resolver = GuardedResolver::default();
        resolver.hosts.lock().unwrap().push((
            name.into(),
            ips.iter().map(|ip| ip.parse().unwrap()).collect(),
        ));
        resolver
    }
    fn resolve(
        resolver: &GuardedResolver,
        uri: &str,
        config: &Config,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let uri: Uri = uri.parse().unwrap();
        resolver.resolve(&uri, config, NextTimeout::default())
    }

    #[test]
    fn s158_a_local_target_is_refused_also_with_a_socks4_proxy_configured() {
        let proxy = Proxy::new("socks4://127.0.0.1:9").unwrap();
        let config = config(Some(proxy));
        let r = resolver("lan.test", &["127.0.0.1"]);
        let got = resolve(&r, "http://lan.test:8080/", &config);
        assert!(is_local_refusal(&got.unwrap_err()));
    }
    #[test]
    fn s158_the_proxy_endpoint_is_not_checked_even_when_on_no_proxy() {
        let proxy = Proxy::builder(ureq::ProxyProtocol::Http)
            .host("127.0.0.1")
            .port(40855)
            .no_proxy("127.0.0.1")
            .build()
            .unwrap();
        let config = config(Some(proxy));
        let r = resolver("global.test", &["8.8.8.8"]);
        // The proxy itself: allowed. A global target: allowed. A local one on another port: refused.
        assert!(resolve(&r, "http://127.0.0.1:40855/", &config).is_ok());
        assert!(resolve(&r, "http://global.test/", &config).is_ok());
        let local = resolver("127.0.0.1", &["127.0.0.1"]);
        assert!(is_local_refusal(
            &resolve(&local, "http://127.0.0.1:9/", &config).unwrap_err()
        ));
    }
    #[test]
    fn s158_the_default_port_follows_the_scheme() {
        let r = resolver("global.test", &["8.8.8.8"]);
        let config = config(None);
        let port = |uri| resolve(&r, uri, &config).unwrap()[0].port();
        assert_eq!(port("https://global.test/"), 443);
        assert_eq!(port("http://global.test/"), 80);
        assert_eq!(port("http://global.test:81/"), 81);
    }
}
