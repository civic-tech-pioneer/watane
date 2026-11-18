use crate::oracle;
use proptest::prelude::*;
use watane_log::{Entry, Log, Status};
mod chain {
    use std::collections::BTreeMap;
    #[derive(Default)]
    pub struct Chain {
        pub rows: BTreeMap<u64, usize>,
        pub fork: Option<(usize, usize)>,
    }
    impl Chain {
        pub fn ingest(
            &mut self,
            mut batch: Vec<usize>,
            raw: &[Vec<u8>],
            seq: &[u64],
            prev: &[Option<[u8; 32]>],
        ) -> (Vec<usize>, Option<u64>) {
            if self.fork.is_some() {
                return (vec![], None);
            }
            batch.sort_by_key(|&i| (seq[i], *blake3::hash(&raw[i]).as_bytes()));
            batch.dedup();
            let mut stored = vec![];
            let mut gap = None;
            for i in batch {
                if let Some(&old) = self.rows.get(&seq[i]) {
                    if old != i {
                        self.fork = Some((old, i));
                        break;
                    }
                    continue;
                }
                let next = self.rows.len() as u64;
                if seq[i] > next {
                    gap.get_or_insert(seq[i]);
                    continue;
                }
                if seq[i] == next {
                    if let Some((&_, &h)) = self.rows.last_key_value()
                        && prev[i] != Some(*blake3::hash(&raw[h]).as_bytes())
                    {
                        self.fork = Some((h, i));
                        break;
                    }
                    self.rows.insert(seq[i], i);
                    stored.push(i);
                }
            }
            (stored, gap)
        }
    }
}

#[test]
fn ac5_ingest_matches_independent_chain() {
    use proptest::test_runner::{Config, RngSeed, TestRunner};
    let config = Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0),
        ..Config::default()
    };
    let batches =
        prop::collection::vec((0usize..2, prop::collection::vec(0usize..8, 0..10)), 0..30);
    TestRunner::new(config)
        .run(&batches, |batches| {
            let d = oracle::directory();
            let log = Log::open(d.path()).unwrap();
            let mut raws = vec![];
            let mut authors = vec![];
            for seed in [oracle::SEED, [42; 32]] {
                let public =
                    *ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(seed)).pk;
                authors.push(watane_log::PublicKey::from_bytes(&public).unwrap());
                let mut entries = oracle::chain(seed, 6);
                for seq in [1, 3] {
                    let signed =
                        oracle::cbor::signed(&public, seq, Some(&[7; 32]), 0, "a", b"fork");
                    entries.push(oracle::cbor::entry(
                        &public,
                        seq,
                        Some(&[7; 32]),
                        0,
                        "a",
                        b"fork",
                        &oracle::sign(seed, &signed),
                    ));
                }
                raws.push(entries);
            }
            let entries: Vec<Vec<Entry>> = raws
                .iter()
                .map(|channel| {
                    channel
                        .iter()
                        .map(|raw| Entry::decode(raw).unwrap())
                        .collect()
                })
                .collect();
            let mut models = [chain::Chain::default(), chain::Chain::default()];
            for (a, batch) in batches {
                let author = authors[a];
                // Metadata comes from the fixture layout, independently of Entry's parser.
                let seq = [0, 1, 2, 3, 4, 5, 1, 3];
                let mut prev = vec![None];
                prev.extend(
                    raws[a][..5]
                        .iter()
                        .map(|r| Some(*blake3::hash(r).as_bytes())),
                );
                prev.extend([Some([7; 32]); 2]);
                let (stored, gap) = models[a].ingest(batch.clone(), &raws[a], &seq, &prev);
                let input: Vec<_> = batch.iter().map(|&i| entries[a][i].clone()).collect();
                let actual = log.ingest(&author, &input).unwrap();
                let expected_ids: Vec<_> = stored
                    .iter()
                    .map(|&i| *blake3::hash(&raws[a][i]).as_bytes())
                    .collect();
                prop_assert_eq!(
                    actual
                        .stored
                        .iter()
                        .map(|id| *id.as_bytes())
                        .collect::<Vec<_>>(),
                    expected_ids
                );
                prop_assert_eq!(actual.gap, gap);
                let expected: Vec<_> = models[a]
                    .rows
                    .values()
                    .map(|&i| raws[a][i].clone())
                    .collect();
                let actual_entries = log.entries(&author, 0, 100).unwrap();
                prop_assert_eq!(
                    actual_entries
                        .iter()
                        .map(|e| e.bytes().to_vec())
                        .collect::<Vec<_>>(),
                    expected.clone()
                );
                prop_assert_eq!(
                    log.head(&author)
                        .unwrap()
                        .map(|h| (h.seq, *h.id.as_bytes())),
                    expected
                        .last()
                        .map(|raw| (expected.len() as u64 - 1, *blake3::hash(raw).as_bytes()))
                );
                match (&models[a].fork, log.status(&author).unwrap()) {
                    (None, Status::Ok) => {}
                    (Some((chain, other)), Status::Compromised(f)) => {
                        prop_assert_eq!(f.chain.bytes(), raws[a][*chain].as_slice());
                        prop_assert_eq!(f.other.bytes(), raws[a][*other].as_slice());
                    }
                    _ => prop_assert!(false, "fork status differs from model"),
                }
            }
            Ok(())
        })
        .unwrap();
}
