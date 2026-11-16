use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AskId(String); // 32 lowercase hex characters from 16 random bytes
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "answer", rename_all = "snake_case")]
pub enum Answer {
    Approve { withheld: Vec<Withheld> },
    Pick { choice: String },
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
    #[error("this answer does not fit the request")]
    WrongAnswer,
    #[error("the choice is not in the request: {0}")]
    UnknownChoice(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Join {
        node: String,
        role: Role,
        text: String,
    },
    Reach {
        what: Reach,
        url: Option<String>,
        text: String,
    },
    Pick {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        asked_by: Option<Asker>,
        consumer: String,
        provider: String,
        cause: String,
        candidates: Vec<Candidate>,
    },
    Install {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        asked_by: Option<Asker>,
        mods: Vec<PlanMod>,
        by_program: bool,
    }, // install order; Requester::LocalApi
    Manage {
        asked_by: Asker,
        action: ManageAction,
        target: Target,
        grant: Option<GrantLine>,
        users: Vec<String>,
    },
    Sign {
        statement: Statement,
        signer: Party,
        details: Vec<Detail>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asker {
    pub local: String,
    pub name: String,
    pub publisher: Option<Party>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Target {
    pub local: String,
    pub name: String,
    pub version: String,
    pub publisher: Option<Party>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManageAction {
    Enable,
    Grant,
    Remove,
    RemoveData,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Full,
    Light,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    Relay,
    Dht,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub choice: String,
    pub name: String,
    pub summary: String,
    pub version: String,
    pub publisher: Party,
    pub origin: Option<String>,
    pub installed: bool,
    pub checked_at: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanMod {
    #[serde(default)]
    pub becomes_main: bool,
    pub local: String,
    pub name: String,
    pub slug: String,
    pub version: String,
    pub release: Option<String>,  // ReleaseRef text; None = local
    pub publisher: Option<Party>, // None = local; Some = signature verified by host
    pub replaces: Option<String>, // the installed version that this replaces
    pub grants: Vec<GrantLine>,
    pub uses: Vec<String>, // providers, as {provider} of section 4
    pub origin: Option<String>,
    #[serde(default)]
    pub binds: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Party {
    #[serde(default)]
    pub you: bool,
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
    Manage,
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

impl Request {
    pub(crate) fn answer(
        &self,
        approve: bool,
        choice: Option<String>,
        withheld: Vec<Withheld>,
    ) -> Result<Answer, ConsentError> {
        if !approve {
            return Ok(Answer::Deny);
        }
        match (self, choice) {
            (Self::Pick { .. }, Some(choice)) if withheld.is_empty() => Ok(Answer::Pick { choice }),
            (Self::Pick { .. }, _) | (_, Some(_)) => Err(ConsentError::WrongAnswer),
            (Self::Join { .. } | Self::Reach { .. } | Self::Manage { .. }, None)
                if !withheld.is_empty() =>
            {
                Err(ConsentError::WrongAnswer)
            }
            (_, None) => Ok(Answer::Approve { withheld }),
        }
    }
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
            match (&entry.request, &answer) {
                (Request::Pick { candidates, .. }, Answer::Pick { choice }) => {
                    if !candidates.iter().any(|c| c.choice == *choice) {
                        return Err(ConsentError::UnknownChoice(choice.clone()));
                    }
                }
                (
                    Request::Join { .. } | Request::Reach { .. } | Request::Manage { .. },
                    Answer::Approve { withheld },
                ) if !withheld.is_empty() => {
                    return Err(ConsentError::WrongAnswer);
                }
                (_, Answer::Deny) => {}
                (Request::Pick { .. }, _) | (_, Answer::Pick { .. }) => {
                    return Err(ConsentError::WrongAnswer);
                }
                _ => {}
            }
            if let Answer::Approve { withheld } = &answer {
                for grant in withheld {
                    let known = match &entry.request {
                        Request::Install { mods, .. } => mods.iter().any(|m| {
                            m.local == grant.local && m.grants.iter().any(|g| g.key == grant.key)
                        }),
                        Request::Sign { .. }
                        | Request::Pick { .. }
                        | Request::Join { .. }
                        | Request::Reach { .. }
                        | Request::Manage { .. } => false,
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

    #[tokio::test]
    async fn ac2_manage_answers_and_legacy_requests() {
        let request: Request = serde_json::from_value(json!({"kind":"manage", "asked_by":{"local":"desk","name":"Desk","publisher":null}, "action":"enable", "target":{"local":"weather","name":"Weather","version":"1.0.0","publisher":null},"grant":null,"users":[]})).unwrap();
        let withheld = vec![Withheld {
            local: "weather".into(),
            key: "storage".into(),
        }];
        assert!(matches!(
            request.answer(true, None, withheld.clone()),
            Err(ConsentError::WrongAnswer)
        ));
        assert!(matches!(
            request.answer(true, Some("x".into()), vec![]),
            Err(ConsentError::WrongAnswer)
        ));
        for answer in [Answer::Approve { withheld: vec![] }, Answer::Deny] {
            let b = Broker::new();
            let mut pending = pin!(b.ask(request.clone()));
            queue(pending.as_mut()).await;
            let id = b.current().unwrap().0;
            for wrong in [
                Answer::Approve {
                    withheld: withheld.clone(),
                },
                Answer::Pick { choice: "x".into() },
            ] {
                assert!(matches!(
                    b.answer(&id, wrong),
                    Err(ConsentError::WrongAnswer)
                ));
            }
            b.answer(&id, answer.clone()).unwrap();
            assert_eq!(pending.await, answer);
        }
        assert!(matches!(install(), Request::Install { asked_by: None, .. }));
        assert!(matches!(pick(), Request::Pick { asked_by: None, .. }));
    }

    fn install() -> Request {
        serde_json::from_value(json!({"kind":"install","by_program":false,"mods":[{
            "local":"reading-list","name":"Reading list","slug":"reading-list",
            "version":"0.1.0","release":null,"publisher":null,"replaces":null,
            "grants":[{"key":"storage","kind":"storage","text":"Keep its own data on this device",
            "granted":true,"new":true}],"uses":[],"origin":null,"binds":[]}]}))
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

    fn pick() -> Request {
        serde_json::from_value(json!({"kind":"pick", "consumer":"Dave's stats",
            "provider":"Carol's ratings", "cause":"site unavailable", "candidates":[{
                "choice":"frank/ratings@1.0.0", "name":"Frank ratings", "summary":"Scores",
                "version":"1.0.0", "publisher":{"name":"Frank", "fingerprint":"frank-key"},
                "origin":null, "installed":false, "checked_at":null}]}))
        .unwrap()
    }

    #[tokio::test]
    async fn ac1_pick_answers_validate_kind_choice_and_keep_rejected_head() {
        let sign = serde_json::from_value(json!({"kind":"sign", "statement":"profile",
            "signer":{"name":null,"fingerprint":"key"},"details":[]}))
        .unwrap();
        for request in [pick(), install(), sign] {
            let b = Broker::new();
            let mut pending = pin!(b.ask(request.clone()));
            queue(pending.as_mut()).await;
            let id = b.current().unwrap().0;
            for value in [
                json!({"answer":"pick","choice":"unknown"}),
                json!({"answer":"approve","withheld":[]}),
                json!({"answer":"pick","choice":"frank/ratings@1.0.0"}),
            ] {
                let answer: Answer = serde_json::from_value(value.clone()).unwrap();
                let is_pick = matches!(request, Request::Pick { .. });
                let result = b.answer(&id, answer.clone());
                let accepted = if is_pick {
                    value["choice"] == "frank/ratings@1.0.0"
                } else {
                    value["answer"] == "approve"
                };
                if accepted {
                    result.unwrap();
                    assert_eq!(pending.as_mut().await, answer);
                    break;
                }
                if is_pick && value["answer"] == "pick" {
                    assert!(
                        matches!(result, Err(ConsentError::UnknownChoice(c)) if c == "unknown")
                    );
                } else {
                    assert!(matches!(result, Err(ConsentError::WrongAnswer)));
                }
                assert_eq!(b.current(), Some((id.clone(), request.clone())));
            }
            // Each kind also accepts deny.
            let mut pending = pin!(b.ask(request));
            queue(pending.as_mut()).await;
            b.answer(&b.current().unwrap().0, Answer::Deny).unwrap();
            assert_eq!(pending.await, Answer::Deny);
        }
    }

    #[test]
    fn ac2_answer_argument_table_every_row() {
        let sign: Request = serde_json::from_value(json!({"kind":"sign", "statement":"profile",
            "signer":{"name":null,"fingerprint":"key"},"details":[]}))
        .unwrap();
        for request in [install(), sign, pick()] {
            for approve in [false, true] {
                for choice in [
                    None,
                    Some("frank/ratings@1.0.0".to_string()),
                    Some("unknown".to_string()),
                ] {
                    for withheld in [
                        vec![],
                        vec![Withheld {
                            local: "reading-list".into(),
                            key: "storage".into(),
                        }],
                    ] {
                        let result = request.answer(approve, choice.clone(), withheld.clone());
                        if !approve {
                            assert_eq!(result.unwrap(), Answer::Deny);
                        } else if matches!(request, Request::Pick { .. }) {
                            if choice.is_none() || !withheld.is_empty() {
                                assert!(matches!(result, Err(ConsentError::WrongAnswer)));
                            } else {
                                assert_eq!(
                                    result.unwrap(),
                                    Answer::Pick {
                                        choice: choice.clone().unwrap()
                                    }
                                );
                            }
                        } else if choice.is_some() {
                            assert!(matches!(result, Err(ConsentError::WrongAnswer)));
                        } else {
                            assert_eq!(result.unwrap(), Answer::Approve { withheld });
                        }
                    }
                }
            }
        }
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
                    you: false,
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
            "becomes_main":false,"release":null,"publisher":null,"replaces":null,"grants":[{"key":"storage","kind":"storage",
            "text":"Keep its own data on this device","granted":true,"new":true}],"uses":[],"origin":null,"binds":[]}]});
        assert_eq!(serde_json::to_value(install()).unwrap(), value);
        let sign = json!({"kind":"sign","statement":"release","signer":{"name":"Alice","fingerprint":"abc","you":false},
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
                    ("Release", format!("reading-list 0.1.0 — {reference}")),
                    ("Package", release.package.to_string()),
                    ("Size", "123".into()),
                    ("Source included", "yes".into()),
                    ("Summary", "Books".into()),
                    ("Made from", "none".into()),
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
                    ("Release", format!("reading-list 0.1.0 — {reference}")),
                    ("Package", release.package.to_string()),
                    ("Size", "123".into()),
                    ("Source included", "no".into()),
                    ("Summary", "Books".into()),
                    ("Made from", "none".into()),
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
                    ("Release", format!("reading-list 0.1.0 — {reference}")),
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
                    dist: std::sync::OnceLock::new(),
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

#[cfg(test)]
mod machines_tests {
    use super::*;
    #[test]
    fn ac10_join_and_reach_only_take_plain_approval_or_deny() {
        for request in [
            serde_json::json!({"kind":"join","node":"short","role":"full","text":"join"}),
            serde_json::json!({"kind":"reach","what":"relay","url":"https://example.org/","text":"reach"}),
        ] {
            let request: Request = serde_json::from_value(request).unwrap();
            assert_eq!(
                request.answer(true, None, vec![]).unwrap(),
                Answer::Approve { withheld: vec![] }
            );
            assert_eq!(request.answer(false, None, vec![]).unwrap(), Answer::Deny);
            assert!(matches!(
                request.answer(true, Some("x".into()), vec![]),
                Err(ConsentError::WrongAnswer)
            ));
            assert!(matches!(
                request.answer(
                    true,
                    None,
                    vec![Withheld {
                        local: "x".into(),
                        key: "y".into()
                    }]
                ),
                Err(ConsentError::WrongAnswer)
            ));
        }
    }
}

#[cfg(test)]
mod machines_broker_tests {
    use super::*;
    #[tokio::test]
    async fn ac10_broker_rejects_wrong_join_and_reach_answers_without_consuming_request() {
        for request in [
            Request::Join {
                node: "short".into(),
                role: Role::Full,
                text: "join".into(),
            },
            Request::Reach {
                what: Reach::Dht,
                url: None,
                text: "DHT".into(),
            },
        ] {
            for allowed in [Answer::Approve { withheld: vec![] }, Answer::Deny] {
                let broker = Broker::new();
                let worker = broker.clone();
                let request = request.clone();
                let waiting = tokio::spawn(async move { worker.ask(request).await });
                while broker.current().is_none() {
                    tokio::task::yield_now().await;
                }
                let (id, _) = broker.current().unwrap();
                for answer in [
                    Answer::Pick { choice: "x".into() },
                    Answer::Approve {
                        withheld: vec![Withheld {
                            local: "x".into(),
                            key: "y".into(),
                        }],
                    },
                ] {
                    assert!(matches!(
                        broker.answer(&id, answer),
                        Err(ConsentError::WrongAnswer)
                    ));
                    assert_eq!(broker.current().unwrap().0, id);
                }
                broker.answer(&id, allowed.clone()).unwrap();
                assert_eq!(waiting.await.unwrap(), allowed);
            }
        }
    }
}

#[cfg(test)]
mod machines_pairing_tests {
    use super::*;
    use std::time::{Duration, Instant};
    struct Approve;
    impl watane_dist::Confirm for Approve {
        fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
            watane_dist::Decision::Approve
        }
    }
    fn open(home: &std::path::Path) -> Arc<watane_node::Node> {
        let node = Arc::new(
            watane_node::Node::open(
                home,
                Arc::new(Approve),
                watane_node::Options {
                    net: watane_node::net::Options {
                        bind: Some("127.0.0.1:0".parse().unwrap()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        let mut options = node.net().options();
        options.on = true;
        node.net().set_options(options).unwrap();
        node
    }
    #[test]
    fn ac2_real_pairing_events_wait_for_consent_and_decline_changes_nothing() {
        let home = tempfile::tempdir().unwrap();
        let anchor = open(home.path());
        let broker = Broker::new();
        let _machines = crate::machines::Machines::start(&anchor, broker.clone());
        let mut used = None;
        for approve in [true, false] {
            let replica_home = tempfile::tempdir().unwrap();
            let replica = open(replica_home.path());
            let code = anchor
                .net()
                .pair_code(watane_node::net::Role::Full)
                .unwrap();
            let before = anchor.net().devices();
            let worker = replica.clone();
            let copy = code.clone();
            let joining = std::thread::spawn(move || worker.join(&copy, false));
            let end = Instant::now() + Duration::from_secs(10);
            let (id, request) = loop {
                if let Some(current) = broker.current() {
                    break current;
                }
                assert!(Instant::now() < end, "join never reached broker");
                std::thread::yield_now();
            };
            let Request::Join { node, role, text } = request else {
                panic!("join request");
            };
            assert_eq!(node, replica.net().node_key().short());
            assert_eq!(role, Role::Full);
            assert_eq!(
                text,
                watane_node::words::join_asked(
                    &replica.net().node_key(),
                    watane_node::net::Role::Full
                )
            );
            assert_eq!(anchor.net().devices(), before);
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
            let result = joining.join().unwrap();
            if approve {
                assert_eq!(result.unwrap(), anchor.net().devices().unwrap());
                assert_eq!(replica.net().devices(), anchor.net().devices());
                assert!(replica.dist().identity().is_none());
                assert!(!replica_home.path().join("log/identity.key").exists());
                used = Some(code);
            } else {
                assert!(matches!(
                    result,
                    Err(watane_node::NodeError::Net(
                        watane_node::net::NetError::Declined
                    ))
                ));
                assert_eq!(anchor.net().devices(), before);
            }
        }
        let other_home = tempfile::tempdir().unwrap();
        let other = open(other_home.path());
        assert!(other.join(&used.unwrap(), false).is_err());
        assert!(broker.current().is_none());
    }
}
