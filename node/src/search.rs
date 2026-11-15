//! Local and remote hits (TECHNICAL 5.4).
use crate::{Machine, Node, NodeHit, convert};
use std::collections::BTreeSet;
use watane_dist::{DistError, Hit, PublicKey, Query};
use watane_net::NodeKey;

const MAX_HITS: usize = 200;
/// The cap of net 5.9 on channels pulled per query.
const MAX_PULLS: usize = 32;

impl Node {
    pub fn search(&self, query: &Query) -> Result<Vec<NodeHit>, DistError> {
        let local = self.dist.search(query)?;
        if self.machine() != Machine::Light {
            return Ok(local
                .into_iter()
                .take(MAX_HITS)
                .map(|hit| NodeHit {
                    hit,
                    from: Vec::new(),
                })
                .collect());
        }
        let me = self.net.node_key();
        let remote: Vec<(NodeKey, Vec<watane_net::Hit>)> = self
            .net
            .query(&convert::net_query(query))
            .into_iter()
            .filter(|(node, _)| *node != me)
            .collect();
        let mut channels: BTreeSet<PublicKey> = BTreeSet::new();
        for (_, hits) in &remote {
            channels.extend(hits.iter().map(|h| h.channel));
        }
        let mut needed = 0;
        for channel in &channels {
            let stored = self
                .dist
                .heads(Some(&[*channel]))?
                .into_iter()
                .find_map(|(_, head)| head.map(|h| h.seq));
            let top = remote
                .iter()
                .flat_map(|(_, hits)| hits.iter())
                .filter(|h| h.channel == *channel)
                .map(|h| h.seq)
                .max();
            if stored.is_some_and(|s| top.is_some_and(|t| s < t)) && needed < MAX_PULLS {
                needed += 1;
                let _ = self.net.need(channel);
            }
        }
        let channels: Vec<_> = channels.into_iter().collect();
        let stored = self.dist.search_stored(query, &channels)?;
        // A remote hit is verified when dist stored its entry and shows it as a hit.
        let from_of = |hit: &Hit| -> Vec<NodeKey> {
            remote
                .iter()
                .filter(|(_, hits)| {
                    hits.iter()
                        .any(|h| h.channel == hit.info.channel && h.entry == hit.info.entry)
                })
                .map(|(node, _)| *node)
                .collect()
        };
        let mut out: Vec<NodeHit> = Vec::new();
        for hit in local {
            let from = stored
                .iter()
                .find(|s| s.info.reference == hit.info.reference)
                .map(from_of)
                .unwrap_or_default();
            out.push(NodeHit { hit, from });
        }
        for hit in stored {
            let from = from_of(&hit);
            if !from.is_empty()
                && !out
                    .iter()
                    .any(|o| o.hit.info.reference == hit.info.reference)
            {
                out.push(NodeHit { hit, from });
            }
        }
        out.truncate(MAX_HITS);
        Ok(out)
    }
}
