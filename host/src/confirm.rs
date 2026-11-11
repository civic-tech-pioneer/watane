use crate::{LocalName, PlanEntry};
use std::{future::Future, pin::Pin};
use watane_pkg::Grant;

pub trait Confirm: Send + Sync + 'static {
    fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>>;
}
#[derive(Clone, Debug)]
pub struct InstallRequest {
    pub entries: Vec<PlanEntry>,
    pub requester: Requester,
}
#[derive(Clone, Debug)]
pub enum Requester {
    App,
    LocalApi,
}
#[derive(Clone, Debug)]
pub enum Answer {
    Approve { withhold: Vec<(LocalName, Grant)> },
    Deny,
}
