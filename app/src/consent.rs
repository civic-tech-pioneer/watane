use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AskId(String); // 32 lowercase hex characters from 16 random bytes
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "answer", rename_all = "snake_case")]
pub enum Answer {
    Approve { withheld: Vec<Withheld> },
    Deny,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Withheld {
    pub local: String,
    pub key: String,
} // PlanMod.local; GrantLine.key
#[derive(Debug, thiserror::Error)]
pub enum ConsentError {
    #[error("this request is no longer open")]
    Stale,
    #[error("a withheld grant is not in the request: {0}")]
    UnknownGrant(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Install {
        mods: Vec<PlanMod>,
        by_program: bool,
    }, // install order; Requester::LocalApi
    Sign {
        statement: Statement,
        signer: Party,
        details: Vec<Detail>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanMod {
    pub local: String,
    pub name: String,
    pub slug: String,
    pub version: String,
    pub release: Option<String>,  // ReleaseRef text; None = local
    pub publisher: Option<Party>, // None = local; Some = signature verified by host
    pub replaces: Option<String>, // the installed version that this replaces
    pub grants: Vec<GrantLine>,
    pub uses: Vec<String>, // providers, as {provider} of section 4
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Party {
    pub name: Option<String>,
    pub fingerprint: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrantLine {
    pub key: String,
    pub kind: GrantKind,
    pub text: String,
    pub granted: bool,
    pub new: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantKind {
    Storage,
    Http,
    Call,
    Subscribe,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Statement {
    Profile,
    Release,
    Withdraw,
    List,
    Include,
    Unlist,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Detail {
    pub label: String,
    pub value: String,
}

impl AskId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl FromStr for AskId {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.into()))
    }
}
/// The single FIFO for app confirmations. Callbacks run without the state lock.
pub struct Broker {
    state: std::sync::Mutex<State>,
}

type Listener = Arc<dyn Fn(Option<AskId>) + Send + Sync>;

#[derive(Default)]
struct State {
    queue: std::collections::VecDeque<Entry>,
    listener: Option<Listener>,
    notifications: std::collections::VecDeque<(Listener, Option<AskId>)>,
    notifying: bool,
}

struct Entry {
    id: AskId,
    request: Request,
    reply: tokio::sync::oneshot::Sender<Answer>,
}

impl State {
    fn head_changed(&mut self) {
        if let Some(listener) = &self.listener {
            self.notifications
                .push_back((listener.clone(), self.queue.front().map(|e| e.id.clone())));
        }
    }
}

/// Removes a request even when its waiting future is cancelled.
struct Pending<'a> {
    broker: &'a Broker,
    id: AskId,
}

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.broker.remove(&self.id);
    }
}

impl Broker {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Default::default(),
        })
    }

    /// Queue on first poll. The deadline starts here, even behind another request.
    pub async fn ask(&self, request: Request) -> Answer {
        let mut random = [0; 16];
        getrandom::fill(&mut random).expect("cannot draw a consent request id");
        let id = AskId(random.iter().map(|byte| format!("{byte:02x}")).collect());
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(600);
        let (reply, receiver) = tokio::sync::oneshot::channel();
        let pending = Pending {
            broker: self,
            id: id.clone(),
        };
        {
            let mut state = self.state.lock().unwrap();
            let was_empty = state.queue.is_empty();
            state.queue.push_back(Entry { id, request, reply });
            if was_empty {
                state.head_changed();
            }
        }
        self.notify();
        let answer = tokio::select! {
            biased;
            answer = receiver => answer.unwrap_or(Answer::Deny),
            _ = tokio::time::sleep_until(deadline) => Answer::Deny,
        };
        drop(pending);
        answer
    }

    /// For plain blocking threads only, never a Tokio worker or the UI thread.
    pub fn ask_blocking(&self, request: Request) -> Answer {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("cannot start consent runtime")
            .block_on(self.ask(request))
    }

