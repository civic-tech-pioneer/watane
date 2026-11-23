//! Reads from a web host (TECHNICAL 4.5): GET with a body limit and a
//! whole-request timeout on every request. Outgoing only; nothing listens.

use crate::{Dist, DistError, Entry, PublicKey};
use std::time::{Duration, Instant};

/// The most redirects one read follows (TECHNICAL 4.5).
pub(crate) const MAX_REDIRECTS: usize = 5;

/// The local-address rule applied to one redirect target.
pub(crate) type Check = dyn Fn(&str) -> Result<(), DistError>;

/// What one hop of a listed read answers: the bytes, or the next URL.
pub(crate) enum Hop<T> {
    Done(T),
    Redirect(String),
}

/// Follow up to `MAX_REDIRECTS` redirects by hand: each `Location` is resolved against
/// the current URL and passes `check` before any request to it (TECHNICAL 4.5).
pub(crate) fn follow_hops<T>(
    first: &str,
    check: &Check,
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
        check: Option<&Check>,
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
        check: Option<&Check>,
    ) -> Result<Vec<u8>, DistError> {
        let Some(check) = check else {
            return match self.get_hop(url, limit, timeout, head, &self.agent)? {
                Hop::Done(bytes) => Ok(bytes),
                Hop::Redirect(_) => unreachable!("the agent follows redirects"),
            };
        };
        // One deadline for all hops.
        let deadline = Instant::now() + timeout;
        follow_hops(url, check, |current| {
            let left = deadline.saturating_duration_since(Instant::now());
            self.get_hop(current, limit, left, head, &self.listed_agent)
        })
    }
    fn get_hop(
        &self,
        url: &str,
        limit: u64,
        timeout: Duration,
        head: bool,
        agent: &ureq::Agent,
    ) -> Result<Hop<Vec<u8>>, DistError> {
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
            301 | 302 | 303 | 307 | 308 if !std::ptr::eq(agent, &self.agent) => {
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
}
