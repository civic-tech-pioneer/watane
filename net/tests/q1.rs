//! Net Q1, against the pinned releases (iroh 1.4.1, iroh-blobs 0.105.0,
//! iroh-gossip 0.102.0): a file added by reference leaves no second copy, and
//! a wrapper around a protocol handler admits a connection by its remote key.
#![allow(clippy::unwrap_used, reason = "tests may panic")]

mod support;
use iroh::endpoint::Connection;
use iroh::protocol::{AcceptError, ProtocolHandler, Router};
use iroh::{Endpoint, EndpointAddr, EndpointId, TransportAddr};
use iroh_blobs::api::blobs::{AddPathOptions, ImportMode};
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::{BlobFormat, BlobsProtocol};
use iroh_gossip::net::Gossip;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn dir_size(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let meta = e.metadata().unwrap();
            if meta.is_dir() {
                dir_size(&e.path())
            } else {
                meta.len()
            }
        })
        .sum()
}

fn file_1mib() -> Vec<u8> {
    (0..1 << 20)
        .map(|i: u32| (i.wrapping_mul(2654435761) >> 13) as u8)
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn q1a_blobs_by_reference_keep_no_second_copy() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("package.bin");
    let data = file_1mib();
    std::fs::write(&file, &data).unwrap();
    let store_dir = dir.path().join("blobs");
    let store = FsStore::load(&store_dir).await.unwrap();
    // redb preallocates the database: measure the growth, not the total
    let empty = dir_size(&store_dir);
    let tag = store
        .blobs()
        .add_path_with_opts(AddPathOptions {
            path: file.clone(),
            format: BlobFormat::Raw,
            mode: ImportMode::TryReference,
        })
        .await
        .unwrap();
    // the hash is the plain BLAKE3 hash of the file
    assert_eq!(tag.hash.as_bytes(), blake3::hash(&data).as_bytes());
    // the bytes read back equal the file
    let back = store.blobs().get_bytes(tag.hash).await.unwrap();
    assert_eq!(&*back, &*data);
    // and the store keeps only the BLAKE3 tree, not a second copy
    store.shutdown().await.unwrap();
    let size = dir_size(&store_dir);
    assert!(
        size < empty + 64 * 1024,
        "the store grew from {empty} to {size} bytes for a 1 MiB file"
    );
    // and no data file beside the database
    assert!(dir_size(&store_dir.join("data")) < 64 * 1024);

    // the control: the same file added with `Copy` does make a second copy
    let copy_dir = dir.path().join("copy");
    let copy = FsStore::load(&copy_dir).await.unwrap();
    copy.blobs().add_path(&file).await.unwrap();
    copy.shutdown().await.unwrap();
    assert!(dir_size(&copy_dir.join("data")) >= data.len() as u64);
    assert!(file.exists());
}

/// A wrapper that admits a connection only from an allowed remote key.
#[derive(Debug)]
struct Gate<P> {
    inner: P,
    allowed: HashSet<EndpointId>,
    passed: Arc<AtomicUsize>,
}

impl<P: ProtocolHandler> ProtocolHandler for Gate<P> {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        if !self.allowed.contains(&connection.remote_id()) {
            connection.close(403u32.into(), b"not allowed");
            return Ok(());
        }
        self.passed.fetch_add(1, Ordering::SeqCst);
        self.inner.accept(connection).await
    }
    async fn shutdown(&self) {
        self.inner.shutdown().await
    }
}

async fn endpoint(seed: u8) -> Endpoint {
    support::builder([seed; 32]).bind().await.unwrap()
}

fn direct(ep: &Endpoint) -> EndpointAddr {
    EndpointAddr::from_parts(
        ep.id(),
        ep.bound_sockets().into_iter().map(TransportAddr::Ip),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn q1b_a_wrapper_admits_a_connection_by_its_remote_key() {
    let dir = tempfile::tempdir().unwrap();
    let data = file_1mib();
    let file = dir.path().join("package.bin");
    std::fs::write(&file, &data).unwrap();
    let store = FsStore::load(dir.path().join("blobs")).await.unwrap();
    let hash = store.blobs().add_slice(&data).await.unwrap().hash;

    let (server, allowed, refused) = (endpoint(11).await, endpoint(12).await, endpoint(13).await);
    let passed = Arc::new(AtomicUsize::new(0));
    fn gate_of<P>(inner: P, allowed: EndpointId, passed: &Arc<AtomicUsize>) -> Gate<P> {
        Gate {
            inner,
            allowed: HashSet::from([allowed]),
            passed: Arc::clone(passed),
        }
    }
    // the same wrapper type around Gossip compiles, and sits on the same router
    let gossip = Gossip::builder().spawn(server.clone());
    let gossip_gate: Gate<Gossip> = gate_of(gossip, allowed.id(), &passed);
    let router = Router::builder(server.clone())
        .accept(
            iroh_blobs::ALPN,
            gate_of(BlobsProtocol::new(&store, None), allowed.id(), &passed),
        )
        .accept(iroh_gossip::ALPN, gossip_gate)
        .spawn();

    // an allowed node downloads the blob
    let conn = allowed
        .connect(direct(&server), iroh_blobs::ALPN)
        .await
        .unwrap();
    let got = iroh_blobs::get::request::get_blob(conn, hash)
        .await
        .unwrap();
    assert_eq!(&*got, &*data);
    assert_eq!(passed.load(Ordering::SeqCst), 1);

    // a refused node's download fails, and the inner handler never sees it
    let conn = refused
        .connect(direct(&server), iroh_blobs::ALPN)
        .await
        .unwrap();
    let result = iroh_blobs::get::request::get_blob(conn, hash).await;
    assert!(result.is_err(), "a refused node downloaded the blob");
    assert_eq!(passed.load(Ordering::SeqCst), 1);

    router.shutdown().await.unwrap();
}
