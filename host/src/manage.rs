//! Step 10 (6.10): the manager's functions, the request table and the
//! application of a chain's manage effects.
use crate::{
    Ask, CallCode, CallError, Host, InstalledMod, LinkKind, LocalName, ManageOutcome, ManageRecord,
    ManageRequest, Reduction, RequestId, bus,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use tokio::sync::oneshot;
use watane_pkg::Grant;

pub(crate) const MANAGE: &str = "watane:host/manage@0.1.0";
const KEPT: usize = 32;

/// What a manager's successful invoke asks the host to do (6.10).
#[derive(Clone, Debug)]
pub(crate) struct Effect {
    pub by: LocalName,
    pub record: ManageRecord,
    pub action: Action,
}
#[derive(Clone, Debug)]
pub(crate) enum Action {
    Reduce(Reduction),
    Request(RequestId, Ask),
    /// A harness records and applies nothing.
    Fake {
        request: bool,
    },
}

#[derive(Clone, Debug)]
enum Finished {
    Approved,
    Denied,
    Failed(String),
}
#[derive(Debug, Default)]
struct Manager {
    pending: Option<(RequestId, oneshot::Sender<()>)>,
    finished: VecDeque<(RequestId, Finished)>,
}
#[derive(Debug, Default)]
pub(crate) struct Requests(BTreeMap<LocalName, Manager>);
impl Requests {
    fn finish(&mut self, by: &LocalName, id: RequestId, state: Finished) {
        let manager = self.0.entry(by.clone()).or_default();
        manager.finished.push_back((id, state));
        while manager.finished.len() > KEPT {
            manager.finished.pop_front();
        }
    }
    fn pending(&self, by: &LocalName) -> Option<&RequestId> {
        self.0.get(by)?.pending.as_ref().map(|(id, _)| id)
    }
    fn state(&self, by: &LocalName, id: &str) -> Value {
        let Some(manager) = self.0.get(by) else {
            return Value::Null;
        };
        if manager
            .pending
            .as_ref()
            .is_some_and(|(p, _)| p.as_str() == id)
        {
            return json!("pending");
        }
        match manager.finished.iter().find(|(f, _)| f.as_str() == id) {
            None => Value::Null,
            Some((_, Finished::Approved)) => json!("approved"),
            Some((_, Finished::Denied)) => json!("denied"),
            Some((_, Finished::Failed(m))) => json!({"failed": m}),
        }
    }
    /// Drops the future of a pending request: it ends denied.
    fn cancel(&mut self, by: &LocalName) {
        let Some((id, _sender)) = self.0.get_mut(by).and_then(|m| m.pending.take()) else {
            return;
        };
        self.finish(by, id, Finished::Denied);
    }
}

fn failure(code: &str, message: impl std::fmt::Display) -> Value {
    json!({"err": {"code": code, "message": message.to_string()}})
}
fn ok(value: Value) -> Value {
    json!({"ok": value})
}
fn bad(path: &str, reason: impl std::fmt::Display) -> Value {
    failure("bad_request", format!("{path}: {reason}"))
}

pub(crate) fn grant_json(grant: &Grant) -> Value {
    match grant {
        Grant::Storage => json!("storage"),
        Grant::Manage => json!("manage"),
        Grant::ClaudeCode => json!("claude-code"),
        Grant::ClaudeCodeShell => json!("claude-code-shell"),
        Grant::Http { host } => json!({"http": host.as_str()}),
        Grant::Call {
            interface,
            provider,
        } => {
            json!({"call": {"interface": interface.to_string(), "provider": provider.to_string()}})
        }
        Grant::Subscribe {
            interface,
            provider,
        } => {
            json!({"subscribe": {"interface": interface.to_string(), "provider": provider.to_string()}})
        }
    }
}
fn grant_from(value: &Value) -> Result<Grant, String> {
    let provided = |v: &Value| -> Result<_, String> {
        let text = |key: &str| {
            v.get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("missing {key}"))
        };
        Ok((
            text("interface")?.parse().map_err(|e| format!("{e}"))?,
            crate::catalog::parse_identity(text("provider")?)
                .ok_or_else(|| "not a provider identity".to_owned())?,
        ))
    };
    match value {
        Value::String(s) if s == "storage" => Ok(Grant::Storage),
        Value::String(s) if s == "manage" => Ok(Grant::Manage),
        Value::String(s) if s == "claude-code" => Ok(Grant::ClaudeCode),
        Value::String(s) if s == "claude-code-shell" => Ok(Grant::ClaudeCodeShell),
        Value::Object(map) if map.len() == 1 => {
            let (case, payload) = map
                .iter()
                .next()
                .expect("the map was checked to hold one entry");
            match case.as_str() {
                "http" => Ok(Grant::Http {
                    host: payload
                        .as_str()
                        .ok_or("http host is not a string")?
                        .parse()
                        .map_err(|e| format!("{e}"))?,
                }),
                "call" => provided(payload).map(|(interface, provider)| Grant::Call {
                    interface,
                    provider,
                }),
                "subscribe" => provided(payload).map(|(interface, provider)| Grant::Subscribe {
                    interface,
                    provider,
                }),
                other => Err(format!("unknown grant {other}")),
            }
        }
        _ => Err("not a grant".into()),
    }
}

