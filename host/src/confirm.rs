use crate::{LocalName, PlanEntry};
use std::{future::Future, pin::Pin};
use watane_pkg::{Grant, ReleaseRef};

pub trait Confirm: Send + Sync + 'static {
    fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>>;
    /// Called after an install writes the catalog and before it returns.
    /// Runs on the host's thread and must not block. Default: nothing.
    fn installed(&self, report: &Installed) {
        let _ = report;
    }
    /// Step 10 (6.10): a request of a manager. The app decides it in the consent view and acts.
    /// Default: `Failed("this app takes no requests of mods")`.
    fn manage(
        &self,
        request: ManageRequest,
    ) -> Pin<Box<dyn Future<Output = ManageOutcome> + Send>> {
        let _ = request;
        Box::pin(async { ManageOutcome::Failed("this app takes no requests of mods".into()) })
    }
    /// Step 10 (6.10): a manager reduces authority or opens a page; called before the host acts.
    /// Must not block. Default: nothing.
    fn reduced(&self, by: &LocalName, reduction: &Reduction) {
        let _ = (by, reduction);
    }
}
/// The changed plan entries, in install order, after the catalog commit.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub requester: Requester,
    pub new: Vec<LocalName>,
    pub updated: Vec<LocalName>,
}
#[derive(Clone, Debug)]
pub struct InstallRequest {
    pub entries: Vec<PlanEntry>,
    pub requester: Requester,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Requester {
    App,
    LocalApi,
    /// Step 10: the manager that asked (6.10).
    Mod(LocalName),
}
#[derive(Clone, Debug)]
pub enum Answer {
    Approve { withhold: Vec<(LocalName, Grant)> },
    Deny,
}

/// Step 10 (6.10): a request of a manager that the app decides.
#[derive(Clone, Debug, PartialEq)]
pub struct ManageRequest {
    pub id: RequestId,
    pub by: LocalName,
    pub ask: Ask,
}
/// 32 lowercase hex characters from 16 random bytes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(String);
impl RequestId {
    pub(crate) fn random() -> Result<Self, crate::HostError> {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(|e| crate::HostError::Internal(e.to_string()))?;
        Ok(Self(bytes.iter().map(|b| format!("{b:02x}")).collect()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    Install {
        release: ReleaseRef,
        link: Option<String>,
    },
    Update(LocalName),
    Enable(LocalName),
    Grant {
        local: LocalName,
        grant: Grant,
    },
    Remove {
        local: LocalName,
        delete_data: bool,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManageOutcome {
    Approved,
    Denied,
    Failed(String),
}
/// Step 10 (6.10): a reduction of authority or a request to open a page.
#[derive(Clone, Debug, PartialEq)]
pub enum Reduction {
    Disable(LocalName),
    Withdraw {
        local: LocalName,
        grant: Grant,
    },
    ClosePage(LocalName),
    /// Open or focus an installed mod's page. Gives no authority (Q31).
    OpenPage(LocalName),
}
