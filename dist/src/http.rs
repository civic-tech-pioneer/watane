//! Reads from a web host (TECHNICAL 4.5): GET with a body limit and a
//! whole-request timeout on every request. Outgoing only; nothing listens.

use crate::{Dist, DistError, Entry, PublicKey};
use std::time::{Duration, Instant};

/// The most redirects one read follows (TECHNICAL 4.5).
pub(crate) const MAX_REDIRECTS: usize = 5;

/// The local-address rule applied to one redirect target.
pub(crate) type Check<'a> = dyn Fn(&str) -> Result<(), DistError> + 'a;

/// What one hop of a listed read answers: the bytes, or the next URL.
pub(crate) enum Hop<T> {
    Done(T),
    Redirect(String),
}

/// Follow up to `MAX_REDIRECTS` redirects by hand: each `Location` is resolved against
/// the current URL and passes `check` before any request to it (TECHNICAL 4.5).
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
        let check = |target: &str| self.check_listed_base(target);
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
        let Some(check) = check else {
            return match self.get_hop(url, limit, timeout, head, false)? {
                Hop::Done(bytes) => Ok(bytes),
                Hop::Redirect(_) => unreachable!("the agent follows redirects"),
            };
        };
        // One deadline for all hops.
        let deadline = Instant::now() + timeout;
        follow_hops(url, check, |current| {
            let left = deadline.saturating_duration_since(Instant::now());
            self.get_hop(current, limit, left, head, true)
        })
    }
    fn get_hop(
        &self,
        url: &str,
        limit: u64,
        timeout: Duration,
        head: bool,
        by_hand: bool,
    ) -> Result<Hop<Vec<u8>>, DistError> {
        // `by_hand`: the listed agent follows no redirect; the caller checks each target.
        let agent = if by_hand {
            &self.listed_agent
        } else {
            &self.agent
        };
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
            300 | 301 | 302 | 303 | 307 | 308 if by_hand => {
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
            matches!(&got, Err(DistError::BadUrl { cause: "local address", url }) if url == &format!("{base}/secret")),
            "{got:?}"
        );
        assert_eq!(*seen.lock().unwrap(), ["/a"]);
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
        match cause {
            Some(DistError::BadUrl { url, cause }) => {
                assert_eq!(url, target);
                assert_eq!(cause, "local address");
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
            assert_refused(got, &target);
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
        assert_refused(got, &target);
        assert_eq!(w.b.count(), 0);
        // No record.
        let (_dir, eve) = w.eve(&[&w.a]);
        let got = eve.read_link(&w.link(&w.a)).map(|_| ()).err();
        assert_refused(got, &target);
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
        assert_refused(got, &head);
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
    /// AC4 (DD-48): a listed-only record that a link at the listing's base turns followed
    /// or chosen takes that base but does not give it; the first read is listed too.
    #[test]
    fn ac4_a_link_at_the_listed_base_gives_nothing() {
        for choose in [false, true] {
            let w = world();
            let (dir, eve) = w.eve(&[&w.a]);
            let _z = listed_record(&w, &eve);
            let link = crate::Link::new(w.key, &w.a.url, None).unwrap();
            let take = |eve: &Dist| match choose {
                true => eve.choose_index(&link).map(|_| ()),
                false => eve.follow(&w.link(&w.a)).map(|_| ()),
            };
            w.a.redirect("/head", &format!("{}head", w.b.url));
            assert_refused(take(&eve).err(), &format!("{}head", w.b.url));
            assert_eq!(w.b.count(), 0, "the first read was not listed");
            w.a.redirects.lock().unwrap().clear();
            take(&eve).unwrap();
            let file: serde_json::Value = serde_json::from_slice(
                &std::fs::read(dir.path().join("dist/follows.json")).unwrap(),
            )
            .unwrap();
            let record = &file["channels"][w.key.to_string()];
            assert_eq!(record["base"], w.a.url.as_str());
            assert!(record["given"].as_array().is_none_or(|g| g.is_empty()));
            w.a.redirect("/head", &format!("{}head", w.b.url));
            let report = eve.refresh_channel(&w.key).unwrap();
            assert_refused(failure(Ok(report)), &format!("{}head", w.b.url));
            assert_eq!(w.b.count(), 0, "the listing's base was given");
        }
    }
    /// AC1 (F1.1): a record without `given` (saved before S105) is read under the listed
    /// rule, redirects included; no link gives its seen base (DD-48).
    #[test]
    fn ac2_ac3_an_old_followed_record_base_is_never_given_by_a_link() {
        let w = world();
        let c = Host::serve(&w.publisher.site_dir());
        let (dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&c)).unwrap();
        drop(eve);
        let old = serde_json::json!({"version": 1, "channels": {w.key.to_string(): {
            "base": w.a.url, "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
        std::fs::write(dir.path().join("dist/follows.json"), old.to_string()).unwrap();
        let eve = Dist::open(dir.path(), Arc::new(Yes), Options::default()).unwrap();
        exempt(&eve, &[&w.a]);
        w.a.redirect("/head", &format!("{}head", w.b.url));
        w.a.redirect(
            &format!("/{}", w.package),
            &format!("{}{}", w.b.url, w.package),
        );
        let report = eve.refresh_channel(&w.key).unwrap();
        assert_refused(failure(Ok(report)), &format!("{}head", w.b.url));
        let got = eve.fetch(&w.reference).map(|_| ()).err();
        assert_refused(got, &format!("{}{}", w.b.url, w.package));
        assert_eq!(w.b.count(), 0, "no request to a redirect target");
        // AC2: a follow at the held base gives nothing; the redirect is still refused.
        let got = eve.follow(&w.link(&w.a)).map(|_| ());
        assert!(got.is_err(), "the listed rule refuses the local redirect");
        let index = crate::Link::new(w.key, &w.a.url, None).unwrap();
        assert!(eve.choose_index(&index).is_err(), "so does choose_index");
        let report = eve.refresh_channel(&w.key).unwrap();
        assert_refused(failure(Ok(report)), &format!("{}head", w.b.url));
        assert_eq!(w.b.count(), 0, "a repeated link gives no base");
        // DD-48: unfollow, then follow by a link at the seen base gives nothing either.
        eve.unfollow(&w.key).unwrap();
        let redirects = w.a.redirects.lock().unwrap().clone();
        w.a.redirects.lock().unwrap().clear();
        eve.follow(&w.link(&w.a)).unwrap();
        *w.a.redirects.lock().unwrap() = redirects;
        let report = eve.refresh_channel(&w.key).unwrap();
        assert_refused(failure(Ok(report)), &format!("{}head", w.b.url));
        let got = eve.fetch(&w.reference).map(|_| ()).err();
        assert_refused(got, &format!("{}{}", w.b.url, w.package));
        assert_eq!(w.b.count(), 0, "a link never gives a seen base");
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
    /// Adversary S105 r3: "Install a hit" (5.10) follows `hit.link`, whose base is the
    /// record's front base. For a record saved before S105 that base is unknown (DD-47);
    /// the install marks it `given` though the participant never gave a link at it, and
    /// its redirects to a local address are then followed unchecked.
    #[test]
    fn adv_s105_r3_hit_install_gives_an_unknown_base() {
        let w = world();
        let c = Host::serve(&w.publisher.site_dir());
        let (dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&c)).unwrap();
        drop(eve);
        let old = serde_json::json!({"version": 1, "channels": {w.key.to_string(): {
            "base": w.a.url, "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
        std::fs::write(dir.path().join("dist/follows.json"), old.to_string()).unwrap();
        let eve = Dist::open(dir.path(), Arc::new(Yes), Options::default()).unwrap();
        exempt(&eve, &[&w.a]);
        let hits = eve
            .search(&crate::Query {
                text: String::new(),
                publisher: Some(w.key),
                origin: None,
                interface: None,
            })
            .unwrap();
        let link = hits[0].link.clone().expect("hit link");
        assert_eq!(link.base(), w.a.url, "the hit names the unknown base");
        // The app installs the hit: follow(hit.link), then fetch (5.10).
        let _ = eve.follow(&link);
        w.a.redirect("/head", &format!("{}head", w.b.url));
        let before = w.b.count();
        let report = eve.refresh_channel(&w.key).unwrap();
        assert_refused(failure(Ok(report)), &format!("{}head", w.b.url));
        assert_eq!(
            w.b.count(),
            before,
            "no request to the local redirect target"
        );
    }

    /// Adversary S105 r4: the 4-base cap drops an unknown (listing-learned) base after a
    /// search named it; installing that stale hit then finds the base "new to the record"
    /// and marks it given, so its local redirects are followed (DD-47, 5.9).
    #[test]
    fn adv_s105_r4_cap_evicts_unknown_base_then_hit_install_gives_it() {
        let w = world();
        let c = Host::serve(&w.publisher.site_dir());
        let e = Host::serve(&w.publisher.site_dir());
        let (dir, eve) = w.eve(&[&w.a, &c, &e]);
        eve.follow(&w.link(&c)).unwrap();
        drop(eve);
        // A home saved before S105: four bases, none given; `a` is a listing-learned one.
        let old = serde_json::json!({"version": 1, "channels": {w.key.to_string(): {
            "base": w.a.url, "bases": [w.a.url, c.url, "http://127.0.0.1:9/", "http://127.0.0.1:8/"],
            "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
        std::fs::write(dir.path().join("dist/follows.json"), old.to_string()).unwrap();
        let eve = Dist::open(dir.path(), Arc::new(Yes), Options::default()).unwrap();
        exempt(&eve, &[&w.a, &c, &e]);
        let hits = eve
            .search(&crate::Query {
                text: String::new(),
                publisher: Some(w.key),
                origin: None,
                interface: None,
            })
            .unwrap();
        let hit = hits[0].link.clone().expect("hit link");
        assert_eq!(hit.base(), w.a.url, "the hit names the unknown base");
        // `a` fails one refresh, so `c` moves to the front and `a` to index 1.
        w.a.redirect("/head", &format!("{}head", w.b.url));
        let _ = eve.refresh_channel(&w.key);
        w.a.redirects.lock().unwrap().clear();
        // The participant opens a link at a new base: the cap drops `a`.
        let _ = eve.open_link(&w.link(&e));
        // The app installs the hit it showed earlier: follow(hit.link) (5.10).
        let _ = eve.follow(&hit);
        w.a.redirect("/head", &format!("{}head", w.b.url));
        let before = w.b.count();
        let _ = eve.refresh_channel(&w.key);
        assert_eq!(
            w.b.count(),
            before,
            "no request to the local redirect target of a listing-learned base"
        );
    }

    /// Adversary S105 r4 (prune): `unfollow` prunes the record, but its `seen` stays as a
    /// tombstone, so a kept hit link makes a new record that gives nothing (DD-48).
    #[test]
    fn adv_s105_r4_prune_then_hit_install_gives_nothing() {
        let w = world();
        let c = Host::serve(&w.publisher.site_dir());
        let (dir, eve) = w.eve(&[&w.a]);
        eve.follow(&w.link(&c)).unwrap();
        drop(eve);
        let old = serde_json::json!({"version": 1, "channels": {w.key.to_string(): {
            "base": w.a.url, "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
        std::fs::write(dir.path().join("dist/follows.json"), old.to_string()).unwrap();
        let eve = Dist::open(dir.path(), Arc::new(Yes), Options::default()).unwrap();
        exempt(&eve, &[&w.a]);
        let hits = eve
            .search(&crate::Query {
                text: String::new(),
                publisher: Some(w.key),
                origin: None,
                interface: None,
            })
            .unwrap();
        let hit = hits[0].link.clone().expect("hit link");
        assert_eq!(hit.base(), w.a.url);
        eve.unfollow(&w.key).unwrap();
        let file = || -> serde_json::Value {
            serde_json::from_slice(&std::fs::read(dir.path().join("dist/follows.json")).unwrap())
                .unwrap()
        };
        let key = w.key.to_string();
        assert!(
            file()["channels"].get(&key).is_none(),
            "the record is pruned"
        );
        assert_eq!(file()["seen"][&key]["seen"], serde_json::json!([w.a.url]));
        // The first read of the tombstone's seen base is listed: the redirect is refused.
        w.a.redirect("/head", &format!("{}head", w.b.url));
        assert_refused(eve.follow(&hit).err(), &format!("{}head", w.b.url));
        w.a.redirects.lock().unwrap().clear();
        let _ = eve.follow(&hit);
        let record = &file()["channels"][&key];
        assert!(record["given"].as_array().is_none_or(|g| g.is_empty()));
        assert!(
            file()["seen"].get(&key).is_none(),
            "the tombstone is used up"
        );
        w.a.redirect("/head", &format!("{}head", w.b.url));
        let before = w.b.count();
        let _ = eve.refresh_channel(&w.key);
        assert_eq!(w.b.count(), before, "no request to the redirect target");
    }

    /// Adversary S105 r5: an index moves its listing of a channel off base `a` to a fresh
    /// base each refresh (unlist, list again); 16 such refreshes push `a` out of the
    /// listed record's 16-entry `seen`. A hit the app showed while `a` was listed is then
    /// installed: `a` looks never seen, the link gives it, and its redirects into the
    /// local network are followed (A35, DD-48).
    #[test]
    fn adv_s105_r5_listing_churn_evicts_base_from_seen_then_stale_hit_gives_it() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a]);
        let (_zdir, zed, _zhost) = listed_record(&w, &eve);
        let hits = eve
            .search(&crate::Query {
                text: String::new(),
                publisher: Some(w.key),
                origin: None,
                interface: None,
            })
            .unwrap();
        let hit = hits[0].link.clone().expect("hit link");
        assert_eq!(hit.base(), w.a.url, "the hit names the listing's base");
        let mut listed = zed.own().unwrap().listings[0].entry;
        for round in 0..16 {
            zed.unlist(&listed).unwrap();
            let base = format!("{}j{round}/", w.a.url);
            let link = crate::Link::new(w.key, &base, Some(&w.slug)).unwrap();
            listed = zed.list(&link, "moved").unwrap().id();
            eve.refresh();
        }
        let file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("dist/follows.json")).unwrap())
                .unwrap();
        let record = file["channels"][w.key.to_string()].clone();
        // The app installs the hit it showed earlier (5.10): follow(hit.link), then fetch.
        w.a.redirect("/head", &format!("{}head", w.b.url));
        w.a.redirect(
            &format!("/{}", w.package),
            &format!("{}{}", w.b.url, w.package),
        );
        let _ = eve.follow(&hit);
        let _ = eve.refresh_channel(&w.key);
        assert_eq!(
            w.b.count(),
            0,
            "a listing's base pushed out of `seen` was given by a stale hit: {record}"
        );
    }

    // Codex review r2 of PR #19 (F2.1 to F2.4) and S105-CHURN "(a) fail closed".

    /// Rewrites `follows.json` of `dir` as a home saved before S105: one record for the
    /// publisher at base `a`, nothing in `given`.
    fn write_old(dir: &std::path::Path, w: &World, record: serde_json::Value) {
        let old = serde_json::json!({"version": 1, "channels": {w.key.to_string(): record}});
        std::fs::write(dir.join("dist/follows.json"), old.to_string()).unwrap();
    }
    fn old_record(w: &World, given: &[&str], index: bool) -> serde_json::Value {
        serde_json::json!({"base": w.a.url, "followed": !index, "index": index, "given": given,
            "followed_at": 1, "last_attempt": 1, "last_success": 1})
    }
    fn strict(dir: &std::path::Path) -> Dist {
        Dist::open(dir, Arc::new(Yes), Options::default()).unwrap()
    }
    fn tomb(seen: &[&str], at: u64) -> crate::Tombstone {
        crate::Tombstone {
            seen: seen.iter().map(|b| (*b).to_owned()).collect(),
            given: Vec::new(),
            seen_full: false,
            at,
        }
    }
    /// A home that saw the publisher only through a pre-S105 record at `a`, then pruned it:
    /// a tombstone `seen = [a]`, and the hit link the app showed before.
    fn pruned_old_record(w: &World) -> (tempfile::TempDir, Dist, crate::Link) {
        let c = Host::serve(&w.publisher.site_dir());
        let (dir, eve) = w.eve(&[&w.a, &c]);
        eve.follow(&w.link(&c)).unwrap();
        drop(eve);
        write_old(dir.path(), w, old_record(w, &[], false));
        let eve = strict(dir.path());
        exempt(&eve, &[&w.a]);
        let hits = eve
            .search(&crate::Query {
                text: String::new(),
                publisher: Some(w.key),
                origin: None,
                interface: None,
            })
            .unwrap();
        let hit = hits[0].link.clone().expect("hit link");
        assert_eq!(hit.base(), w.a.url);
        eve.unfollow(&w.key).unwrap();
        assert!(eve.test_tomb_keys().contains(&w.key.to_string()));
        (dir, eve, hit)
    }

    /// F2.1: a listed read of a first base that is itself local gets no request, at follow
    /// and choose too.
    fn local_first_base_gets_no_request(choose: bool) {
        let w = world();
        let c = Host::serve(&w.publisher.site_dir());
        let (dir, eve) = w.eve(&[&c]);
        eve.follow(&w.link(&c)).unwrap();
        drop(eve);
        write_old(dir.path(), &w, old_record(&w, &[], choose));
        let eve = strict(dir.path());
        let before = w.a.count();
        let link = crate::Link::new(w.key, &w.a.url, None).unwrap();
        let got = if choose {
            eve.choose_index(&link).map(|_| ()).err()
        } else {
            eve.follow(&link).map(|_| ()).err()
        };
        assert_eq!(w.a.count(), before, "the local base was requested");
        assert_refused(got, &w.a.url);
    }
    #[test]
    fn adv_s105_r2_follow_of_a_listed_local_first_base_gets_no_request() {
        local_first_base_gets_no_request(false);
    }
    #[test]
    fn adv_s105_r2_choose_index_of_a_listed_local_first_base_gets_no_request() {
        local_first_base_gets_no_request(true);
    }
    /// F2.1: a base the participant gave still reads a local base (a LAN mirror).
    #[test]
    fn adv_s105_r2_a_given_local_first_base_is_still_read() {
        let w = world();
        let c = Host::serve(&w.publisher.site_dir());
        let (dir, eve) = w.eve(&[&c]);
        eve.follow(&w.link(&c)).unwrap();
        drop(eve);
        write_old(dir.path(), &w, old_record(&w, &[&w.a.url], false));
        let eve = strict(dir.path());
        eve.follow(&w.link(&w.a)).unwrap();
        assert!(w.a.count() > 0, "a given local base is read");
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
        write_old(dir.path(), &w, old_record(&w, &[], false));
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

    /// F2.2: `held_ungiven` classifies from one snapshot. A `keep` that moves the
    /// tombstone into a record between the two lookups must not turn a seen, never-given
    /// base into a given-looking one: at the gap between the record and the tombstone
    /// lookup, the classifier still holds the follows lock, so no `keep` can run there
    /// (Codex r3 P3.2: shown by the lock itself, with no timeout).
    #[test]
    fn adv_s105_r2_held_ungiven_reads_one_snapshot() {
        let w = world();
        let (_dir, eve, _hit) = pruned_old_record(&w);
        let (gap_tx, gap_rx) = std::sync::mpsc::channel::<()>();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let done_rx = std::sync::Mutex::new(done_rx);
        *eve.test_held_gap.lock().unwrap() = Some(Box::new(move || {
            gap_tx.send(()).unwrap();
            done_rx.lock().unwrap().recv().unwrap();
        }));
        let (locked, held) = std::thread::scope(|s| {
            let classify = s.spawn(|| eve.held_ungiven(w.key, &w.a.url));
            gap_rx.recv().unwrap();
            let locked = matches!(
                eve.follows.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            );
            done_tx.send(()).unwrap();
            (locked, classify.join().unwrap())
        });
        assert!(locked, "the follows lock was free at the lookup gap");
        assert!(held, "a seen, never-given base was classified as given");
        eve.keep(&w.key).unwrap();
        assert!(
            eve.held_ungiven(w.key, &w.a.url),
            "the kept record forgot the tombstone"
        );
    }

    /// F2.4: a failed save leaves the tombstones as they were, also when 1024 were held.
    #[test]
    fn adv_s105_r2_a_failed_save_leaves_the_tombstones_in_a_prune() {
        for held in [0u64, 1024] {
            let w = world();
            let (dir, eve) = w.eve(&[&w.a]);
            let (_zdir, zed, _zhost) = listed_record(&w, &eve);
            for n in 0..held {
                eve.test_add_tombstone(&format!("k{n}"), tomb(&[], n + 2));
            }
            let id = zed.own().unwrap().listings[0].entry;
            zed.unlist(&id).unwrap();
            eve.refresh_channel(&zed.identity().unwrap()).unwrap();
            let before = eve.test_tomb_keys();
            let file = std::fs::read(dir.path().join("dist/follows.json")).unwrap();
            *eve.test_fail_saves.lock().unwrap() = Some(0);
            assert!(eve.prune_listed().is_err());
            assert_eq!(eve.test_tomb_keys(), before, "held={held}");
            let after = std::fs::read(dir.path().join("dist/follows.json")).unwrap();
            assert_eq!(after, file, "held={held}: the file changed");
        }
    }

    /// F3.2 (Codex r3): a directory sync that fails after the rename still installed the
    /// new `follows.json`; the save reports the error, and the records and tombstones in
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
        assert!(!eve.test_tomb_keys().contains(&w.key.to_string()));
        crate::site::TEST_FAIL_DIR_SYNC.with(|fail| fail.set(1));
        let err = eve.prune_listed().unwrap_err();
        assert!(
            matches!(&err, DistError::Io { source, .. } if source.raw_os_error() == Some(5)),
            "the save reports the sync error: {err:?}"
        );
        let file = follows_file(dir.path());
        let on_disk: Vec<String> = file["seen"]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        assert!(
            on_disk.contains(&w.key.to_string()),
            "the prune was installed: {file}"
        );
        assert_eq!(eve.test_tomb_keys(), on_disk, "tombstones in memory");
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

    /// F2.4: a failed kept ingest that cannot take its record back leaves the tombstones
    /// as the first save left them (the record took its channel's tombstone).
    #[test]
    fn adv_s105_r2_a_failed_save_leaves_the_tombstones_in_take_back_kept() {
        for held in [0u64, 1023] {
            let w = world();
            let (dir, eve) = w.eve(&[]);
            let key = w.key.to_string();
            eve.test_add_tombstone(&key, tomb(&[&w.a.url], 1));
            for n in 0..held {
                eve.test_add_tombstone(&format!("k{n}"), tomb(&[], n + 2));
            }
            let mut expected = eve.test_tomb_keys();
            expected.retain(|k| *k != key);
            // Entries from seq 1 on, with no seq 0 stored: the ingest has a gap and fails.
            let entries = w.publisher.entries(&w.key, 1, 100, usize::MAX).unwrap();
            assert!(!entries.is_empty());
            // The first save (the kept mark) passes, the take back fails.
            *eve.test_fail_saves.lock().unwrap() = Some(1);
            let got = eve.ingest(&w.key, entries, crate::Reason::Kept);
            assert!(matches!(got, crate::Ingested::Failed(_)), "{got:?}");
            assert_eq!(eve.test_tomb_keys(), expected, "held={held}");
            let file: serde_json::Value = serde_json::from_slice(
                &std::fs::read(dir.path().join("dist/follows.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(file["channels"][&key]["kept"], true);
            assert!(file["seen"].get(&key).is_none());
        }
    }

    /// S105-CHURN (a): once the tombstone of a channel is evicted, a stale hit link at its
    /// seen base gives nothing (the DELIVERED RISK of the churn route).
    #[test]
    fn adv_s105_churn_tombstone_overflow_then_stale_hit_gives_nothing() {
        let w = world();
        let (dir, eve, hit) = pruned_old_record(&w);
        let key = w.key.to_string();
        for n in 0..1024 {
            eve.test_add_tombstone(&format!("k{n}"), tomb(&[], u64::MAX - 5000 + n));
        }
        assert!(
            !eve.test_tomb_keys().contains(&key),
            "the tombstone is evicted"
        );
        w.a.redirect("/head", &format!("{}head", w.b.url));
        let first = eve.follow(&hit).map(|_| ()).err();
        assert_eq!(w.b.count(), 0, "the first read followed a local redirect");
        assert_refused(first, &format!("{}head", w.b.url));
        w.a.redirects.lock().unwrap().clear();
        eve.follow(&hit).unwrap();
        let file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("dist/follows.json")).unwrap())
                .unwrap();
        let record = &file["channels"][&key];
        assert!(record["given"].as_array().is_none_or(|g| g.is_empty()));
        w.a.redirect("/head", &format!("{}head", w.b.url));
        let report = eve.refresh_channel(&w.key).unwrap();
        assert_refused(failure(Ok(report)), &format!("{}head", w.b.url));
        assert_eq!(w.b.count(), 0, "the stale hit gave its base");
    }

    /// S105-CHURN (a): a channel dist never knew still gets its base from a link, also
    /// after tombstones overflowed.
    #[test]
    fn adv_s105_churn_after_overflow_a_fresh_channel_link_gives_its_base() {
        let w = world();
        let (dir, eve) = w.eve(&[]);
        for n in 0..1100 {
            eve.test_add_tombstone(&format!("k{n}"), tomb(&[], n + 2));
        }
        eve.follow(&w.link(&w.a)).unwrap();
        let file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("dist/follows.json")).unwrap())
                .unwrap();
        let record = &file["channels"][w.key.to_string()];
        assert_eq!(record["given"], serde_json::json!([w.a.url]));
    }

    /// S105-CHURN (a): the `gone` filter is written to `follows.json` and read back on open,
    /// so a restart does not forget an evicted channel.
    #[test]
    fn adv_s105_churn_the_gone_filter_survives_a_reopen() {
        let w = world();
        let (dir, eve) = w.eve(&[]);
        eve.test_add_tombstone(&w.key.to_string(), tomb(&[], 1));
        for n in 0..1024 {
            eve.test_add_tombstone(&format!("k{n}"), tomb(&[], n + 2));
        }
        eve.save_follows(&mut std::collections::BTreeMap::new(), Default::default())
            .unwrap();
        drop(eve);
        let path = dir.path().join("dist/follows.json");
        let mut file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(file["gone"].as_str().map(str::len), Some(8192));
        // The throwaway tombstone keys are no channel keys: keep only the filter.
        file.as_object_mut().unwrap().remove("seen");
        std::fs::write(&path, file.to_string()).unwrap();
        let eve = strict(dir.path());
        assert!(
            eve.held_ungiven(w.key, &w.a.url),
            "the reopened filter forgot"
        );
    }

    /// S105-CHURN (a): the filter rules only a channel with no tombstone; a gone channel
    /// that was pruned again starts from its new tombstone, which gives a new base.
    #[test]
    fn adv_s105_churn_a_new_tombstone_wins_over_the_filter() {
        let w = world();
        let (dir, eve) = w.eve(&[]);
        let key = w.key.to_string();
        eve.test_add_tombstone(&key, tomb(&[], 1));
        for n in 0..1024 {
            eve.test_add_tombstone(&format!("k{n}"), tomb(&[], n + 2));
        }
        eve.test_add_tombstone(&key, tomb(&[], u64::MAX));
        assert!(eve.test_tomb_keys().contains(&key));
        eve.follow(&w.link(&w.a)).unwrap();
        let file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("dist/follows.json")).unwrap())
                .unwrap();
        assert_eq!(
            file["channels"][&key]["given"],
            serde_json::json!([w.a.url])
        );
    }

    /// r7: a provider hint's base (another channel's listing, 5.11) becomes
    /// `Fetched.link` of a channel with no record; dist keeps it in no `seen` and no
    /// tombstone, so a follow of that derived link gives the hint's base, and its
    /// redirect into the local network is followed (A35, A37, DD-48).
    #[test]
    fn adv_s105_r7_provider_fetched_link_gives_a_hint_base() {
        let w = world();
        // Dave lists the publisher at `a`: for Eve that base is only a hint.
        let (_dave_dir, dave) = home("Dave", true);
        dave.list(&w.link(&w.a), "hint").unwrap();
        let dave_host = Host::serve(&dave.site_dir());
        let dave_key = dave.identity().unwrap();
        let (_dir, eve) = w.eve(&[&w.a, &dave_host]);
        eve.follow(&crate::Link::new(dave_key, &dave_host.url, None).unwrap())
            .unwrap();
        let got = eve.fetch_for(&w.reference, Some(&dave_key)).unwrap();
        let derived = got.link.expect("a provider fetch names its base");
        assert_eq!(
            derived,
            w.link(&w.a),
            "the link is built from the hint base"
        );
        // The app installs from the fetched link; the hint host now redirects home.
        w.a.redirect("/head", &format!("{}head", w.b.url));
        let _ = eve.follow(&derived);
        let _ = eve.refresh_channel(&w.key);
        assert_eq!(
            w.b.count(),
            0,
            "a hint base led dist into the local network"
        );
    }

    /// r7: the same for a listed-only record: a hint base the record never held becomes
    /// `Fetched.link`, and a follow of it gives that base (not in `seen`).
    #[test]
    fn adv_s105_r7_listed_record_provider_link_gives_a_hint_base() {
        let w = world();
        let a2 = Host::serve(&w.publisher.site_dir());
        let (_dave_dir, dave) = home("Dave", true);
        dave.list(&w.link(&a2), "hint").unwrap();
        let dave_host = Host::serve(&dave.site_dir());
        let dave_key = dave.identity().unwrap();
        let (_dir, eve) = w.eve(&[&w.a, &a2, &dave_host]);
        let _z = listed_record(&w, &eve);
        eve.follow(&crate::Link::new(dave_key, &dave_host.url, None).unwrap())
            .unwrap();
        // The listed base fails its package (a refused redirect); the hint base serves it.
        let target = format!("{}{}", w.b.url, w.package);
        w.a.redirect(&format!("/{}", w.package), &target);
        let got = eve.fetch_for(&w.reference, Some(&dave_key)).unwrap();
        let derived = got.link.expect("a provider fetch names its base");
        assert_eq!(derived, w.link(&a2), "the link is built from the hint base");
        assert_eq!(w.b.count(), 0);
        a2.redirect("/head", &format!("{}head", w.b.url));
        let _ = eve.follow(&derived);
        let _ = eve.refresh_channel(&w.key);
        assert_eq!(
            w.b.count(),
            0,
            "a hint base led dist into the local network"
        );
    }
    fn follows_file(dir: &std::path::Path) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(dir.join("dist/follows.json")).unwrap()).unwrap()
    }

    /// r7 AC2: a provider fetch through a hint base for a channel with no record leaves a
    /// tombstone for it that saw the base, and no record (A36).
    #[test]
    fn ac2_provider_fetch_notes_the_hint_base_in_a_tombstone() {
        let w = world();
        let (_dave_dir, dave) = home("Dave", true);
        dave.list(&w.link(&w.a), "hint").unwrap();
        let dave_host = Host::serve(&dave.site_dir());
        let dave_key = dave.identity().unwrap();
        let (dir, eve) = w.eve(&[&w.a, &dave_host]);
        eve.follow(&crate::Link::new(dave_key, &dave_host.url, None).unwrap())
            .unwrap();
        eve.fetch_for(&w.reference, Some(&dave_key)).unwrap();
        let file = follows_file(dir.path());
        let key = w.key.to_string();
        assert!(
            file["channels"].get(&key).is_none(),
            "a tombstone is no record"
        );
        assert_eq!(file["seen"][&key]["seen"], serde_json::json!([w.a.url]));
        assert!(file["seen"][&key]["given"].as_array().unwrap().is_empty());
    }

    /// r7 AC2: with a listed record, the record's `seen` holds the hint base.
    #[test]
    fn ac2_provider_fetch_notes_the_hint_base_in_a_listed_record() {
        let w = world();
        let a2 = Host::serve(&w.publisher.site_dir());
        let (_dave_dir, dave) = home("Dave", true);
        dave.list(&w.link(&a2), "hint").unwrap();
        let dave_host = Host::serve(&dave.site_dir());
        let dave_key = dave.identity().unwrap();
        let (dir, eve) = w.eve(&[&w.a, &a2, &dave_host]);
        let _z = listed_record(&w, &eve);
        eve.follow(&crate::Link::new(dave_key, &dave_host.url, None).unwrap())
            .unwrap();
        w.a.redirect(
            &format!("/{}", w.package),
            &format!("{}{}", w.b.url, w.package),
        );
        eve.fetch_for(&w.reference, Some(&dave_key)).unwrap();
        let file = follows_file(dir.path());
        let seen = file["channels"][w.key.to_string()]["seen"].clone();
        assert!(
            seen.as_array().unwrap().contains(&a2.url.clone().into()),
            "{seen}"
        );
    }

    /// r7 AC4: a failed save while noting the base makes no request to it, and the
    /// tombstones stay as they were.
    #[test]
    fn ac4_a_failed_save_noting_a_hint_base_makes_no_request_to_it() {
        let w = world();
        let (_dave_dir, dave) = home("Dave", true);
        dave.list(&w.link(&w.a), "hint").unwrap();
        let dave_host = Host::serve(&dave.site_dir());
        let dave_key = dave.identity().unwrap();
        let (dir, eve) = w.eve(&[&w.a, &dave_host]);
        eve.follow(&crate::Link::new(dave_key, &dave_host.url, None).unwrap())
            .unwrap();
        let before = eve.test_tomb_keys();
        let file = std::fs::read(dir.path().join("dist/follows.json")).unwrap();
        let requests = w.a.count();
        *eve.test_fail_saves.lock().unwrap() = Some(0);
        let err = eve.fetch_for(&w.reference, Some(&dave_key)).unwrap_err();
        assert!(
            matches!(&err, DistError::Io { source, .. } if source.to_string() == "test: save fails"),
            "the read fails with the save's error: {err:?}"
        );
        assert_eq!(
            w.a.count(),
            requests,
            "the base was read before it was noted"
        );
        assert_eq!(eve.test_tomb_keys(), before);
        let after = std::fs::read(dir.path().join("dist/follows.json")).unwrap();
        assert_eq!(after, file);
    }

    /// r7: a new tombstone for a channel in the `gone` filter starts `seen_full`, so noting
    /// a provider base does not undo the fail-closed state (4.4, DD-48).
    #[test]
    fn provider_base_tombstone_of_a_gone_channel_starts_full() {
        let w = world();
        let (_dave_dir, dave) = home("Dave", true);
        dave.list(&w.link(&w.a), "hint").unwrap();
        let dave_host = Host::serve(&dave.site_dir());
        let dave_key = dave.identity().unwrap();
        let (dir, eve) = w.eve(&[&w.a, &dave_host]);
        eve.follow(&crate::Link::new(dave_key, &dave_host.url, None).unwrap())
            .unwrap();
        eve.test_add_tombstone(&w.key.to_string(), tomb(&[], 1));
        for n in 0..1024 {
            eve.test_add_tombstone(&format!("k{n}"), tomb(&[], n + 2));
        }
        assert!(eve.held_ungiven(w.key, &w.b.url), "the channel is gone");
        eve.fetch_for(&w.reference, Some(&dave_key)).unwrap();
        let file = follows_file(dir.path());
        assert_eq!(file["seen"][w.key.to_string()]["seen_full"], true);
        assert!(
            eve.held_ungiven(w.key, &w.b.url),
            "the new tombstone forgot"
        );
    }

    /// r7: a failed save while noting one provider base skips only that base; the read
    /// ends with the other bases' result and the unnoted base gets no request.
    #[test]
    fn provider_base_a_failed_note_moves_to_the_next_base() {
        let w = world();
        let a2 = Host::serve(&w.publisher.site_dir());
        let (_dir, eve) = w.eve(&[&w.a, &a2]);
        w.a.redirect(
            &format!("/{}", w.package),
            &format!("{}{}", w.b.url, w.package),
        );
        eve.list(&w.link(&w.a), "first").unwrap();
        let (_dave_dir, dave) = home("Dave", true);
        dave.list(&w.link(&a2), "second").unwrap();
        let dave_host = Host::serve(&dave.site_dir());
        let dave_key = dave.identity().unwrap();
        eve.follow(&crate::Link::new(dave_key, &dave_host.url, None).unwrap())
            .unwrap();
        let (a_before, a2_before) = (w.a.count(), a2.count());
        // The first note (base `a`) saves; the second (base `a2`) fails.
        *eve.test_fail_saves.lock().unwrap() = Some(1);
        let err = eve.fetch_for(&w.reference, Some(&dave_key)).unwrap_err();
        assert!(matches!(err, DistError::BadUrl { .. }), "{err:?}");
        assert!(w.a.count() > a_before, "the noted base is read");
        assert_eq!(a2.count(), a2_before, "a base not noted is not read");
    }

    /// r8: `read_link` of a link for a channel with no record reads its base as listed
    /// (5.11, A37) and returns it as `Fetched.link`, but notes it in no `seen` and no
    /// tombstone, unlike a provider base (fix7). After a managed install (app
    /// `read_managed_release`, a manager mod's link) the caller follows `Fetched.link`
    /// (DD-29); that follow finds the base unseen and gives it, so the host's later
    /// redirect into the local network is followed (A35, A37, DD-48).
    #[test]
    fn adv_s105_r8_read_link_fetched_link_gives_its_listed_base() {
        let w = world();
        let (_dir, eve) = w.eve(&[&w.a]);
        let got = eve.read_link(&w.link(&w.a)).unwrap();
        let derived = got.link.expect("read_link names its base");
        w.a.redirect("/head", &format!("{}head", w.b.url));
        let _ = eve.follow(&derived);
        let _ = eve.refresh_channel(&w.key);
        assert_eq!(
            w.b.count(),
            0,
            "a base read as listed by read_link led dist into the local network"
        );
    }

    /// r8 AC2: `read_link` of a link for a channel with no record leaves no record and a
    /// tombstone whose `seen` holds the link's base (DD-48).
    #[test]
    fn ac2_read_link_notes_its_base_in_a_tombstone() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a]);
        eve.read_link(&w.link(&w.a)).unwrap();
        let file = follows_file(dir.path());
        assert!(file["channels"].get(w.key.to_string()).is_none(), "{file}");
        let seen = file["seen"][w.key.to_string()]["seen"].clone();
        assert!(
            seen.as_array().unwrap().contains(&w.a.url.clone().into()),
            "{seen}"
        );
    }

    /// r8 AC3: a failed save while `read_link` notes its base makes no request.
    #[test]
    fn ac3_read_link_with_a_failed_note_makes_no_request() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a]);
        let requests = w.a.count();
        let before = eve.test_tomb_keys();
        *eve.test_fail_saves.lock().unwrap() = Some(0);
        let err = eve.read_link(&w.link(&w.a)).unwrap_err();
        assert!(
            matches!(&err, DistError::Io { source, .. } if source.to_string() == "test: save fails"),
            "{err:?}"
        );
        assert_eq!(
            w.a.count(),
            requests,
            "the base was read before it was noted"
        );
        assert_eq!(eve.test_tomb_keys(), before);
        assert!(!dir.path().join("dist/follows.json").exists());
    }

    /// r8: a followed channel's `read_link` notes a new link base in the record before any
    /// request; the own channel notes nothing (DD-48).
    #[test]
    fn read_link_notes_a_record_base_first_and_never_the_own_key() {
        let w = world();
        let (dir, eve) = w.eve(&[&w.a, &w.b]);
        eve.follow(&w.link(&w.a)).unwrap();
        let requests = (w.a.count(), w.b.count());
        *eve.test_fail_saves.lock().unwrap() = Some(0);
        assert!(matches!(
            eve.read_link(&w.link(&w.b)),
            Err(DistError::Io { .. })
        ));
        assert_eq!((w.a.count(), w.b.count()), requests, "read before the note");
        *eve.test_fail_saves.lock().unwrap() = None;
        eve.read_link(&w.link(&w.b)).unwrap();
        let seen = follows_file(dir.path())["channels"][w.key.to_string()]["seen"].clone();
        assert!(
            seen.as_array().unwrap().contains(&w.b.url.clone().into()),
            "{seen}"
        );
        *w.publisher.test_fail_saves.lock().unwrap() = Some(0);
        w.publisher.read_link(&w.link(&w.a)).unwrap();
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

    /// F4.1 (Codex r4): after a rename whose directory sync failed, a retry that reads
    /// provenance first retries the sync; while it fails, the call makes no request.
    #[test]
    fn f4_1_a_retry_reads_no_provenance_while_the_dir_sync_fails() {
        let w = world();
        let (_dir, eve) = w.eve(&[&w.a]);
        let before = w.a.count();
        sync_fails(u32::MAX);
        for _ in 0..2 {
            assert_eio(eve.read_link(&w.link(&w.a)).map(|_| ()));
        }
        assert_eq!(w.a.count(), before, "a request while the store is unsynced");
        sync_fails(0);
        eve.read_link(&w.link(&w.a)).unwrap();
        assert!(w.a.count() > before);
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
        assert!(file["seen"].get(&key).is_some(), "{file}");
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
            assert!(file["seen"].get(&key).is_some(), "{key}: {file}");
        }
    }
}