    pub fn current(&self) -> Option<(AskId, Request)> {
        self.state
            .lock()
            .unwrap()
            .queue
            .front()
            .map(|entry| (entry.id.clone(), entry.request.clone()))
    }

    pub fn answer(&self, id: &AskId, answer: Answer) -> Result<(), ConsentError> {
        {
            let mut state = self.state.lock().unwrap();
            let entry = state
                .queue
                .front()
                .filter(|e| &e.id == id)
                .ok_or(ConsentError::Stale)?;
            if let Answer::Approve { withheld } = &answer {
                for grant in withheld {
                    let known = match &entry.request {
                        Request::Install { mods, .. } => mods.iter().any(|m| {
                            m.local == grant.local && m.grants.iter().any(|g| g.key == grant.key)
                        }),
                        Request::Sign { .. } => false,
                    };
                    if !known {
                        return Err(ConsentError::UnknownGrant(format!(
                            "{}/{}",
                            grant.local, grant.key
                        )));
                    }
                }
            }
            let entry = state.queue.pop_front().unwrap();
            let _ = entry.reply.send(answer);
            state.head_changed();
        }
        self.notify();
        Ok(())
    }

    pub fn deny_all(&self) {
        {
            let mut state = self.state.lock().unwrap();
            if state.queue.is_empty() {
                return;
            }
            for entry in state.queue.drain(..) {
                let _ = entry.reply.send(Answer::Deny);
            }
            state.head_changed();
        }
        self.notify();
    }

    /// Replace the listener. Subsequent head changes are delivered in order.
    pub fn set_listener(&self, f: Box<dyn Fn(Option<AskId>) + Send + Sync>) {
        self.state.lock().unwrap().listener = Some(Arc::from(f));
    }

    fn remove(&self, id: &AskId) {
        {
            let mut state = self.state.lock().unwrap();
            let Some(index) = state.queue.iter().position(|entry| &entry.id == id) else {
                return;
            };
            state.queue.remove(index);
            if index == 0 {
                state.head_changed();
            }
        }
        self.notify();
    }

