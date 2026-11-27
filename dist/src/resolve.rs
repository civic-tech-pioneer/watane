//! The resolver of the guarded agent (TECHNICAL 4.5, 5.9): a listed read refuses a host
//! name whose resolved addresses include a local one, at connect time.

use crate::index::local_ip;
use std::net::{SocketAddr, ToSocketAddrs};
use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
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

/// The port ureq dials for `uri`: its own, else its scheme's default (ureq's `SchemeExt`).
fn port(uri: &Uri) -> Option<u16> {
    uri.port_u16().or(
        match uri.scheme_str().map(str::to_ascii_lowercase).as_deref() {
            Some("https") => Some(443),
            Some("http") => Some(80),
            Some("socks4" | "socks4a" | "socks" | "socks5") => Some(1080),
            _ => None,
        },
    )
}

/// `uri` is the configured proxy's own host and port (ureq's CONNECT and SOCKS connectors
/// resolve it through the same resolver as a target).
fn is_proxy_endpoint(uri: &Uri, config: &Config) -> bool {
    config
        .proxy()
        .is_some_and(|proxy| uri.host() == Some(proxy.host()) && port(uri) == Some(proxy.port()))
}

/// One lookup of `host:port` as ureq's `DefaultResolver` does it, with every address it
/// returns: with a timeout, on a helper thread that gives up at the deadline (the thread
/// ureq itself would spawn for this lookup; TECHNICAL 4.5).
fn lookup(addr: String, timeout: NextTimeout) -> Result<Vec<SocketAddr>, ureq::Error> {
    if timeout.after.is_not_happening() {
        return Ok(addr.to_socket_addrs()?.collect());
    }
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || tx.send(addr.to_socket_addrs()).ok());
    match rx.recv_timeout(*timeout.after) {
        Ok(found) => Ok(found?.collect()),
        Err(_) => Err(ureq::Error::Timeout(timeout.reason)),
    }
}

impl GuardedResolver {
    /// Every address of one lookup of `uri`'s host, before any cap.
    fn lookup_all(&self, uri: &Uri, timeout: NextTimeout) -> Result<Vec<SocketAddr>, ureq::Error> {
        let (Some(host), Some(port)) = (uri.host(), port(uri)) else {
            return Err(ureq::Error::BadUri(format!(
                "{uri} has no host or known scheme"
            )));
        };
        #[cfg(test)]
        if let Some((_, ips)) = self
            .hosts
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .iter()
            .find(|(name, _)| name == host)
        {
            return Ok(ips.iter().map(|ip| SocketAddr::new(*ip, port)).collect());
        }
        lookup(format!("{host}:{port}"), timeout)
    }
}

impl Resolver for GuardedResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let all: Vec<SocketAddr> = config
            .ip_family()
            .keep_wanted(self.lookup_all(uri, timeout)?.into_iter())
            .collect();
        // The whole result of the lookup is checked; only then is it capped to what ureq
        // keeps, so a local address past the cap still refuses the name.
        if !is_proxy_endpoint(uri, config) && all.iter().any(|addr| local_ip(addr.ip())) {
            return Err(ureq::Error::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                LocalAddress,
            )));
        }
        let mut out = self.empty();
        for addr in all.into_iter().take(MAX_ADDRS) {
            out.push(addr);
        }
        if out.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(out)
    }
}

/// The most addresses ureq's `ResolvedSocketAddrs` holds.
const MAX_ADDRS: usize = 16;

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
        resolve(&r, "http://127.0.0.1:40855/", &config).unwrap();
        resolve(&r, "http://global.test/", &config).unwrap();
        let local = resolver("127.0.0.1", &["127.0.0.1"]);
        assert!(is_local_refusal(
            &resolve(&local, "http://127.0.0.1:9/", &config).unwrap_err()
        ));
    }
    #[test]
    fn s158_a_proxy_endpoint_without_a_port_gets_its_scheme_default() {
        // ureq resolves `Proxy::uri()` itself, which keeps no port when none was given.
        let r = resolver("proxy.test", &["127.0.0.1"]);
        for proxy in ["http://proxy.test", "https://proxy.test"] {
            let config = config(Some(Proxy::new(proxy).unwrap()));
            resolve(&r, proxy, &config).unwrap();
        }
        let config = config(Some(Proxy::new("http://proxy.test").unwrap()));
        assert!(is_local_refusal(
            &resolve(&r, "https://proxy.test/", &config).unwrap_err()
        ));
        assert_eq!(LocalAddress.to_string(), "local address");
    }
    #[test]
    fn s158_r1_a_local_address_past_the_sixteenth_refuses_the_whole_name() {
        // One lookup: 16 global addresses, then a local one that ureq's cap would drop.
        let mut ips: Vec<String> = (1..=16).map(|i| format!("8.8.8.{i}")).collect();
        ips.push("127.0.0.1".into());
        let ips: Vec<&str> = ips.iter().map(String::as_str).collect();
        let r = resolver("many.test", &ips);
        let got = resolve(&r, "http://many.test/", &config(None));
        assert!(is_local_refusal(&got.unwrap_err()));
        // 17 global addresses: allowed, capped at what ureq keeps.
        let ips: Vec<String> = (1..=17).map(|i| format!("8.8.8.{i}")).collect();
        let ips: Vec<&str> = ips.iter().map(String::as_str).collect();
        let r = resolver("many.test", &ips);
        assert_eq!(
            resolve(&r, "http://many.test/", &config(None))
                .unwrap()
                .len(),
            16
        );
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
