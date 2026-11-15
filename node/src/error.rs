use std::path::PathBuf;
use watane_dist::DistError;
use watane_net::NetError;

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error(transparent)]
    Dist(#[from] DistError),
    #[error(transparent)]
    Net(#[from] NetError),
    #[error(
        "this home's identity {fingerprint} signed statements; a home that joins your machines holds no identity"
    )]
    IdentityInUse { fingerprint: String },
    #[error("{path} is a watane home with mods; watane-node runs a home of its own")]
    HostHome { path: PathBuf },
    #[error("this hint names no head")]
    EmptyHint,
    #[error("this holder already holds a home")]
    HolderSet,
}
