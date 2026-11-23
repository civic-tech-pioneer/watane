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
            301 | 302 | 303 | 307 | 308 if by_hand => {
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
}