    // Serialize callback delivery across threads, while allowing callbacks to
    // read or answer the broker without taking its mutex recursively.
    fn notify(&self) {
        {
            let mut state = self.state.lock().unwrap();
            if state.notifying {
                return;
            }
            state.notifying = true;
        }
        loop {
            let notification = {
                let mut state = self.state.lock().unwrap();
                match state.notifications.pop_front() {
                    Some(notification) => notification,
                    None => {
                        state.notifying = false;
                        return;
                    }
                }
            };
            (notification.0)(notification.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{future::Future, pin::pin, task::Poll, time::Duration};

    fn install() -> Request {
        serde_json::from_value(json!({"kind":"install","by_program":false,"mods":[{
            "local":"reading-list","name":"Reading list","slug":"reading-list",
            "version":"0.1.0","release":null,"publisher":null,"replaces":null,
            "grants":[{"key":"storage","kind":"storage","text":"Keep its own data on this device",
            "granted":true,"new":true}],"uses":[]}]}))
        .unwrap()
    }

    async fn queue(f: std::pin::Pin<&mut impl Future<Output = Answer>>) {
        let mut f = f;
        std::future::poll_fn(|cx| {
            assert!(f.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
    }

    #[tokio::test]
    async fn ac1_workspace_and_modules() {
        assert!(include_str!("../../Cargo.toml").contains("\"app\""));
        let b = Broker::new();
        assert!(b.current().is_none());
    }

    #[tokio::test]
    async fn ac2_fifo_and_head_notifications() {
        let b = Broker::new();
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = events.clone();
        // Reading current from a listener must not deadlock.
        let weak = Arc::downgrade(&b);
        b.set_listener(Box::new(move |id| {
            assert_eq!(weak.upgrade().unwrap().current().map(|x| x.0), id);
            log.lock().unwrap().push(id);
        }));
        let requests: Vec<_> = ["A", "B", "C"]
            .into_iter()
            .map(|name| {
                let mut request = install();
                if let Request::Install { mods, .. } = &mut request {
                    mods[0].name = name.into();
                }
                request
            })
            .collect();
        let mut a = pin!(b.ask(requests[0].clone()));
        queue(a.as_mut()).await;
        assert_eq!(b.current().unwrap().1, requests[0]);
        let aid = b.current().unwrap().0;
        let mut c = pin!(b.ask(requests[1].clone()));
        queue(c.as_mut()).await;
        let mut d = pin!(b.ask(requests[2].clone()));
        queue(d.as_mut()).await;
        b.answer(&aid, Answer::Deny).unwrap();
        assert_eq!(b.current().unwrap().1, requests[1]);
        let bid = b.current().unwrap().0;
        b.answer(&bid, Answer::Deny).unwrap();
        assert_eq!(b.current().unwrap().1, requests[2]);
        let cid = b.current().unwrap().0;
        b.answer(&cid, Answer::Deny).unwrap();
        assert_eq!(
            *events.lock().unwrap(),
            vec![Some(aid), Some(bid), Some(cid), None]
        );
        assert_eq!(a.await, Answer::Deny);
        assert_eq!(c.await, Answer::Deny);
        assert_eq!(d.await, Answer::Deny);
    }

    #[tokio::test]
    async fn ac3_non_head_unknown_and_repeated_ids_are_stale() {
        let b = Broker::new();
        let mut a = pin!(b.ask(install()));
        queue(a.as_mut()).await;
        let aid = b.current().unwrap().0;
        let mut c = pin!(b.ask(install()));
        queue(c.as_mut()).await;
        let bid = b.state.lock().unwrap().queue[1].id.clone();
        for id in [bid, "anything".parse().unwrap()] {
            assert!(matches!(
                b.answer(&id, Answer::Deny),
                Err(ConsentError::Stale)
            ));
        }
        b.answer(&aid, Answer::Deny).unwrap();
        assert!(matches!(
            b.answer(&aid, Answer::Deny),
            Err(ConsentError::Stale)
        ));
    }

    #[tokio::test]
    async fn ac4_unknown_grants_leave_request_open() {
        let b = Broker::new();
        for request in [
            install(),
            Request::Sign {
                statement: Statement::Profile,
                signer: Party {
                    name: None,
                    fingerprint: "fingerprint".into(),
                },
                details: vec![],
            },
        ] {
            let mut f = pin!(b.ask(request));
            queue(f.as_mut()).await;
            let id = b.current().unwrap().0;
            for (local, key) in [
                ("reading-list", "unknown"),
                ("other", "storage"),
                ("reading-list", "storage"),
            ] {
                let answer = Answer::Approve {
                    withheld: vec![Withheld {
                        local: local.into(),
                        key: key.into(),
                    }],
                };
                let valid = matches!(b.current().unwrap().1, Request::Install { .. })
                    && local == "reading-list"
                    && key == "storage";
                let result = b.answer(&id, answer.clone());
                if valid {
                    result.unwrap();
                    assert_eq!(f.as_mut().await, answer);
                } else {
                    assert!(matches!(result, Err(ConsentError::UnknownGrant(_))));
                    assert_eq!(b.current().unwrap().0, id);
                }
            }
            b.deny_all();
        }
    }

    #[tokio::test]
    async fn ac5_dropping_ask_removes_it_and_notifies() {
        let b = Broker::new();
        let (tx, rx) = std::sync::mpsc::channel();
        b.set_listener(Box::new(move |id| tx.send(id).unwrap()));
        let mut a = Box::pin(b.ask(install()));
        queue(a.as_mut()).await;
        let aid = rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        let mut c = pin!(b.ask(install()));
        queue(c.as_mut()).await;
        drop(a);
        let bid = rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        assert_ne!(aid, bid);
        assert_eq!(b.current().unwrap().0, bid);
        b.deny_all();
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), None);
        assert_eq!(c.await, Answer::Deny);
    }

    #[tokio::test]
    async fn ac6_deny_all_resolves_every_request() {
        let b = Broker::new();
        let mut a = pin!(b.ask(install()));
        let mut c = pin!(b.ask(install()));
        queue(a.as_mut()).await;
        queue(c.as_mut()).await;
        b.deny_all();
        assert!(b.current().is_none());
        assert_eq!(a.await, Answer::Deny);
        assert_eq!(c.await, Answer::Deny);
    }

    #[test]
    fn ac6_blocking_caller_receives_another_threads_answer() {
        let b = Broker::new();
        let (tx, rx) = std::sync::mpsc::channel();
        b.set_listener(Box::new(move |id| {
            let _ = tx.send(id);
        }));
        let worker = b.clone();
        let thread = std::thread::spawn(move || worker.ask_blocking(install()));
        let id = rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        let answer = Answer::Approve { withheld: vec![] };
        b.answer(&id, answer.clone()).unwrap();
        assert_eq!(thread.join().unwrap(), answer);
    }

    #[tokio::test(start_paused = true)]
    async fn ac7_timeout_is_600_seconds_from_enqueue_even_behind_head() {
        let b = Broker::new();
        let mut a = pin!(b.ask(install()));
        queue(a.as_mut()).await;
        let aid = b.current().unwrap().0;
        tokio::time::advance(Duration::from_secs(100)).await;
        let mut c = pin!(b.ask(install()));
        queue(c.as_mut()).await;
        tokio::time::advance(Duration::from_secs(499)).await;
        queue(a.as_mut()).await;
        assert_eq!(b.current().unwrap().0, aid);
        tokio::time::advance(Duration::from_secs(1)).await;
        assert_eq!(a.await, Answer::Deny);
        assert_ne!(b.current().unwrap().0, aid);
        tokio::time::advance(Duration::from_secs(99)).await;
        queue(c.as_mut()).await;
        tokio::time::advance(Duration::from_secs(1)).await;
        assert_eq!(c.await, Answer::Deny);
        assert!(b.current().is_none());
    }

    #[tokio::test]
    async fn ac8_random_ids_are_lowercase_hex() {
        let b = Broker::new();
        let mut ids = Vec::new();
        for _ in 0..2 {
            let mut f = pin!(b.ask(install()));
            queue(f.as_mut()).await;
            let id = b.current().unwrap().0;
            assert_eq!(id.as_str().len(), 32);
            assert!(
                id.as_str()
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            );
            assert_eq!(id.as_str().parse::<AskId>().unwrap(), id);
            ids.push(id);
            b.deny_all();
        }
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn ac9_request_and_answer_json_match_spec() {
        let value = json!({"kind":"install","by_program":false,"mods":[{
            "local":"reading-list","name":"Reading list","slug":"reading-list","version":"0.1.0",
            "release":null,"publisher":null,"replaces":null,"grants":[{"key":"storage","kind":"storage",
            "text":"Keep its own data on this device","granted":true,"new":true}],"uses":[]}]});
        assert_eq!(serde_json::to_value(install()).unwrap(), value);
        let sign = json!({"kind":"sign","statement":"release","signer":{"name":"Alice","fingerprint":"abc"},
            "details":[{"label":"Package","value":"123"}]});
        assert_eq!(
            serde_json::to_value(serde_json::from_value::<Request>(sign.clone()).unwrap()).unwrap(),
            sign
        );
        for answer in [
            Answer::Deny,
            Answer::Approve {
                withheld: vec![Withheld {
                    local: "reading-list".into(),
                    key: "storage".into(),
                }],
            },
        ] {
            let value = serde_json::to_value(&answer).unwrap();
            assert_eq!(serde_json::from_value::<Answer>(value).unwrap(), answer);
        }
        assert_eq!(
            serde_json::to_value(Answer::Deny).unwrap(),
            json!({"answer":"deny"})
        );
        assert_eq!(
            serde_json::to_value(Answer::Approve { withheld: vec![] }).unwrap(),
            json!({"answer":"approve","withheld":[]})
        );
    }
}

#[cfg(test)]
mod stage_c_tests {
    use super::*;
    use watane_dist::{Confirm, Decision, SignRequest};
    #[test]
    fn ac1_dist_confirm_maps_each_statement_and_broker_answer() {
        let mut key = [0x66; 32];
        key[0] = 0x58;
        let channel = watane_dist::PublicKey::from_bytes(&key).unwrap();
        let release = watane_dist::Release {
            slug: "reading-list".parse().unwrap(),
            version: "0.1.0".parse().unwrap(),
            name: "Reading list".into(),
            summary: "Books".into(),
            package: "ab".repeat(32).parse().unwrap(),
            size: 123,
            source: true,
            offers: vec![],
            events: vec![],
            grants: vec![],
            origin: None,
        };
        let reference: watane_dist::ReleaseRef =
            format!("{channel}/reading-list@0.1.0").parse().unwrap();
        let cases = [
            (
                watane_dist::Statement::Profile(watane_dist::Profile {
                    name: "Alice".into(),
                }),
                Statement::Profile,
                vec![("Name", "Alice".to_string())],
            ),
            (
                watane_dist::Statement::Release {
                    reference: reference.clone(),
                    release: release.clone(),
                },
                Statement::Release,
                vec![
                    ("Release", reference.to_string()),
                    ("Package", release.package.to_string()),
                    ("Size", "123".into()),
                    ("Source included", "yes".into()),
                    ("Summary", "Books".into()),
                ],
            ),
            (
                watane_dist::Statement::Release {
                    reference: reference.clone(),
                    release: watane_dist::Release {
                        source: false,
                        ..release.clone()
                    },
                },
                Statement::Release,
                vec![
                    ("Release", reference.to_string()),
                    ("Package", release.package.to_string()),
                    ("Size", "123".into()),
                    ("Source included", "no".into()),
                    ("Summary", "Books".into()),
                ],
            ),
            (
                watane_dist::Statement::Withdraw {
                    reference: reference.clone(),
                    release,
                    withdraw: watane_dist::Withdraw {
                        release: "00".repeat(32).parse().unwrap(),
                        reason: "Retired".into(),
                    },
                },
                Statement::Withdraw,
                vec![
                    ("Release", reference.to_string()),
                    ("Reason", "Retired".into()),
                ],
            ),
        ];
        for (statement, expected, details) in cases {
            for approve in [true, false] {
                let broker = Broker::new();
                let (tx, rx) = std::sync::mpsc::channel();
                broker.set_listener(Box::new(move |id| {
                    let _ = tx.send(id);
                }));
                let parties = std::sync::Arc::new(crate::words::Parties::new());
                parties.insert(channel.to_string(), "Alice".into());
                let adapter = crate::confirm::DistConfirm {
                    broker: broker.clone(),
                    parties,
                };
                let request = SignRequest {
                    channel,
                    fingerprint: "request fingerprint".into(),
                    statement: statement.clone(),
                };
                let worker = std::thread::spawn(move || adapter.confirm(&request));
                let id = rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap()
                    .unwrap();
                let Request::Sign {
                    statement,
                    signer,
                    details: actual,
                } = broker.current().unwrap().1
                else {
                    panic!("sign expected")
                };
                assert_eq!(statement, expected);
                assert_eq!(signer.fingerprint, "request fingerprint");
                assert_eq!(signer.name.as_deref(), Some("Alice"));
                assert_eq!(
                    actual,
                    details
                        .iter()
                        .map(|(k, v)| Detail {
                            label: (*k).into(),
                            value: v.clone()
                        })
                        .collect::<Vec<_>>()
                );
                broker
                    .answer(
                        &id,
                        if approve {
                            Answer::Approve { withheld: vec![] }
                        } else {
                            Answer::Deny
                        },
                    )
                    .unwrap();
                assert_eq!(
                    worker.join().unwrap(),
                    if approve {
                        Decision::Approve
                    } else {
                        Decision::Refuse
                    }
                );
            }
        }
    }
}
