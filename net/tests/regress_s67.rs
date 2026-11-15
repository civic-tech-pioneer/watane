//! S67 user-sim regressions.
mod support;
use support::*;
use watane_net::{BlobId, NetError, Role};

/// NET-F-32, AC6: a light machine asks for a package the reachable full
/// machine does not have (or will not serve: its file does not match). The
/// error must give watane's cause, not iroh's transport words
/// ("io: stream reset by peer: error 1").
#[test]
fn regress_s67_a_missing_package_has_a_cause_in_watanes_words() {
    let anchor = node(MemHoldings::custom(Default::default(), None, true), None);
    let light = node(MemHoldings::empty(), None);
    pair_into(&anchor, &light, Role::Light);
    let none = BlobId([7; 32]);
    match light.net.need_blob(&none, 10) {
        Err(NetError::Unreachable { node, cause }) => {
            assert_eq!(node, anchor.key);
            assert!(cause.contains(&hex(&none.0[..4])), "{cause}");
            assert!(
                !cause.contains("stream reset") && !cause.contains("io:"),
                "iroh's words, not watane's: {cause}"
            );
        }
        other => panic!("{other:?}"),
    }
}