pub(crate) fn mod_info(m: &InstalledMod) -> Value {
    let text = |o: Option<String>| o.map_or(Value::Null, Value::String);
    json!({
        "local": m.local.to_string(),
        "identity": m.identity,
        "slug": m.slug.to_string(),
        "name": m.name,
        "summary": m.summary,
        "version": m.version.to_string(),
        "publisher": text(m.publisher.as_ref().map(ToString::to_string)),
        "release": text(m.release.as_ref().map(ToString::to_string)),
        "origin": text(m.origin.as_ref().map(ToString::to_string)),
        "enabled": m.enabled,
        "has-page": m.page.is_some(),
        "has-logic": m.has_logic,
        "grants": m.grants.iter().map(|g| json!({"grant": grant_json(&g.grant), "granted": g.granted})).collect::<Vec<_>>(),
        "links": m.links.iter().map(|l| json!({
            "kind": match l.kind { LinkKind::Call => "call", LinkKind::Subscribe => "subscribe" },
            "interface": l.interface.to_string(),
            "provider": text(l.provider.as_ref().map(ToString::to_string)),
            "provider-interface": text(l.provider_interface.as_ref().map(ToString::to_string)),
        })).collect::<Vec<_>>(),
        "binds": m.binds.iter().map(|b| json!({
            "consumer": b.consumer, "provider": b.provider.to_string(), "to": b.to,
        })).collect::<Vec<_>>(),
        "error": text(m.error.clone()),
    })
}

fn local_arg(args: &Value, key: &str) -> Result<LocalName, Value> {
    let text = args.get(key).and_then(Value::as_str).unwrap_or_default();
    LocalName::parse(text).map_err(|e| bad(&format!("$.{key}"), e))
}

impl Host {
    /// True when `local` is enabled and holds `Manage` granted, now.
    pub(crate) fn is_manager(&self, local: &LocalName) -> bool {
        self.catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .mods
            .iter()
            .any(|m| {
                &m.local == local
                    && m.enabled
                    && m.error.is_none()
                    && m.grants
                        .iter()
                        .any(|g| g.granted && matches!(g.grant, Grant::Manage))
            })
    }
    fn app(&self) -> Option<Arc<dyn crate::Confirm>> {
        match &self.home.mode {
            crate::Mode::Home { app } => app.clone(),
            crate::Mode::Preview => None,
        }
    }

