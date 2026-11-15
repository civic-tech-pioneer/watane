//! S6 and S7: a light laptop searches and installs through the server.
mod support;
use std::{fs, sync::Arc};
use support::*;
use watane_dist::{PublicKey, Query};
use watane_net::{BlobId, Entry, Hit, Holdings, Ingested, Role, Why};
use watane_node::Node;

fn reading() -> Query {
    Query {
        text: "reading".into(),
        ..Default::default()
    }
}

struct World {
    _dirs: Vec<tempfile::TempDir>,
    laptop: Node,
    server: Node,
    light: Node,
}

/// Alice's laptop (anchor), the server (full) and a light laptop, all paired.
fn world() -> World {
    let dirs: Vec<_> = (0..3).map(|_| tempfile::tempdir().unwrap()).collect();
    let laptop = app(dirs[0].path());
    let server = keyless(dirs[1].path());
    let light = keyless(dirs[2].path());
    pair(&laptop, &light, Role::Light, false);
    pair(&laptop, &server, Role::Full, false);
    introduce(&light, &server);
    // the machines are all connected before anyone publishes: a hint sent while a
    // machine is still joining the topic is only caught by the 10-minute sync
    light.net().hello(&key_of(&server)).unwrap();
    server.net().hello(&key_of(&light)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    World {
        _dirs: dirs,
        laptop,
        server,
        light,
    }
}

/// Waits until the server holds the release's entry and package. A workaround for a net
/// gap (node TECHNICAL 12): net acts on a hint with the machine that relayed it over
/// gossip, which may not hold the head yet, and then nothing is pulled until the next
/// sync. Then the server reconnects, which syncs (net 5.6, on connect), as its 10-minute
/// sync would. Remove it, and the one in S7, once net pulls from the machine that told.
fn keeps(w: &World, p: &watane_dist::Published) -> bool {
    let alice = w.laptop.dist().identity().unwrap();
    let want = Some((p.entry.seq(), p.entry.id()));
    let held = || {
        head(&w.server, &alice) == want
            && w.server
                .dist()
                .package_file(&p.info.release.package)
                .is_some()
    };
    if wait_for(3, held) {
        return true;
    }
    reconnect(&w.server, &w.laptop);
    wait_for(12, held)
}

fn reconnect(node: &Node, peer: &Node) {
    node.net().set_options(Default::default()).unwrap();
    node.net().set_options(net_opts()).unwrap();
    introduce(node, peer);
}

#[test]
fn s6_the_light_laptop_searches_and_installs_through_the_server() {
    let w = world();
    let published = w.laptop.publish(package("1.0.0")).unwrap();
    let want = fs::read(
        w.laptop
            .dist()
            .package_file(&published.info.release.package)
            .unwrap(),
    )
    .unwrap();
    assert!(keeps(&w, &published));
    assert!(w.light.dist().search(&reading()).unwrap().is_empty());

    let hits = w.light.search(&reading()).unwrap();
    assert_eq!(hits.len(), 1, "{hits:?}");
    let short = key_of(&w.server).short();
    assert_eq!(hits[0].from, vec![key_of(&w.server)], "from {short}");
    assert_eq!(hits[0].hit.info.reference, published.info.reference);

    // no link location and no web host, and the laptop is gone: the package can only
    // come from the server
    w.laptop.net().set_options(Default::default()).unwrap();
    let fetched = w.light.fetch(&hits[0].hit.info.reference).unwrap();
    assert_eq!(fetched.package.as_bytes(), &want[..]);
    assert!(fetched.link.is_none());
}

/// A machine whose holder answers every search with one hit that names the entry `entry`.
struct Liar {
    hit: Hit,
}
impl Holdings for Liar {
    fn is_anchor(&self) -> bool {
        false
    }
    fn heads(&self, _: Option<&[PublicKey]>) -> Vec<(PublicKey, Option<watane_net::HeadInfo>)> {
        Vec::new()
    }
    fn entries(&self, _: &PublicKey, _: u64, _: usize, _: usize) -> Vec<Entry> {
        Vec::new()
    }
    fn fork(&self, _: &PublicKey) -> Option<(Entry, Entry)> {
        None
    }
    fn blob(&self, _: &BlobId) -> Option<std::path::PathBuf> {
        None
    }
    fn wanted_blobs(&self, _: Role) -> Vec<(BlobId, u64)> {
        Vec::new()
    }
    fn ingest(&self, _: &PublicKey, _: Vec<Entry>, _: Why) -> Ingested {
        Ingested::NotWanted
    }
    fn store_blob(&self, _: &BlobId, _: &std::path::Path) -> Result<(), String> {
        Err("no".into())
    }
    fn search(&self, _: &watane_net::Query) -> Vec<Hit> {
        vec![self.hit.clone()]
    }
}

#[test]
fn s6_a_hit_whose_entry_the_light_home_did_not_verify_is_not_shown() {
    // The light home holds Alice's channel (it fetched her release); the only full machine
    // answers a hit of her channel with an entry id that is not the stored one.
    let dirs: Vec<_> = (0..3).map(|_| tempfile::tempdir().unwrap()).collect();
    let laptop = app(dirs[0].path());
    let light = keyless(dirs[1].path());
    pair(&laptop, &light, Role::Light, false);
    let published = laptop.publish(package("1.0.0")).unwrap();
    let alice = laptop.dist().identity().unwrap();
    // the light home takes Alice's channel from the anchor, as it would from a full machine
    assert!(wait_for(10, || light.net().need(&alice).is_ok()));
    assert!(head(&light, &alice).is_some());

    let liar_net = watane_net::Net::open(
        &dirs[2].path().join("net"),
        Arc::new(Liar {
            hit: Hit {
                channel: alice,
                seq: published.entry.seq(),
                entry: watane_dist::EntryId::from_bytes([9; 32]),
            },
        }),
        net_opts(),
    )
    .unwrap();
    liar_net.set_options(net_opts()).unwrap();
    // pair the liar as a full machine through the anchor's net
    let events = laptop.net().events();
    let code = laptop.net().pair_code(Role::Full).unwrap();
    let liar_key = liar_net.node_key();
    std::thread::scope(|s| {
        let joined = s.spawn(|| liar_net.join(&code, false));
        loop {
            if let watane_net::Event::JoinAsked { node, .. } = events
                .recv_timeout(std::time::Duration::from_secs(15))
                .unwrap()
            {
                laptop.net().answer_join(&node, true);
                break;
            }
        }
        joined.join().unwrap().unwrap();
    });
    laptop.net().add_addrs(&liar_key, &liar_net.addrs());
    liar_net.add_addrs(&key_of(&laptop), &laptop.net().addrs());
    light.net().add_addrs(&liar_key, &liar_net.addrs());
    liar_net.add_addrs(&key_of(&light), &light.net().addrs());
    assert!(wait_for(10, || light
        .net()
        .devices()
        .is_some_and(|l| l.devices.iter().any(|(k, _)| *k == liar_key))
        && liar_net.devices().is_some_and(|l| l
            .devices
            .iter()
            .any(|(k, _)| *k == key_of(&light)))));

    let remote = light.net().query(&watane_net::Query {
        text: Some("reading".into()),
        publisher: None,
        interface: None,
        origin: None,
        limit: 10,
    });
    assert!(
        remote
            .iter()
            .any(|(n, hits)| *n == liar_key && hits.len() == 1),
        "the liar answered: {remote:?}"
    );
    let hits = light.search(&reading()).unwrap();
    assert!(hits.iter().all(|h| h.from.is_empty()), "{hits:?}");
    assert!(
        hits.iter()
            .all(|h| h.hit.info.entry == published.entry.id()),
        "no hit with the made-up entry"
    );
    liar_net.close();
}

#[test]
fn s7_the_installed_mod_gets_the_next_release_through_the_server() {
    let w = world();
    let first = w.laptop.publish(package("1.0.0")).unwrap();
    let alice = w.laptop.dist().identity().unwrap();
    assert!(keeps(&w, &first));
    let hits = w.light.search(&reading()).unwrap();
    let fetched = w.light.fetch(&hits[0].hit.info.reference).unwrap();
    assert!(fetched.link.is_none());
    w.light.installed(&fetched).unwrap();
    assert!(w.light.dist().channel(&alice).unwrap().kept);

    let second = w.laptop.publish(package("0.2.0")).unwrap();
    let slug = second.info.release.slug.clone();
    let version = second.info.release.version.clone();
    let offered = || {
        w.light.refresh();
        w.light
            .dist()
            .latest(&alice, &slug)
            .unwrap()
            .is_some_and(|r| r.release.version == version)
    };
    if !wait_for(5, offered) {
        // the net gap of `keeps`: the machines reconnect and sync
        reconnect(&w.server, &w.laptop);
        std::thread::sleep(std::time::Duration::from_secs(2));
        reconnect(&w.light, &w.server);
        assert!(wait_for(12, offered), "the light home offers 0.2.0");
    }
}
