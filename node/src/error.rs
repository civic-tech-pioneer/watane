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
        "this home's identity {fingerprint} has signed at least one statement; a home that joins your machines holds no identity"
    )]
    IdentityInUse { fingerprint: String },
    #[error("{path} is a watane home with mods; watane-node runs a home of its own")]
    HostHome { path: PathBuf },
    #[error("this hint names no head")]
    EmptyHint,
    #[error("this holder already holds a home")]
    HolderSet,
}

#[cfg(test)]
mod tests {
    #[test]
    fn ac10_identity_in_use_is_true_after_one_statement() {
        let text = super::NodeError::IdentityInUse { fingerprint: "Alice".into() }.to_string();
        assert_eq!(text,"this home's identity Alice has signed at least one statement; a home that joins your machines holds no identity");
    }
}