    /// One function of `watane:host/manage`, called by the manager `by`.
    /// `queued` holds the effects of this store; `chain` those of the chain so far.
    pub(crate) fn manage_call(
        &self,
        by: &LocalName,
        fake: Option<(&BTreeMap<String, Value>, usize)>,
        queued: &mut Vec<Effect>,
        chain: &[Effect],
        function: &str,
        args: Value,
    ) -> Result<Value, CallError> {
        if !self.is_manager(by) {
            return Err(CallError::new(
                CallCode::NotGranted,
                format!("{by} has no grant for watane:host/manage"),
            ));
        }
        let is_request = function.starts_with("request-");
        let is_effect = is_request
            || matches!(
                function,
                "disable" | "withdraw" | "close-page" | "open-page"
            );
        if let Some((fakes, serial)) = fake {
            let faked = fakes.get(function).cloned();
            if !is_effect {
                return Ok(faked.unwrap_or_else(|| match function {
                    "mods" => json!([]),
                    _ => json!("pending"),
                }));
            }
            let number = 1
                + serial
                + queued
                    .iter()
                    .chain(chain)
                    .filter(|e| matches!(e.action, Action::Fake { request: true }))
                    .count();
            queued.push(Effect {
                by: by.clone(),
                record: ManageRecord {
                    function: function.into(),
                    args,
                },
                action: Action::Fake {
                    request: is_request,
                },
            });
            return Ok(faked.unwrap_or_else(|| {
                ok(if is_request {
                    json!(format!("r{number}"))
                } else {
                    Value::Null
                })
            }));
        }
        match function {
            "mods" => {
                let mut mods = self
                    .catalog
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it")
                    .mods
                    .clone();
                mods.sort_by(|a, b| a.local.cmp(&b.local));
                return Ok(Value::Array(mods.iter().map(mod_info).collect()));
            }
            "request" => {
                let id = args.get("id").and_then(Value::as_str).unwrap_or_default();
                let queued_here = queued.iter().chain(chain).any(|e| {
                    &e.by == by && matches!(&e.action, Action::Request(r, _) if r.as_str() == id)
                });
                if queued_here {
                    return Ok(json!("pending"));
                }
                return Ok(self
                    .requests
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it")
                    .state(by, id));
            }
            _ => {}
        }
        Ok(self
            .manage_effect(by, queued, chain, function, args, is_request)
            .unwrap_or_else(|failed| failed))
    }

