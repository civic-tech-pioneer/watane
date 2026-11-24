//! Reads from a web host (TECHNICAL 4.5): GET with a body limit and a
//! whole-request timeout on every request. Outgoing only; nothing listens.

use crate::{Dist, DistError, Entry, PublicKey};
use std::time::{Duration, Instant};

/// The most redirects one read follows (TECHNICAL 4.5).
pub(crate) const MAX_REDIRECTS: usize = 5;

/// Checks the resolved URL of a redirect before the next request.
pub(crate) type Check<'a> = dyn Fn(&str) -> Result<(), DistError> + 'a;

/// The result of one HTTP hop: a completed read or a redirect's Location.
pub(crate) enum Hop<T> {
    Done(T),
    Redirect(String),
}

/// Follow up to `MAX_REDIRECTS` redirects by hand: each `Location` is resolved against
/// the current URL, must be http or https, and passes `check` before any request to it
/// (TECHNICAL 4.5). A refused scheme is `BadUrl` with `first`, the URL the caller asked.
pub(crate) fn follow_hops<T>(
    first: &str,
    check: &Check<'_>,
    mut step: impl FnMut(&str) -> Result<Hop<T>, DistError>,
) -> Result<T, DistError> {
    let mut current = first.to_owned();
    for hop in 0..=MAX_REDIRECTS {
        let location = match step(&current)? {
            Hop::Done(value) => return Ok(value),
            Hop::Redirect(location) => location,
        };
        if hop == MAX_REDIRECTS {
            break;
        }
        let target = url::Url::parse(&current)
            .and_then(|base| base.join(&location))
            .map_err(|_| DistError::Network {
                url: current.clone(),
                cause: format!("bad redirect location {location:?}"),
            })?;
        if !matches!(target.scheme(), "http" | "https") {
            return Err(DistError::BadUrl {
                url: first.to_owned(),
                cause: "redirects to a scheme other than http or https",
            });
        }
        check(target.as_str())?;
        current = target.into();
    }
    Err(DistError::Network {
        url: current,
        cause: "too many redirects".into(),
    })
}

const RETRY_WAITS: [Duration; 2] = [Duration::from_millis(250), Duration::from_millis(500)];

