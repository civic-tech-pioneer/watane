use crate::*;
pub trait Confirm: Send + Sync {
    /// Blocks until the participant answers in the app. dist holds its
    /// own-channel lock during the call, so an implementation must not call
    /// set_name, publish, withdraw, list, include or unlist of the same Dist
    /// (it would wait forever).
    fn confirm(&self, request: &SignRequest) -> Decision;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Refuse,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SignRequest {
    pub channel: PublicKey,
    pub fingerprint: String,
    pub statement: Statement, // decoded from the exact body bytes that dist signs
}
#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    Profile(Profile),
    List(Listing),
    Include(Listing),
    Unlist {
        entry: EntryId,
        listing: Listing,
        include: bool,
    },
    /// `reference`: own key, slug and version. `release`: pkg's body (PKG-F-20).
    Release {
        reference: ReleaseRef,
        release: Release,
    },
    /// `reference` and `release`: the release that it withdraws.
    Withdraw {
        withdraw: Withdraw,
        reference: ReleaseRef,
        release: Release,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Withdraw {
    pub release: EntryId,
    pub reason: String,
} // the body (4.1)
impl Statement {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Profile(_) => "profile",
            Self::List(_) => "list",
            Self::Include(_) => "include",
            Self::Unlist { .. } => "unlist",
            Self::Release { .. } => "release",
            Self::Withdraw { .. } => "withdraw",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    pub link: Link,
    pub note: String,
}
