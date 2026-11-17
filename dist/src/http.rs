use crate::{Dist, DistError, Entry, PublicKey};
use std::time::{Duration, Instant};

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
    pub(crate) fn get(
        &self,
        url: &str,
        limit: u64,
        timeout: Duration,
        head: bool,
    ) -> Result<Vec<u8>, DistError> {
        let deadline = Instant::now() + timeout;
        let mut waits = RETRY_WAITS.iter();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let error = match self.get_once(url, limit, left, head) {
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
    ) -> Result<Vec<u8>, DistError> {
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
        let mut request = self
            .agent
            .get(url)
            .config()
            .timeout_global(Some(timeout))
            .build();
        if head {
            request = request.header("Cache-Control", "no-cache");
        }
        let mut response = request.call().map_err(map)?;
        match response.status().as_u16() {
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
                Ok(bytes)
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
    ) -> Result<Entry, DistError> {
        let bytes = self.get(
            url,
            self.options.max_entry_file,
            self.options.entry_timeout,
            seq.is_none(),
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