pub(crate) fn file_url(base: &str, file: &str) -> String {
    // Bases have passed Link validation; files are fixed text, seqs or verified hashes.
    url::Url::parse(base)
        .expect("validated base")
        .join(file)
        .expect("relative file")
        .into()
}
impl Dist {
    /// One read with its whole timeout. A 5xx is tried twice more, after 250 ms and
    /// 500 ms; the waits and the attempts all end at the first attempt's start plus
    /// `timeout`, so a host that keeps failing costs no more than one read (TECHNICAL 4.5).
    ///
    /// `listed`: the base came from a listing or a hint, so every redirect target must
    /// pass the local-address rule of a listed base (5.9) before it is requested.
    pub(crate) fn get(
        &self,
        url: &str,
        limit: u64,
        timeout: Duration,
        head: bool,
        listed: bool,
    ) -> Result<Vec<u8>, DistError> {
        let check = |target: &str| {
            self.check_listed_base(target).map_err(|error| match error {
                DistError::BadUrl { .. } => DistError::BadUrl {
                    url: url.into(),
                    cause: "redirects to a local address",
                },
                other => other,
            })
        };
        self.get_guarded(url, limit, timeout, head, listed.then_some(&check))
    }
    pub(crate) fn get_guarded(
        &self,
        url: &str,
        limit: u64,
        timeout: Duration,
        head: bool,
        check: Option<&Check<'_>>,
    ) -> Result<Vec<u8>, DistError> {
        let deadline = Instant::now() + timeout;
        let mut waits = RETRY_WAITS.iter();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let error = match self.get_once(url, limit, left, head, check) {
                Err(
                    error @ DistError::Http {
                        status: 500..=599, ..
                    },
                ) => error,
                other => return other,
            };
            let Some(wait) = waits.next() else {
                return Err(error);
            };
            // No retry starts when the wait would use up the time left.
            if Instant::now() + *wait >= deadline {
                return Err(error);
            }
            std::thread::sleep(*wait);
        }
    }
    fn get_once(
        &self,
        url: &str,
        limit: u64,
        timeout: Duration,
        head: bool,
        check: Option<&Check<'_>>,
    ) -> Result<Vec<u8>, DistError> {
        // Every read follows by hand, so each target meets the scheme rule; an unlisted
        // read has no local-address rule.
        let check = check.unwrap_or(&|_: &str| Ok(()));
        // One deadline for all hops.
        let deadline = Instant::now() + timeout;
        follow_hops(url, check, |current| {
            let left = deadline.saturating_duration_since(Instant::now());
            self.get_hop(current, limit, left, head)
        })
    }
    fn get_hop(
        &self,
        url: &str,
        limit: u64,
        timeout: Duration,
        head: bool,
    ) -> Result<Hop<Vec<u8>>, DistError> {
        // The agent follows no redirect; `follow_hops` checks each target.
        let agent = &self.agent;
        let map = |error: ureq::Error| match error {
            ureq::Error::Timeout(_) => DistError::Timeout { url: url.into() },
            ureq::Error::BodyExceedsLimit(_) => DistError::TooLarge {
                url: url.into(),
                limit,
            },
            other => DistError::Network {
                url: url.into(),
                cause: other.to_string(),
            },
        };
        let mut request = agent
            .get(url)
            .config()
            .timeout_global(Some(timeout))
            .build();
        if head {
            request = request.header("Cache-Control", "no-cache");
        }
        let mut response = request.call().map_err(map)?;
        match response.status().as_u16() {
            300 | 301 | 302 | 303 | 307 | 308 => {
                match response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                {
                    Some(location) => Ok(Hop::Redirect(location.to_owned())),
                    None => Err(DistError::Http {
                        url: url.into(),
                        status: response.status().as_u16(),
                    }),
                }
            }
            200 => {
                // ureq's limit reader errors at the boundary before probing EOF.
                // Read one sentinel byte so an exactly-sized signed package is accepted.
                let bytes = response
                    .body_mut()
                    .with_config()
                    .limit(limit.saturating_add(1))
                    .read_to_vec()
                    .map_err(map)?;
                if bytes.len() as u64 > limit {
                    return Err(DistError::TooLarge {
                        url: url.into(),
                        limit,
                    });
                }
                Ok(Hop::Done(bytes))
            }
            404 | 410 => Err(DistError::NotFound { url: url.into() }),
            status => Err(DistError::Http {
                url: url.into(),
                status,
            }),
        }
    }
    pub(crate) fn read_entry(
        &self,
        key: PublicKey,
        url: &str,
        seq: Option<u64>,
        listed: bool,
    ) -> Result<Entry, DistError> {
        let bytes = self.get(
            url,
            self.options.max_entry_file,
            self.options.entry_timeout,
            seq.is_none(),
            listed,
        )?;
        let entry = Entry::decode(&bytes).map_err(|cause| DistError::Invalid {
            url: url.into(),
            cause,
        })?;
        if entry.author() != key {
            return Err(DistError::WrongChannel {
                url: url.into(),
                expected: key,
                found: entry.author(),
            });
        }
        if let Some(expected) = seq
            && entry.seq() != expected
        {
            return Err(DistError::WrongSeq {
                url: url.into(),
                expected,
                found: entry.seq(),
            });
        }
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Options;
    use std::cell::RefCell;
    use std::sync::Arc;

    struct NoSign;
    impl crate::Confirm for NoSign {
        fn confirm(&self, _: &crate::SignRequest) -> crate::Decision {
            panic!("a read must not sign")
        }
    }
    fn local(target: &str) -> Result<(), DistError> {
        if crate::index::local_base(target) {
            Err(DistError::BadUrl {
                url: target.into(),
                cause: "local address",
            })
        } else {
            Ok(())
        }
    }
    #[test]
    fn ac1_local_target_refused_before_its_request() {
        let seen = RefCell::new(Vec::new());
        let result = follow_hops("http://example.org/head", &local, |url| {
            seen.borrow_mut().push(url.to_owned());
            Ok(Hop::<()>::Redirect("http://127.0.0.1:9/head".into()))
        });
        match result {
            Err(DistError::BadUrl { url, cause }) => {
                assert_eq!(url, "http://127.0.0.1:9/head");
                assert_eq!(cause, "local address");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(*seen.borrow(), ["http://example.org/head"]);
    }
    #[test]
    fn s112_listed_redirects_to_local_ranges_stop_before_the_next_hop() {
        let first = "https://publisher.example/head";
        for target in [
            "http://0.1.2.3/head",
            "http://100.64.0.1/head",
            "http://198.19.255.255/head",
            "http://[fec0::1]/head",
            "http://[64:ff9b::a00:1]/head",
            "http://[2002:c0a8:101::]/head",
        ] {
            let result = follow_hops(first, &local, |url| {
                assert_eq!(url, first, "the refused target must receive no request");
                Ok(Hop::<()>::Redirect(target.into()))
            });
            assert!(matches!(
                result,
                Err(DistError::BadUrl {
                    cause: "local address",
                    ..
                })
            ));
        }
    }

    #[test]
    fn ac1_trailing_dot_target_refused() {
        let result = follow_hops("http://example.org/head", &local, |_| {
            Ok(Hop::<()>::Redirect("http://localhost./head".into()))
        });
        assert!(matches!(result, Err(DistError::BadUrl { .. })));
    }
    #[test]
    fn ac1_relative_location_resolved_against_current_url() {
        let seen = RefCell::new(Vec::new());
        let result = follow_hops("http://example.org/a/head", &local, |url| {
            seen.borrow_mut().push(url.to_owned());
            Ok(match seen.borrow().len() {
                1 => Hop::Redirect("../b/head".into()),
                2 => Hop::Redirect("/c/head".into()),
                _ => Hop::Done(7),
            })
        });
        assert_eq!(result.unwrap(), 7);
        assert_eq!(
            *seen.borrow(),
            [
                "http://example.org/a/head",
                "http://example.org/b/head",
                "http://example.org/c/head"
            ]
        );
    }
    #[test]
    fn s116_http_redirect_to_https_is_allowed() {
        let first = "http://publisher.example/head";
        let target = "https://publisher.example/head";
        let mut requested = Vec::new();
        let result = follow_hops(
            first,
            &|url| {
                assert_eq!(url, target);
                Ok(())
            },
            |url| {
                requested.push(url.to_owned());
                Ok(if url == first {
                    Hop::Redirect(target.into())
                } else {
                    Hop::Done("head")
                })
            },
        );

        assert_eq!(result.unwrap(), "head");
        assert_eq!(requested, [first, target]);
    }
    #[test]
    fn ac3_five_hops_followed_sixth_is_an_error() {
        let hops = |redirects: usize| {
            let n = RefCell::new(0);
            follow_hops("http://example.org/0", &local, |_| {
                *n.borrow_mut() += 1;
                Ok(if *n.borrow() <= redirects {
                    Hop::Redirect(format!("/{}", n.borrow()))
                } else {
                    Hop::Done(())
                })
            })
        };
        hops(5).unwrap();
        assert!(matches!(hops(6), Err(DistError::Network { .. })));
    }
    /// A tiny_http server: path -> (status, location or body).
    fn serve(
        routes: Vec<(&'static str, u16, &'static str)>,
    ) -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://{}", server.server_addr().to_ip().unwrap());
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                let url = request.url().to_owned();
                log.lock().unwrap().push(url.clone());
                let (status, text) = routes
                    .iter()
                    .find(|r| r.0 == url)
                    .map_or((404, ""), |r| (r.1, r.2));
                let mut response =
                    tiny_http::Response::from_string(if status == 200 { text } else { "" })
                        .with_status_code(status);
                if status / 100 == 3 {
                    response = response
                        .with_header(tiny_http::Header::from_bytes("Location", text).unwrap());
                }
                let _ = request.respond(response);
            }
        });
        (base, seen)
    }
    /// Review S112 F1.1 (adversary): a redirect to a proxy scheme of a listed read
    /// reached a local port; a target scheme other than http or https is refused
    /// before any request to it.
    #[test]
    fn adv_s112_redirect_to_proxy_scheme_reaches_a_local_port() {
        let (victim, victim_seen) = serve(vec![("/secret", 200, "internal")]);
        for scheme in ["socks5", "socks4", "socks", "socks5h", "socks4a"] {
            let target = victim.replacen("http", scheme, 1) + "/secret";
            let target: &'static str = Box::leak(target.into_boxed_str());
            let (base, _) = serve(vec![("/a", 302, target)]);
            let dir = tempfile::tempdir().unwrap();
            let dist = Dist::open(dir.path(), Arc::new(NoSign), Options::default()).unwrap();
            let got = dist.get(
                &format!("{base}/a"),
                100,
                Duration::from_secs(5),
                false,
                true,
            );
            assert!(
                matches!(&got, Err(DistError::BadUrl { url, .. }) if url == &format!("{base}/a")),
                "{scheme}: {got:?}; local server saw {:?}",
                victim_seen.lock().unwrap()
            );
        }
        assert!(victim_seen.lock().unwrap().is_empty());
    }
    /// F1.1, every read: an unlisted read refuses a non-http(s) redirect target too.
    #[test]
    fn s112_unlisted_redirect_to_another_scheme_is_refused_before_its_request() {
        let (victim, victim_seen) = serve(vec![("/secret", 200, "internal")]);
        for scheme in ["socks5", "socks4", "ftp", "ws"] {
            let target = victim.replacen("http", scheme, 1) + "/secret";
            let target: &'static str = Box::leak(target.into_boxed_str());
            let (base, seen) = serve(vec![("/a", 302, target)]);
            let dir = tempfile::tempdir().unwrap();
            let dist = Dist::open(dir.path(), Arc::new(NoSign), Options::default()).unwrap();
            let got = dist.get(
                &format!("{base}/a"),
                100,
                Duration::from_secs(5),
                false,
                false,
            );
            assert!(
                matches!(&got, Err(DistError::BadUrl { url, .. }) if url == &format!("{base}/a")),
                "{scheme}: {got:?}"
            );
            assert_eq!(*seen.lock().unwrap(), ["/a"]);
        }
        assert!(victim_seen.lock().unwrap().is_empty());
    }
    #[test]
    fn ac1_get_guarded_follows_a_checked_target_over_http() {
        let (base, seen) = serve(vec![("/a", 302, "/b"), ("/b", 200, "ok")]);
        let dir = tempfile::tempdir().unwrap();
        let dist = Dist::open(dir.path(), Arc::new(NoSign), Options::default()).unwrap();
        let allow = |_: &str| Ok(());
        let got = dist
            .get_guarded(
                &format!("{base}/a"),
                100,
                Duration::from_secs(5),
                false,
                Some(&allow),
            )
            .unwrap();
        assert_eq!(got, b"ok");
        assert_eq!(*seen.lock().unwrap(), ["/a", "/b"]);
    }
    #[test]
    fn ac2_get_guarded_follows_a_300_with_a_location() {
        let (base, seen) = serve(vec![("/a", 300, "/b"), ("/b", 200, "ok")]);
        let dir = tempfile::tempdir().unwrap();
        let dist = Dist::open(dir.path(), Arc::new(NoSign), Options::default()).unwrap();
        let allow = |_: &str| Ok(());
        let got = dist
            .get_guarded(
                &format!("{base}/a"),
                100,
                Duration::from_secs(5),
                false,
                Some(&allow),
            )
            .unwrap();
        assert_eq!(got, b"ok");
        assert_eq!(*seen.lock().unwrap(), ["/a", "/b"]);
    }
    #[test]
    fn ac2_a_300_without_a_location_is_an_http_error() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://{}", server.server_addr().to_ip().unwrap());
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                let _ = request
                    .respond(tiny_http::Response::from_data(Vec::new()).with_status_code(300));
            }
        });
        let dir = tempfile::tempdir().unwrap();
        let dist = Dist::open(dir.path(), Arc::new(NoSign), Options::default()).unwrap();
        let allow = |_: &str| Ok(());
        let got = dist.get_guarded(
            &format!("{base}/a"),
            100,
            Duration::from_secs(5),
            false,
            Some(&allow),
        );
        assert!(
            matches!(got, Err(DistError::Http { status: 300, .. })),
            "{got:?}"
        );
    }
    /// A server that waits `delay` before each answer: `/h0` to `/h2` redirect to the next,
    /// `/h3` answers 200 with `body`.
    fn serve_slow(delay: Duration, body: &'static str) -> String {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://{}", server.server_addr().to_ip().unwrap());
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                std::thread::sleep(delay);
                let response = match request.url() {
                    "/h0" | "/h1" | "/h2" => {
                        let next = format!("/h{}", &request.url()[2..].parse::<u8>().unwrap() + 1);
                        tiny_http::Response::from_data(Vec::new())
                            .with_status_code(302)
                            .with_header(tiny_http::Header::from_bytes("Location", next).unwrap())
                    }
                    _ => tiny_http::Response::from_string(body).with_status_code(200),
                };
                let _ = request.respond(response);
            }
        });
        base
    }
    #[test]
    fn ac3_one_deadline_covers_all_hops() {
        // Four hops of 100 ms: each is under the 300 ms timeout, the chain is not.
        let base = serve_slow(Duration::from_millis(100), "ok");
        let dir = tempfile::tempdir().unwrap();
        let dist = Dist::open(dir.path(), Arc::new(NoSign), Options::default()).unwrap();
        let allow = |_: &str| Ok(());
        let got = dist.get_guarded(
            &format!("{base}/h0"),
            100,
            Duration::from_millis(300),
            false,
            Some(&allow),
        );
        assert!(matches!(got, Err(DistError::Timeout { .. })), "{got:?}");
        // The same chain with room enough succeeds, so the delay is what timed out.
        let got = dist.get_guarded(
            &format!("{base}/h0"),
            100,
            Duration::from_secs(5),
            false,
            Some(&allow),
        );
        assert_eq!(got.unwrap(), b"ok");
    }
    #[test]
    fn ac3_the_body_limit_binds_the_response_after_a_redirect() {
        let base = serve_slow(Duration::ZERO, "0123456789abcdef");
        let dir = tempfile::tempdir().unwrap();
        let dist = Dist::open(dir.path(), Arc::new(NoSign), Options::default()).unwrap();
        let allow = |_: &str| Ok(());
        let get = |limit| {
            dist.get_guarded(
                &format!("{base}/h0"),
                limit,
                Duration::from_secs(5),
                false,
                Some(&allow),
            )
        };
        assert!(matches!(
            get(15),
            Err(DistError::TooLarge { limit: 15, .. })
        ));
        assert_eq!(get(16).unwrap(), b"0123456789abcdef");
    }
    #[test]
    fn ac1_get_guarded_refuses_before_requesting_the_target() {
        let (base, seen) = serve(vec![("/a", 302, "/secret"), ("/secret", 200, "no")]);
        let dir = tempfile::tempdir().unwrap();
        let dist = Dist::open(dir.path(), Arc::new(NoSign), Options::default()).unwrap();
        // The first hop is the test server (local); only the targets are checked.
        let got = dist.get(
            &format!("{base}/a"),
            100,
            Duration::from_secs(5),
            false,
            true,
        );
        assert!(
            matches!(&got, Err(DistError::BadUrl { cause: "redirects to a local address", url }) if url == &format!("{base}/a")),
            "{got:?}"
        );
        assert_eq!(*seen.lock().unwrap(), ["/a"]);
    }
    #[test]
    fn s116_second_redirect_refusal_names_the_asked_url() {
        let dir = tempfile::tempdir().unwrap();
        let allowed = Host::serve(dir.path());
        let refused = Host::serve(dir.path());
        allowed.redirect("/asked", "/intermediate");
        allowed.redirect("/intermediate", &format!("{}secret", refused.url));
        let (_home, dist) = home("Eve", false);
        exempt(&dist, &[&allowed]);
        let asked = format!("{}asked", allowed.url);

        let error = dist
            .get(&asked, 100, Duration::from_secs(5), false, true)
            .unwrap_err();

        assert_redirect_refused(Some(error), &asked);
        assert_eq!(
            *allowed.requests.lock().unwrap(),
            ["/asked", "/intermediate"]
        );
        assert_eq!(refused.count(), 0);
    }
    #[test]
    fn ac3_unlisted_get_follows_a_local_redirect() {
        let (base, seen) = serve(vec![("/a", 302, "/b"), ("/b", 200, "ok")]);
        let dir = tempfile::tempdir().unwrap();
        let dist = Dist::open(dir.path(), Arc::new(NoSign), Options::default()).unwrap();
        let got = dist.get(
            &format!("{base}/a"),
            100,
            Duration::from_secs(5),
            false,
            false,
        );
        assert_eq!(got.unwrap(), b"ok");
        assert_eq!(*seen.lock().unwrap(), ["/a", "/b"]);
    }

    // The wiring tests below use real signed channels. Two kinds of local test server:
    // a "listed" host (exempted from the local rule through `test_local_ok`, so its
    // first hop is allowed) and a redirect target (local and not exempt, so refused).

    struct Yes;
    impl crate::Confirm for Yes {
        fn confirm(&self, _: &crate::SignRequest) -> crate::Decision {
            crate::Decision::Approve
        }
    }
    struct Host {
        url: String,
        requests: Arc<std::sync::Mutex<Vec<String>>>,
        redirects: Arc<std::sync::Mutex<std::collections::BTreeMap<String, String>>>,
    }
    impl Host {
        /// Serves `dir`; a path with an entry in `redirects` answers 302 to it.
        fn serve(dir: &std::path::Path) -> Self {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let url = format!("http://{}/", server.server_addr().to_ip().unwrap());
            let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
            let redirects = Arc::new(std::sync::Mutex::new(std::collections::BTreeMap::new()));
            let (log, map, dir) = (
                Arc::clone(&requests),
                Arc::clone(&redirects),
                dir.to_owned(),
            );
            std::thread::spawn(move || {
                for request in server.incoming_requests() {
                    let path = request.url().to_owned();
                    log.lock().unwrap().push(path.clone());
                    let target = map.lock().unwrap().get(&path).cloned();
                    let response = if let Some(target) = target {
                        tiny_http::Response::from_data(Vec::new())
                            .with_status_code(302)
                            .with_header(tiny_http::Header::from_bytes("Location", target).unwrap())
                    } else {
                        match std::fs::read(dir.join(path.trim_start_matches('/'))) {
                            Ok(bytes) => {
                                tiny_http::Response::from_data(bytes).with_status_code(200)
                            }
                            Err(_) => {
                                tiny_http::Response::from_data(Vec::new()).with_status_code(404)
                            }
                        }
                    };
                    let _ = request.respond(response);
                }
            });
            Self {
                url,
                requests,
                redirects,
            }
        }
        fn redirect(&self, path: &str, to: &str) {
            self.redirects
                .lock()
                .unwrap()
                .insert(path.to_owned(), to.to_owned());
        }
        fn count(&self) -> usize {
            self.requests.lock().unwrap().len()
        }
    }
    fn home(name: &str, listed_local: bool) -> (tempfile::TempDir, Dist) {
        let dir = tempfile::tempdir().unwrap();
        let dist = Dist::open(
            dir.path(),
            Arc::new(Yes),
            Options {
                listed_local,
                ..Options::default()
            },
        )
        .unwrap();
        dist.set_name(name).unwrap();
        (dir, dist)
    }
    fn exempt(dist: &Dist, hosts: &[&Host]) {
        *dist.test_local_ok.lock().unwrap() = hosts.iter().map(|h| h.url.clone()).collect();
    }
    struct World {
        _dirs: Vec<tempfile::TempDir>,
        publisher: Dist,
        key: crate::PublicKey,
        reference: crate::ReleaseRef,
        slug: crate::Slug,
        package: String,
        a: Host,
        b: Host,
        served: u64,
    }
    /// A publisher with one release, host `a` serving its site, host `b` a second copy.
    fn world() -> World {
        let (dir, publisher) = home("Carol", true);
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading-list");
        let bytes = watane_pkg::pack(&fixture, false).unwrap().into_bytes();
        publisher.publish(bytes).unwrap();
        let key = publisher.identity().unwrap();
        let info = publisher.channel(&key).unwrap().releases.remove(0);
        let site = publisher.site_dir();
        let served = std::fs::read_dir(site.join("entries")).unwrap().count() as u64 - 1;
        World {
            a: Host::serve(&site),
            b: Host::serve(&site),
            package: format!("packages/{}", info.release.package),
            slug: info.reference.slug.clone(),
            reference: info.reference,
            key,
            served,
            publisher,
            _dirs: vec![dir],
        }
    }
    impl World {
        fn link(&self, host: &Host) -> crate::Link {
            crate::Link::new(self.key, &host.url, Some(&self.slug)).unwrap()
        }
        /// A strict consumer (listed_local false) that trusts only `hosts` as first hops.
        fn eve(&self, hosts: &[&Host]) -> (tempfile::TempDir, Dist) {
            let (dir, eve) = home("Eve", false);
            exempt(&eve, hosts);
            (dir, eve)
        }
    }
    fn failure(result: Result<crate::RefreshReport, DistError>) -> Option<DistError> {
        match result {
            Err(cause)
            | Ok(crate::RefreshReport {
                outcome: crate::Outcome::Failed { cause },
                ..
            }) => Some(cause),
            Ok(_) => None,
        }
    }
    fn assert_refused(cause: Option<DistError>, target: &str) {
        assert_bad_url(cause, target, "local address");
    }
    /// A refused redirect names the URL asked for, and says the redirect was local.
    fn assert_redirect_refused(cause: Option<DistError>, target: &str) {
        assert_bad_url(cause, target, "redirects to a local address");
    }
    fn assert_bad_url(cause: Option<DistError>, target: &str, expected: &str) {
        match cause {
            Some(DistError::BadUrl { url, cause }) => {
                assert_eq!(url, target);
                assert_eq!(cause, expected);
            }
            other => panic!("{other:?}"),
        }
    }

    /// AC1 wiring: each GET of `read_channel_entries` passes its `listed` flag.
    #[test]
    fn ac1_listed_channel_read_refuses_a_local_redirect_at_each_get() {
        let w = world();
        let cases = [
            "head".to_owned(),
            "entries/0".to_owned(),
            format!("entries/{}", w.served),
        ];
        for path in cases {
            let (_dir, eve) = w.eve(&[&w.a]);
            let target = format!("{}{path}", w.b.url);
            w.a.redirects.lock().unwrap().clear();
            w.a.redirect(&format!("/{path}"), &target);
            let before = w.b.count();
            let got = failure(eve.read_channel_entries(w.key, &w.a.url, true));
            assert_redirect_refused(got, &format!("{}{path}", w.a.url));
            assert_eq!(w.b.count(), before, "{path}: no request to the target");
            // The same redirect of an unlisted (followed or chosen) base is followed.
            let (_dir, eve) = w.eve(&[&w.a]);
            let got = failure(eve.read_channel_entries(w.key, &w.a.url, false));
            assert!(got.is_none(), "{path}: {got:?}");
            assert!(w.b.count() > before, "{path}: the redirect was followed");
        }
    }
    /// AC2 wiring: the package GET of a provider base is a listed read, and a refused
    /// redirect moves to the next base.
    #[test]
    fn ac2_provider_package_redirect_local_falls_to_next_base() {
        let w = world();
        let a2 = Host::serve(&w.publisher.site_dir());
        let (_dir, eve) = w.eve(&[&w.a, &a2]);
        let target = format!("{}{}", w.b.url, w.package);
        w.a.redirect(&format!("/{}", w.package), &target);
        eve.list(&w.link(&w.a), "first").unwrap();
        // The second base comes from the consumer's channel, which Eve follows.
        let (_dave_dir, dave) = home("Dave", true);
        dave.list(&w.link(&a2), "second").unwrap();
        let dave_host = Host::serve(&dave.site_dir());
        let dave_link = crate::Link::new(dave.identity().unwrap(), &dave_host.url, None).unwrap();
        eve.follow(&dave_link).unwrap();
        let got = eve
            .fetch_for(&w.reference, Some(&dave.identity().unwrap()))
            .unwrap();
        assert_eq!(got.link, Some(w.link(&a2)));
        assert_eq!(w.b.count(), 0, "the redirect target gets no request");
    }
    /// A listed record for the publisher (through a chosen index that lists it at `a`).
    fn listed_record(w: &World, eve: &Dist) -> (tempfile::TempDir, Dist, Host) {
        let (dir, index) = home("Zed", true);
        index.list(&w.link(&w.a), "listed").unwrap();
        let host = Host::serve(&index.site_dir());
        eve.choose_index(&crate::Link::new(index.identity().unwrap(), &host.url, None).unwrap())
            .unwrap();
        (dir, index, host)
    }
    /// AC1/AC3 wiring in `read_link`: a listed record's package redirect is checked, a
    /// followed record's is not, and a link with no record is checked.
    #[test]
    fn ac1_read_link_package_redirect_by_record_kind() {
        let w = world();
        let target = format!("{}{}", w.b.url, w.package);
        w.a.redirect(&format!("/{}", w.package), &target);
        // Listed record.
        let (_dir, eve) = w.eve(&[&w.a]);
        let _z = listed_record(&w, &eve);
        let got = eve.read_link(&w.link(&w.a)).map(|_| ()).err();
        assert_redirect_refused(got, &format!("{}{}", w.a.url, w.package));
        assert_eq!(w.b.count(), 0);
        // No record.
        let (_dir, eve) = w.eve(&[&w.a]);
        let got = eve.read_link(&w.link(&w.a)).map(|_| ()).err();
        assert_redirect_refused(got, &format!("{}{}", w.a.url, w.package));
        assert_eq!(w.b.count(), 0);
        // Followed record: the participant's own base may redirect anywhere.
        let (_dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&w.a)).unwrap();
        let got = eve.read_link(&w.link(&w.a)).unwrap();
        assert_eq!(got.package.id().to_string(), w.package["packages/".len()..]);
        assert_eq!(w.b.count(), 1);
        // No record: the channel read of the link's base is a listed read too.
        let head = format!("{}head", w.b.url);
        w.a.redirect("/head", &head);
        let (_dir, eve) = w.eve(&[&w.a]);
        let got = eve.read_link(&w.link(&w.a)).map(|_| ()).err();
        assert_redirect_refused(got, &format!("{}head", w.a.url));
        assert_eq!(w.b.count(), 1);
    }
    /// AC5 / 5.9: a listed record whose base is local, with no provider base, is
    /// `BadUrl` and the base gets no request.
    #[test]
    fn ac5_listed_record_local_base_without_providers_is_refused() {
        let w = world();
        let (_dir, eve) = w.eve(&[&w.a]);
        let _z = listed_record(&w, &eve);
        // The index still lists the publisher at `a`, but `a` is no longer exempt.
        exempt(&eve, &[]);
        let before = w.a.count();
        let got = eve.fetch(&w.reference).map(|_| ()).err();
        match got {
            Some(DistError::BadUrl { cause, .. }) => assert_eq!(cause, "local address"),
            other => panic!("{other:?}"),
        }
        assert_eq!(w.a.count(), before);
    }
    /// Adversary S105: a listing's base stays in the record when Eve later follows the
    /// channel by its own link elsewhere, and is then read with unchecked redirects. Ivy's
    /// listed base must not lead dist into the local network (A35, 5.9: a followed
    /// record holds only the bases the participant gave).
    #[test]
    fn adv_s105_listed_base_kept_after_follow_redirects_local() {
        let w = world();
        let c = Host::serve(&w.publisher.site_dir());
        let (_dir, eve) = w.eve(&[&w.a]);
        let _z = listed_record(&w, &eve);
        // Ivy's base `a` now redirects the head and the package into the local network.
        w.a.redirect("/head", &format!("{}head", w.b.url));
        w.a.redirect(
            &format!("/{}", w.package),
            &format!("{}{}", w.b.url, w.package),
        );
        // Eve follows Carol by Carol's own link at `c` (her choice), not at `a`.
        eve.follow(&w.link(&c)).unwrap();
        let _ = eve.fetch(&w.reference);
        assert_eq!(
            w.b.count(),
            0,
            "a listing's base was read as a followed base: redirects into the local network were followed"
        );
    }
    /// AC2: a listed-only record that is chosen as an index keeps only the base given.
    #[test]
    fn ac2_choose_index_drops_listed_bases() {
        let w = world();
        let c = Host::serve(&w.publisher.site_dir());
        let (dir, eve) = w.eve(&[&w.a]);
        let _z = listed_record(&w, &eve);
        let before = w.a.count();
        let link = crate::Link::new(w.key, &c.url, None).unwrap();
        eve.choose_index(&link).unwrap();
        eve.refresh();
        let file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("dist/follows.json")).unwrap())
                .unwrap();
        let record = &file["channels"][w.key.to_string()];
        assert_eq!(record["base"], c.url.as_str());
        assert!(record["bases"].as_array().is_none_or(|b| b.is_empty()));
        assert_eq!(w.a.count(), before, "the listing's base was read again");
    }
    #[test]
    fn s107_explicit_links_can_give_a_base_already_held_by_a_listing() {
        for action in ["follow", "choose", "open"] {
            let w = world();
            let (dir, eve) = w.eve(&[&w.a]);
            let _index = listed_record(&w, &eve);
            exempt(&eve, &[]);
            let link = crate::Link::new(w.key, &w.a.url, None).unwrap();
            match action {
                "choose" => {
                    eve.choose_index(&link).unwrap();
                }
                "open" => {
                    eve.open_link(&w.link(&w.a)).unwrap();
                }
                _ => {
                    eve.follow(&link).unwrap();
                }
            }
            let file = follows_file(dir.path());
            assert_eq!(
                file["channels"][w.key.to_string()]["explicit_given"],
                serde_json::json!([w.a.url])
            );
        }
    }
    /// DD-47 as a listed record: an unknown local base still gives a cached package,
    /// and refuses the base on a miss.
    #[test]
    fn ac1_an_unknown_local_base_gives_a_cached_package() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&w.a)).unwrap();
        eve.fetch(&w.reference).unwrap();
        drop(eve);
        let old = serde_json::json!({"version": 1, "channels": {w.key.to_string(): {
            "base": w.a.url, "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
        std::fs::write(dir.path().join("dist/follows.json"), old.to_string()).unwrap();
        let eve = Dist::open(dir.path(), Arc::new(Yes), Options::default()).unwrap();
        let before = w.a.count();
        eve.fetch(&w.reference).unwrap();
        let package = w.package.trim_start_matches("packages/").to_owned();
        std::fs::remove_file(eve.cache_path(&package.parse().unwrap())).unwrap();
        assert_refused(eve.fetch(&w.reference).map(|_| ()).err(), &w.a.url);
        assert_eq!(w.a.count(), before, "no request to the unknown local base");
    }
    /// open_link at a given base reads it once, as a refresh; no extra read to learn it.
    #[test]
    fn ac1_open_link_at_a_given_base_reads_its_head_once() {
        let w = world();
        let (_dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&w.a)).unwrap();
        let heads = || {
            w.a.requests
                .lock()
                .unwrap()
                .iter()
                .filter(|p| *p == "/head")
                .count()
        };
        let before = heads();
        eve.open_link(&w.link(&w.a)).unwrap();
        assert_eq!(heads(), before + 1);
    }
    fn write_old(dir: &std::path::Path, w: &World) {
        let old = serde_json::json!({"version": 1, "channels": {w.key.to_string(): {
            "base": w.a.url, "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
        std::fs::write(dir.join("dist/follows.json"), old.to_string()).unwrap();
    }

    fn strict(dir: &std::path::Path) -> Dist {
        Dist::open(dir, Arc::new(Yes), Options::default()).unwrap()
    }

    #[test]
    fn s107_derived_follow_keeps_listed_policy_across_record_changes() {
        for change in ["legacy", "cap", "prune"] {
            let w = world();
            let (dir, eve) = w.eve(&[&w.a]);
            // Keep the fixture's HTTP reads local and inspect the saved authorization.
            eve.follow(&w.link(&w.b)).unwrap();
            drop(eve);
            write_old(dir.path(), &w);
            let eve = strict(dir.path());
            exempt(&eve, &[&w.a]);
            let hit = eve
                .search(&crate::Query {
                    publisher: Some(w.key),
                    ..Default::default()
                })
                .unwrap()
                .remove(0)
                .link
                .unwrap();
            let mirrors;
            let eve = match change {
                "cap" => {
                    // Start with the captured base behind another mirror.
                    drop(eve);
                    let path = dir.path().join("dist/follows.json");
                    let mut file = follows_file(dir.path());
                    let record = &mut file["channels"][w.key.to_string()];
                    record["base"] = serde_json::json!(w.b.url);
                    record["bases"] = serde_json::json!([w.b.url, w.a.url]);
                    std::fs::write(path, file.to_string()).unwrap();
                    let eve = strict(dir.path());
                    exempt(&eve, &[&w.a, &w.b]);
                    mirrors = (0..3)
                        .map(|_| Host::serve(&w.publisher.site_dir()))
                        .collect::<Vec<_>>();
                    for mirror in &mirrors {
                        eve.follow(&w.link(mirror)).unwrap();
                    }
                    let file = follows_file(dir.path());
                    let bases = file["channels"][w.key.to_string()]["bases"]
                        .as_array()
                        .unwrap();
                    assert_eq!(bases.len(), 4);
                    assert!(!bases.iter().any(|base| base == hit.base()));
                    eve
                }
                "prune" => {
                    eve.unfollow(&w.key).unwrap();
                    assert!(!eve.follows.lock().unwrap().contains_key(&w.key));
                    eve
                }
                _ => eve,
            };
            eve.follow_derived(&hit).unwrap();
            let file = follows_file(dir.path());
            let record = &file["channels"][w.key.to_string()];
            assert!(
                record["explicit_given"]
                    .as_array()
                    .is_none_or(|bases| !bases.iter().any(|base| base == hit.base())),
                "{change}: {record}"
            );
            assert!(file.get("seen").is_none(), "{change}: {file}");
            exempt(&eve, &[]);
            let before = w.a.count();
            eve.refresh_channel(&w.key).unwrap();
            assert_eq!(w.a.count(), before, "{change}: derived base was requested");
        }
    }

    #[test]
    fn s107_a_derived_index_uses_the_listed_address_policy() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a]);
        eve.follow_derived(&w.link(&w.a)).unwrap();
        let view = eve.followed().unwrap().remove(0);
        let link = crate::Link::new(view.channel, view.base.as_deref().unwrap(), None).unwrap();
        exempt(&eve, &[]);
        let before = w.a.count();
        assert_refused(eve.choose_index_derived(&link).err(), &w.a.url);
        assert_eq!(w.a.count(), before);
        exempt(&eve, &[&w.a]);
        eve.choose_index_derived(&link).unwrap();
        let file = follows_file(dir.path());
        let record = &file["channels"][w.key.to_string()];
        assert_eq!(record["index"], true);
        assert!(record.get("explicit_given").is_none(), "{record}");
    }

    #[test]
    fn s107_a_derived_index_can_read_an_explicitly_given_base() {
        let w = world();
        let (dir, eve) = w.eve(&[]);
        let link = crate::Link::new(w.key, &w.a.url, None).unwrap();
        eve.follow(&link).unwrap();
        eve.choose_index_derived(&link).unwrap();
        let file = follows_file(dir.path());
        let record = &file["channels"][w.key.to_string()];
        assert_eq!(record["index"], true);
        assert_eq!(record["explicit_given"], serde_json::json!([link.base()]));
    }

    #[test]
    fn s107_derived_follow_checks_the_initial_base() {
        let w = world();
        let (_dir, eve) = w.eve(&[]);
        assert_refused(eve.follow_derived(&w.link(&w.a)).err(), &w.a.url);
        assert_eq!(w.a.count(), 0);
    }

    /// F2.3: `read_link` gives a cached package for a refused listed base, as `fetch`
    /// does (DD-47), with no request; on a miss it returns the address refusal.
    #[test]
    fn adv_s105_r2_read_link_gives_a_cached_package_for_a_refused_listed_base() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&w.a)).unwrap();
        eve.fetch(&w.reference).unwrap();
        drop(eve);
        write_old(dir.path(), &w);
        let eve = strict(dir.path());
        let before = w.a.count();
        let got = eve.read_link(&w.link(&w.a)).unwrap();
        assert_eq!(got.package.id().to_string(), w.package["packages/".len()..]);
        assert!(
            got.link.is_none(),
            "a cached read names no base: {:?}",
            got.link
        );
        assert_eq!(w.a.count(), before, "no request to the unknown local base");
        let package = w.package.trim_start_matches("packages/").to_owned();
        std::fs::remove_file(eve.cache_path(&package.parse().unwrap())).unwrap();
        assert_refused(eve.read_link(&w.link(&w.a)).map(|_| ()).err(), &w.a.url);
        assert_eq!(w.a.count(), before, "no request on a miss either");
    }

    #[test]
    fn s107_a_failed_prune_save_keeps_the_record() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a]);
        let (_zdir, zed, _zhost) = listed_record(&w, &eve);
        zed.unlist(&zed.own().unwrap().listings[0].entry).unwrap();
        eve.refresh_channel(&zed.identity().unwrap()).unwrap();
        let file = follows_file(dir.path());
        *eve.test_fail_saves.lock().unwrap() = Some(0);
        assert!(eve.prune_listed().is_err());
        assert!(eve.follows.lock().unwrap().contains_key(&w.key));
        assert_eq!(follows_file(dir.path()), file);
    }

    /// F3.2 (Codex r3): a directory sync that fails after the rename still installed the
    /// new `follows.json`; the save reports the error, and the records in
    /// memory are the ones in the file.
    #[test]
    fn f3_2_a_failed_dir_sync_after_the_rename_keeps_memory_on_the_file() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a]);
        let (_zdir, zed, _zhost) = listed_record(&w, &eve);
        let zed_key = zed.identity().unwrap();
        let id = zed.own().unwrap().listings[0].entry;
        zed.unlist(&id).unwrap();
        eve.refresh_channel(&zed_key).unwrap();
        crate::site::TEST_FAIL_DIR_SYNC.with(|fail| fail.set(1));
        let err = eve.prune_listed().unwrap_err();
        assert!(
            matches!(&err, DistError::Io { source, .. } if source.raw_os_error() == Some(5)),
            "the save reports the sync error: {err:?}"
        );
        let file = follows_file(dir.path());
        assert!(file["channels"].get(w.key.to_string()).is_none(), "{file}");
        let records: Vec<String> = eve
            .follows
            .lock()
            .unwrap()
            .keys()
            .map(ToString::to_string)
            .collect();
        let in_file: Vec<String> = file["channels"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(records, in_file, "records in memory");
    }

    fn follows_file(dir: &std::path::Path) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(dir.join("dist/follows.json")).unwrap()).unwrap()
    }

    fn sync_fails(n: u32) {
        crate::site::TEST_FAIL_DIR_SYNC.with(|fail| fail.set(n));
    }
    fn assert_eio(result: Result<(), DistError>) {
        let err = result.unwrap_err();
        assert!(
            matches!(&err, DistError::Io { source, .. } if source.raw_os_error() == Some(5)),
            "{err:?}"
        );
    }

    /// F4.2 (Codex r4): a failed kept ingest whose kept mark was installed by a rename with
    /// a failed directory sync takes the mark back.
    #[test]
    fn f4_2_a_failed_ingest_takes_back_a_mark_whose_dir_sync_failed() {
        let w = world();
        let (dir, eve) = w.eve(&[]);
        // Entries from seq 1 on, with no seq 0 stored: the ingest has a gap and fails.
        let entries = w.publisher.entries(&w.key, 1, 100, usize::MAX).unwrap();
        sync_fails(1);
        let got = eve.ingest(&w.key, entries, crate::Reason::Kept);
        assert!(matches!(got, crate::Ingested::Failed(_)), "{got:?}");
        assert!(eve.log.head(&w.key).unwrap().is_none());
        assert!(!eve.follows.lock().unwrap().contains_key(&w.key));
        let file = follows_file(dir.path());
        assert!(file["channels"].get(w.key.to_string()).is_none(), "{file}");
    }

    /// F4.2 under a directory sync that keeps failing: the take back is recorded and done
    /// by the first call after the sync succeeds.
    #[test]
    fn f4_2_a_pending_take_back_is_finished_once_the_sync_succeeds() {
        let w = world();
        let (dir, eve) = w.eve(&[]);
        let entries = w.publisher.entries(&w.key, 1, 100, usize::MAX).unwrap();
        sync_fails(u32::MAX);
        let got = eve.ingest(&w.key, entries, crate::Reason::Kept);
        assert!(matches!(got, crate::Ingested::Failed(_)), "{got:?}");
        sync_fails(0);
        eve.refresh();
        assert!(!eve.follows.lock().unwrap().contains_key(&w.key));
        let file = follows_file(dir.path());
        assert!(file["channels"].get(w.key.to_string()).is_none(), "{file}");
    }

    /// F4.3 (Codex r4): `unfollow` whose flag change was installed by a rename with a
    /// failed directory sync; a retry finishes the prune instead of NothingToUnfollow.
    #[test]
    fn f4_3_a_retried_unfollow_finishes_the_prune() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&w.a)).unwrap();
        sync_fails(1);
        assert_eio(eve.unfollow(&w.key));
        eve.unfollow(&w.key).unwrap();
        let file = follows_file(dir.path());
        let key = w.key.to_string();
        assert!(file["channels"].get(&key).is_none(), "{file}");
    }

    /// F4.3 (Codex r4): the same for `drop_index`.
    #[test]
    fn f4_3_a_retried_drop_index_finishes_the_prune() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a]);
        let (_zdir, zed, _zhost) = listed_record(&w, &eve);
        let zed_key = zed.identity().unwrap();
        sync_fails(1);
        assert_eio(eve.drop_index(&zed_key));
        eve.drop_index(&zed_key).unwrap();
        let file = follows_file(dir.path());
        for key in [zed_key.to_string(), w.key.to_string()] {
            assert!(file["channels"].get(&key).is_none(), "{key}: {file}");
        }
    }

    // Review r5 (Claude). F5.1 to F5.3 are the reviewer's red tests as given.

    /// F5.1: a record a failed unfollow left is pruned before it is read, so the call
    /// answers NotFollowed with no request and no panic.
    #[test]
    fn v_refresh_channel_after_a_failed_unfollow_makes_no_request() {
        let w = world();
        let (_dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&w.a)).unwrap();
        sync_fails(1);
        assert_eio(eve.unfollow(&w.key));
        let before = w.a.count();
        let got = eve.refresh_channel(&w.key);
        assert!(matches!(got, Err(DistError::NotFollowed { .. })), "{got:?}");
        assert_eq!(w.a.count(), before, "a request for an unfollowed channel");
    }

    /// F5.2: `settle` returns the error when its own take-back leaves the store unsynced.
    #[test]
    fn v_a_take_back_that_leaves_the_store_unsynced_stops_the_call() {
        let w = world();
        let v = world();
        let (_dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&w.a)).unwrap();
        let entries = v.publisher.entries(&v.key, 1, 100, usize::MAX).unwrap();
        // The kept mark saves; its take back fails before the rename and is left pending.
        *eve.test_fail_saves.lock().unwrap() = Some(1);
        let got = eve.ingest(&v.key, entries, crate::Reason::Kept);
        assert!(matches!(got, crate::Ingested::Failed(_)), "{got:?}");
        *eve.test_fail_saves.lock().unwrap() = None;
        // The pending take back installs its file, but its directory sync fails.
        sync_fails(1);
        let before = w.a.count();
        assert_eio(eve.fetch(&w.reference).map(|_| ()));
        assert_eq!(w.a.count(), before, "a request while the store is unsynced");
    }

    /// P5.1: a retried unfollow answers Ok also when another call finished its prune.
    #[test]
    fn v_a_retried_unfollow_after_a_refresh_is_ok() {
        let w = world();
        let (_dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&w.a)).unwrap();
        sync_fails(1);
        assert_eio(eve.unfollow(&w.key));
        eve.refresh();
        eve.unfollow(&w.key).unwrap();
    }

    /// P5.1: as an unfollow, a retried drop_index after a refresh.
    #[test]
    fn v_a_retried_drop_index_after_a_refresh_is_ok() {
        let w = world();
        let (_dir, eve) = w.eve(&[&w.a]);
        eve.choose_index(&crate::Link::new(w.key, &w.a.url, None).unwrap())
            .unwrap();
        sync_fails(1);
        assert_eio(eve.drop_index(&w.key));
        eve.refresh();
        eve.drop_index(&w.key).unwrap();
    }

    /// P5.2: while unsynced, an asked ingest stores (it changes no record) and a kept one,
    /// which marks a record, does not.
    #[test]
    fn v_while_unsynced_an_asked_ingest_stores_and_a_kept_one_does_not() {
        let w = world();
        let (_dir, eve) = w.eve(&[]);
        eve.unsynced.lock().unwrap().dirty = true;
        sync_fails(u32::MAX);
        let entries = w.publisher.entries(&w.key, 0, 100, usize::MAX).unwrap();
        let kept = eve.ingest(&w.key, entries.clone(), crate::Reason::Kept);
        assert!(matches!(kept, crate::Ingested::Failed(_)), "{kept:?}");
        assert!(eve.log.head(&w.key).unwrap().is_none());
        let asked = eve.ingest(&w.key, entries, crate::Reason::Asked);
        sync_fails(0);
        assert!(matches!(asked, crate::Ingested::Stored(_)), "{asked:?}");
    }

    /// The r5 gate, one test per entry point: an unsynced store whose sync keeps failing
    /// returns the Io error and requests nothing.
    fn unsynced_world(followed: bool) -> (World, tempfile::TempDir, Dist) {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a, &w.b]);
        if followed {
            eve.follow(&w.link(&w.a)).unwrap();
        }
        eve.unsynced.lock().unwrap().dirty = true;
        sync_fails(u32::MAX);
        (w, dir, eve)
    }
    fn assert_no_requests(w: &World, result: Result<(), DistError>) {
        sync_fails(0);
        assert_eq!(
            (w.a.count(), w.b.count()),
            (0, 0),
            "a request while unsynced"
        );
        assert_eio(result);
    }
    fn requests_before(w: &World) -> (usize, usize) {
        (w.a.count(), w.b.count())
    }
    #[test]
    fn v_gate_read_link() {
        let (w, _dir, eve) = unsynced_world(false);
        assert_no_requests(&w, eve.read_link(&w.link(&w.a)).map(|_| ()));
    }
    #[test]
    fn v_gate_read_link_followed() {
        let (w, _dir, eve) = unsynced_world(true);
        let before = requests_before(&w);
        let got = eve.read_link(&w.link(&w.a)).map(|_| ());
        sync_fails(0);
        assert_eq!(requests_before(&w), before, "a request while unsynced");
        assert_eio(got);
    }
    #[test]
    fn v_gate_fetch() {
        let (w, _dir, eve) = unsynced_world(true);
        let before = requests_before(&w);
        let got = eve.fetch(&w.reference).map(|_| ());
        sync_fails(0);
        assert_eq!(requests_before(&w), before, "a request while unsynced");
        assert_eio(got);
    }
    #[test]
    fn v_gate_follow() {
        let (w, _dir, eve) = unsynced_world(false);
        assert_no_requests(&w, eve.follow(&w.link(&w.a)).map(|_| ()));
    }
    #[test]
    fn v_gate_choose_index() {
        let (w, _dir, eve) = unsynced_world(false);
        let link = crate::Link::new(w.key, &w.a.url, None).unwrap();
        assert_no_requests(&w, eve.choose_index(&link).map(|_| ()));
    }
    #[test]
    fn v_gate_refresh_channel() {
        let (w, _dir, eve) = unsynced_world(true);
        let before = requests_before(&w);
        let got = eve.refresh_channel(&w.key).map(|_| ());
        sync_fails(0);
        assert_eq!(requests_before(&w), before, "a request while unsynced");
        assert_eio(got);
    }
    #[test]
    fn v_gate_refresh() {
        let (w, _dir, eve) = unsynced_world(true);
        let before = requests_before(&w);
        let reports = eve.refresh();
        sync_fails(0);
        assert_eq!(requests_before(&w), before, "a request while unsynced");
        assert!(!reports.is_empty());
        for report in reports {
            match report.outcome {
                crate::Outcome::Failed { cause } => assert_eio(Err(cause)),
                other => panic!("{other:?}"),
            }
        }
    }
    #[test]
    fn s112_refresh_channel_reports_a_pending_sync_after_its_base_loop() {
        let w = world();
        let (_dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&w.a)).unwrap();
        {
            let mut records = eve.follows.lock().unwrap();
            let mut next = records.clone();
            next.get_mut(&w.key).unwrap()._last_attempt = 0;
            eve.save_follows(&mut records, next).unwrap()
        };
        let before = w.a.count();

        // The head read succeeds, but publishing its updated attempt time
        // installs the file and leaves a pending directory sync.
        sync_fails(u32::MAX);
        let got = eve.refresh_channel(&w.key).map(|_| ());
        sync_fails(0);

        assert!(w.a.count() > before);
        assert_eio(got);
    }

    #[test]
    fn s112_open_link_prunes_a_stale_record_before_deciding_whether_to_follow() {
        let w = world();
        let (_dir, eve) = w.eve(&[&w.a]);
        let link = w.link(&w.a);
        eve.follow(&link).unwrap();
        sync_fails(1);
        assert_eio(eve.unfollow(&w.key));
        assert!(eve.follows.lock().unwrap().contains_key(&w.key));

        let fetched = eve.open_link(&link).unwrap();
        assert_eq!(
            fetched.package.id().to_string(),
            w.package["packages/".len()..]
        );
        assert!(eve.follows.lock().unwrap().get(&w.key).unwrap().followed);
    }

    #[test]
    fn s112_refresh_stops_at_the_first_settlement_failure() {
        let (w, _dir, eve) = unsynced_world(true);
        sync_fails(1);
        let before = requests_before(&w);
        let reports = eve.refresh();
        assert_eq!(requests_before(&w), before);
        assert_eq!(reports.len(), 1);
        for report in reports {
            assert_eq!(report.channel, w.key);
            match report.outcome {
                crate::Outcome::Failed { cause } => assert_eio(Err(cause)),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn s112_own_kept_ingest_needs_no_record_settlement() {
        let w = world();
        let key = w.key;
        let entries = w.publisher.entries(&key, 0, 100, usize::MAX).unwrap();
        w.publisher.unsynced.lock().unwrap().dirty = true;
        sync_fails(u32::MAX);
        let got = w.publisher.ingest(&key, entries, crate::Reason::Kept);
        sync_fails(0);
        assert!(matches!(got, crate::Ingested::Stored(_)), "{got:?}");
        assert!(!w.publisher.follows.lock().unwrap().contains_key(&key));
    }

    #[test]
    fn v_gate_open_link_at_a_new_base() {
        let (w, _dir, eve) = unsynced_world(true);
        let before = requests_before(&w);
        let got = eve.open_link(&w.link(&w.b)).map(|_| ());
        sync_fails(0);
        assert_eq!(requests_before(&w), before, "a request while unsynced");
        assert_eio(got);
    }
}