    fn manage_effect(
        &self,
        by: &LocalName,
        queued: &mut Vec<Effect>,
        chain: &[Effect],
        function: &str,
        args: Value,
        is_request: bool,
    ) -> Result<Value, Value> {
        if is_request {
            match &self.home.mode {
                crate::Mode::Preview => {
                    return Err(failure("refused", "a preview has no consent view"));
                }
                crate::Mode::Home { app: None } => {
                    return Err(failure(
                        "refused",
                        "no app is attached to decide the request",
                    ));
                }
                crate::Mode::Home { app: Some(_) } => {}
            }
        }
        let local = if function == "request-install" {
            None
        } else {
            Some(local_arg(&args, "local")?)
        };
        let target = match &local {
            Some(local) => Some(
                self.catalog
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it")
                    .mods
                    .iter()
                    .find(|m| &m.local == local)
                    .cloned()
                    .ok_or_else(|| failure("no_mod", format!("no mod {local}")))?,
            ),
            None => None,
        };
        let grant = || -> Result<Grant, Value> {
            grant_from(args.get("grant").unwrap_or(&Value::Null)).map_err(|e| bad("$.grant", e))
        };
        let declared = |grant: &Grant| -> Result<bool, Value> {
            let target = target.as_ref().expect("only request-install has no target");
            target
                .grants
                .iter()
                .find(|g| &g.grant == grant)
                .map(|g| g.granted)
                .ok_or_else(|| {
                    failure(
                        "not_declared",
                        format!("{} does not declare {grant}", target.local),
                    )
                })
        };
        let local = || {
            local
                .clone()
                .expect("only request-install has no local name")
        };
        let effect = |action: Action| Effect {
            by: by.clone(),
            record: ManageRecord {
                function: function.into(),
                args: args.clone(),
            },
            action,
        };
        if !is_request {
            let reduction = match function {
                "disable" => Reduction::Disable(local()),
                "close-page" => Reduction::ClosePage(local()),
                "open-page" => {
                    let target = target.as_ref().expect("only request-install has no target");
                    if !target.enabled || target.page.is_none() {
                        return Err(failure(
                            "bad_request",
                            format!("{} is disabled or has no page", target.local),
                        ));
                    }
                    Reduction::OpenPage(local())
                }
                _ => {
                    let grant = grant()?;
                    declared(&grant)?;
                    Reduction::Withdraw {
                        local: local(),
                        grant,
                    }
                }
            };
            queued.push(effect(Action::Reduce(reduction)));
            return Ok(ok(Value::Null));
        }
        let ask = match function {
            "request-install" => Ask::Install {
                release: args
                    .get("release")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .parse()
                    .map_err(|e| bad("$.release", e))?,
                link: args.get("link").and_then(Value::as_str).map(str::to_owned),
            },
            "request-update" => Ask::Update(local()),
            "request-enable" => {
                if target
                    .as_ref()
                    .expect("only request-install has no target")
                    .enabled
                {
                    return Err(failure("bad_request", format!("{} is enabled", local())));
                }
                Ask::Enable(local())
            }
            "request-grant" => {
                let grant = grant()?;
                if declared(&grant)? {
                    return Err(failure("bad_request", format!("{grant} is granted")));
                }
                Ask::Grant {
                    local: local(),
                    grant,
                }
            }
            _ => Ask::Remove {
                local: local(),
                delete_data: args
                    .get("delete-data")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
        };
        let open = self
            .requests
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .pending(by)
            .cloned()
            .or_else(|| {
                queued.iter().chain(chain).find_map(|e| match &e.action {
                    Action::Request(id, _) if &e.by == by => Some(id.clone()),
                    _ => None,
                })
            });
        if let Some(open) = open {
            return Err(failure(
                "request_open",
                format!("{by} already has a request open: {open}"),
            ));
        }
        let id = RequestId::random().map_err(|e| failure("refused", e))?;
        queued.push(effect(Action::Request(id.clone(), ask)));
        Ok(ok(json!(id.as_str())))
    }

    /// Applies the manage effects of a finished chain, in order (6.10).
    pub(crate) fn apply_manage(&self, effects: Vec<Effect>) {
        for effect in effects {
            let Effect { by, action, .. } = effect;
            match action {
                Action::Fake { .. } => {}
                Action::Reduce(reduction) => {
                    if !self.is_manager(&by) {
                        eprintln!("watane: {by} lost its grant to manage; reduction dropped");
                        continue;
                    }
                    if let Some(app) = self.app() {
                        app.reduced(&by, &reduction);
                    }
                    let result = match &reduction {
                        Reduction::Disable(local) => self.disable(local),
                        Reduction::Withdraw { local, grant } => self.revoke(local, grant),
                        Reduction::ClosePage(_) | Reduction::OpenPage(_) => Ok(()),
                    };
                    if let Err(error) = result {
                        eprintln!("watane: {by}: {reduction:?} failed: {error}");
                    }
                }
                Action::Request(id, ask) => self.start_request(by, id, ask),
            }
        }
    }

    fn start_request(&self, by: LocalName, id: RequestId, ask: Ask) {
        let app = self.app().filter(|_| self.is_manager(&by));
        let Some(app) = app else {
            eprintln!("watane: request {id} of {by} dropped");
            self.requests
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .finish(&by, id, Finished::Denied);
            self.catalog_written();
            return;
        };
        if self
            .requests
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .pending(&by)
            .is_some()
        {
            self.requests
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .finish(&by, id, Finished::Denied);
            return;
        }
        let future = app.manage(ManageRequest {
            id: id.clone(),
            by: by.clone(),
            ask,
        });
        let (cancel, cancelled) = oneshot::channel::<()>();
        self.requests
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .0
            .entry(by.clone())
            .or_default()
            .pending = Some((id.clone(), cancel));
        let weak = std::sync::Weak::<Host>::clone(&self.weak);
        let (thread_by, thread_id) = (by.clone(), id);
        let spawned = std::thread::Builder::new()
            .name("watane-request".into())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                let outcome = runtime.block_on(async {
                    tokio::select! {
                        outcome = future => Some(outcome),
                        _ = cancelled => None,
                    }
                });
                if let (Some(outcome), Some(host)) = (outcome, weak.upgrade()) {
                    host.finish_request(&thread_by, &thread_id, outcome);
                }
            });
        if spawned.is_err() {
            self.requests
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .cancel(&by);
        }
        // The manager may have lost its grant since the check above.
        if !self.is_manager(&by) {
            self.requests
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .cancel(&by);
        }
        self.catalog_written();
    }

    fn finish_request(&self, by: &LocalName, id: &RequestId, outcome: ManageOutcome) {
        {
            let mut requests = self
                .requests
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            let manager = requests.0.entry(by.clone()).or_default();
            if manager.pending.as_ref().is_none_or(|(p, _)| p != id) {
                return;
            }
            manager.pending = None;
            requests.finish(
                by,
                id.clone(),
                match outcome {
                    ManageOutcome::Approved => Finished::Approved,
                    ManageOutcome::Denied => Finished::Denied,
                    ManageOutcome::Failed(message) => Finished::Failed(message),
                },
            )
        };
        self.catalog_written();
    }

    /// After each catalog write and each change of a request state (6.10):
    /// a manager that lost `Manage` loses its pending request, and every
    /// manager's pages are told to re-read.
    pub(crate) fn catalog_written(&self) {
        // A mod stream depends on the consumer's `Call` grant of its link (19.2, Ends).
        self.end_lost_streams();
        // A handle depends on its owner and its origin client (19.5).
        self.end_lost_handles();
        let managers: Vec<LocalName> = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .mods
            .iter()
            .filter(|m| {
                m.enabled
                    && m.error.is_none()
                    && m.grants
                        .iter()
                        .any(|g| g.granted && matches!(g.grant, Grant::Manage))
            })
            .map(|m| m.local.clone())
            .collect();
        {
            let mut requests = self
                .requests
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            let lost: Vec<_> = requests
                .0
                .iter()
                .filter(|(local, m)| m.pending.is_some() && !managers.contains(local))
                .map(|(local, _)| local.clone())
                .collect();
            for local in lost {
                requests.cancel(&local);
            }
        }
        for local in managers {
            self.bus.send(&local, bus::CATALOG);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Limits, Mode};
    use std::path::Path;

    /// Q24's rule in 6.10: the grant is checked again before each effect,
    /// so effects queued after the manager withdrew its own `Manage` drop.
    #[test]
    fn effects_recheck_manage_before_each() {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        let fixture = |name| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/mods")
                .join(name)
        };
        host.install_folders(&[fixture("manager"), fixture("counter")], 0)
            .unwrap();
        let (manager, counter) = (
            LocalName::parse("manager").unwrap(),
            LocalName::parse("counter").unwrap(),
        );
        let effect = |action| Effect {
            by: manager.clone(),
            record: ManageRecord {
                function: String::new(),
                args: Value::Null,
            },
            action,
        };
        let id = RequestId::random().unwrap();
        host.apply_manage(vec![
            effect(Action::Reduce(Reduction::Withdraw {
                local: manager.clone(),
                grant: Grant::Manage,
            })),
            effect(Action::Reduce(Reduction::Disable(counter.clone()))),
            effect(Action::Request(id.clone(), Ask::Enable(counter.clone()))),
        ]);
        assert!(!host.is_manager(&manager));
        assert!(host.get(&counter).unwrap().enabled);
        assert_eq!(
            host.requests
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .state(&manager, id.as_str()),
            json!("denied")
        );
    }
}
