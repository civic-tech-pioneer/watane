//! `watane-node`: one home's `dist` and `net` together (stage 1: the holder).
//! See `TECHNICAL.md`.

mod calls;
mod convert;
mod error;
mod holdings;
mod search;
pub mod words;

pub use error::NodeError;
pub use holdings::DistHoldings;
pub use watane_dist as dist;
pub use watane_net as net;

use std::{path::Path, sync::Arc};
use watane_dist::Dist;
use watane_net::Net;

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub dist: watane_dist::Options,
    pub net: watane_net::Options,
}

/// This home's place among the participant's machines, from net's device list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine {
    Alone,
    Anchor,
    Full,
    Light,
    Removed,
}

/// A hit of this home's search, with the machines that gave it (5.4).
#[derive(Clone, Debug)]
pub struct NodeHit {
    pub hit: watane_dist::Hit,
    /// Empty for a hit of this home only.
    pub from: Vec<watane_net::NodeKey>,
}

/// A full machine's answer about the own channel's head (net 5.11).
#[derive(Clone, Debug)]
pub struct OwnHint {
    pub node: watane_net::NodeKey,
    pub head: Option<watane_net::HeadInfo>,
}

/// One per home. Every method blocks.
pub struct Node {
    dist: Arc<Dist>,
    net: Net,
    holdings: Arc<DistHoldings>,
}

impl Node {
    /// The app's open (5.6): a replica opens keyless, any other home with its identity.
    pub fn open(
        home: &Path,
        confirm: Arc<dyn watane_dist::Confirm>,
        options: Options,
    ) -> Result<Node, NodeError> {
        let holdings = Arc::new(DistHoldings::empty());
        let net = Net::open(&home.join("net"), holdings.clone(), options.net)?;
        let replica = net.devices().is_some_and(|l| l.anchor != net.node_key());
        let dist = if replica {
            Dist::open_keyless(home, options.dist)?
        } else {
            Dist::open(home, confirm, options.dist)?
        };
        Ok(Self::done(dist, net, holdings))
    }

    /// watane-node's open: always keyless. Refuses a home with an identity.
    pub fn open_keyless(home: &Path, options: Options) -> Result<Node, NodeError> {
        if home.join("home.json").exists() {
            return Err(NodeError::HostHome {
                path: home.to_owned(),
            });
        }
        let holdings = Arc::new(DistHoldings::empty());
        let net = Net::open(&home.join("net"), holdings.clone(), options.net)?;
        let dist = Dist::open_keyless(home, options.dist)?;
        Ok(Self::done(dist, net, holdings))
    }

    fn done(dist: Dist, net: Net, holdings: Arc<DistHoldings>) -> Node {
        let dist = Arc::new(dist);
        holdings
            .set(dist.clone())
            .expect("a new holder holds no home yet");
        Node {
            dist,
            net,
            holdings,
        }
    }

    pub fn dist(&self) -> &Arc<Dist> {
        &self.dist
    }

    pub fn net(&self) -> &Net {
        &self.net
    }

    pub fn machine(&self) -> Machine {
        let Some(list) = self.net.devices() else {
            return Machine::Alone;
        };
        let me = self.net.node_key();
        if list.anchor == me {
            return Machine::Anchor;
        }
        match list.devices.iter().find(|(key, _)| *key == me) {
            Some((_, watane_net::Role::Full)) => Machine::Full,
            Some((_, watane_net::Role::Light)) => Machine::Light,
            None => Machine::Removed,
        }
    }

    pub fn close(self) {
        self.net.close();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn node_is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<super::Node>();
    }
}
