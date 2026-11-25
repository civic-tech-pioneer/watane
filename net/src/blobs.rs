//! Packages over iroh-blobs (TECHNICAL 5.8). Serve: the store in
//! `<home>/net/blobs/` holds each package by reference to the holder's file,
//! added when a trusted machine asks for its hash and the holder gives a
//! path. Fetch: download by hash, verified while it streams, to a temp file,
//! check again, hand to the holder, remove the temp file.

use crate::key::NodeKey;
use crate::{BlobId, Core, NetError, client, sync};
use iroh_blobs::api::Store;
use iroh_blobs::api::blobs::{AddPathOptions, ImportMode};
use iroh_blobs::get::request::{get_blob, get_verified_size};
use iroh_blobs::get::{self};
use iroh_blobs::provider::events::{
    AbortReason, EventMask, EventSender, ProviderMessage, RequestMode,
};
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::{BlobFormat, Hash};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Open the store of `dir` and the hook that decides each request.
pub(crate) async fn open(
    core: Arc<Core>,
    dir: &Path,
) -> Result<(FsStore, EventSender, tokio::task::JoinHandle<()>), String> {
    #[cfg(windows)]
    let _directory = watane_log_windows_fs::private_dir(dir).map_err(|error| error.to_string())?;
    let store = FsStore::load(dir).await.map_err(|e| e.to_string())?;
    #[cfg(unix)]
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let mask = EventMask {
        get: RequestMode::Intercept,
        get_many: RequestMode::Intercept,
        ..EventMask::DEFAULT
    };
    let (events, mut rx) = EventSender::channel(32, mask);
    let api: Store = store.clone().into();
    let task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let (core, api) = (Arc::clone(&core), api.clone());
            tokio::spawn(async move {
                match message {
                    ProviderMessage::GetRequestReceived(msg) => {
                        let ok = admit(&core, &api, &[msg.inner.request.hash]).await;
                        // The requester may have gone; there is no one left to tell.
                        let _ = msg.tx.send(verdict(ok)).await;
                    }
                    ProviderMessage::GetManyRequestReceived(msg) => {
                        let ok = admit(&core, &api, &msg.inner.request.hashes).await;
                        // The requester may have gone; there is no one left to tell.
                        let _ = msg.tx.send(verdict(ok)).await;
                    }
                    _ => {}
                }
            });
        }
    });
    Ok((store, events, task))
}

fn verdict(ok: bool) -> Result<(), AbortReason> {
    if ok {
        Ok(())
    } else {
        Err(AbortReason::Permission)
    }
}

fn tag_of(hash: &Hash) -> String {
    format!("package-{}", hash.to_hex())
}

/// Serve these hashes only when the holder gives a file for each: add it by
/// reference. A hash the holder no longer gives loses its reference.
async fn admit(core: &Arc<Core>, store: &Store, hashes: &[Hash]) -> bool {
    for hash in hashes {
        let id = BlobId(*hash.as_bytes());
        let path = sync::holder(core, move |h| h.blob(&id))
            .await
            .ok()
            .flatten();
        let Some(path) = path else {
            let _ = store.tags().delete(tag_of(hash)).await;
            return false;
        };
        if store.blobs().has(*hash).await.unwrap_or(false) {
            continue;
        }
        let added = store
            .blobs()
            .add_path_with_opts(AddPathOptions {
                path,
                format: BlobFormat::Raw,
                mode: ImportMode::TryReference,
            })
            .with_named_tag(tag_of(hash))
            .await;
        match added {
            Ok(t) if t.hash == *hash => {}
            Ok(_) => {
                // the holder's file is not the package it was asked for
                let _ = store.tags().delete(tag_of(hash)).await;
                return false;
            }
            Err(_) => return false,
        }
    }
    true
}

static TEMPS: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn bad(node: &NodeKey, cause: impl Into<String>) -> NetError {
    NetError::BadObject {
        node: *node,
        cause: cause.into(),
    }
}

/// Why a fetch failed: no connection to the machine, or a connection that
/// did not give the package.
pub(crate) enum Failed {
    NoConnection(NetError),
    NotFetched(NetError),
}

impl Failed {
    pub(crate) fn error(self) -> NetError {
        match self {
            Failed::NoConnection(e) | Failed::NotFetched(e) => e,
        }
    }
}

/// Fetch one wanted package from `node` and hand it to the holder.
pub(crate) async fn fetch(
    core: &Arc<Core>,
    node: &NodeKey,
    id: BlobId,
    size: u64,
) -> Result<(), NetError> {
    fetch_via(core, node, id, size).await.map_err(Failed::error)
}

pub(crate) async fn fetch_via(
    core: &Arc<Core>,
    node: &NodeKey,
    id: BlobId,
    size: u64,
) -> Result<(), Failed> {
    let limits = core.limits();
    if size > limits.blob {
        return Err(Failed::NotFetched(NetError::TooLarge { node: *node }));
    }
    let work = fetch_inner(core, node, id, size);
    tokio::time::timeout(limits.blob_deadline, work)
        .await
        .map_err(|_| Failed::NotFetched(NetError::Timeout { node: *node }))?
}

/// The first four bytes of a package's hash, in hex.
pub(crate) fn hex8(id: &BlobId) -> String {
    id.0[..4].iter().map(|b| format!("{b:02x}")).collect()
}

fn mismatch(node: &NodeKey, id: &BlobId) -> NetError {
    bad(
        node,
        format!(
            "package {} from {} does not match its hash",
            hex8(id),
            node.short()
        ),
    )
}

fn failed(node: &NodeKey, id: &BlobId, e: get::GetError) -> NetError {
    match e {
        get::GetError::Decode { .. } => mismatch(node, id),
        // the machine closed the connection as "not allowed" (allow.rs)
        e if closed_as_not_allowed(&e) => NetError::Refused {
            node: *node,
            code: crate::Code::NotAllowed,
        },
        // the machine answered and gave no package
        e if e.iroh_error_code().is_some() => NetError::NotServed {
            node: *node,
            cause: format!(
                "package {}: it does not have it, or its copy failed its check",
                hex8(id)
            ),
        },
        _ => NetError::Unreachable {
            node: *node,
            cause: "could not be read: the connection failed".to_owned(),
        },
    }
}

/// Whether the machine closed the connection as "not allowed" (allow.rs): a
/// 403 close anywhere under `e`, also inside the `io::Error` that iroh-blobs
/// wraps it in.
fn closed_as_not_allowed(e: &(dyn std::error::Error + 'static)) -> bool {
    let mut next = Some(e);
    while let Some(err) = next {
        if let Some(iroh::endpoint::ConnectionError::ApplicationClosed(close)) =
            err.downcast_ref::<iroh::endpoint::ConnectionError>()
            && u64::from(close.error_code) == 403
        {
            return true;
        }
        if let Some(inner) = err
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            && closed_as_not_allowed(inner)
        {
            return true;
        }
        next = err.source();
    }
    false
}

async fn fetch_inner(
    core: &Arc<Core>,
    node: &NodeKey,
    id: BlobId,
    size: u64,
) -> Result<(), Failed> {
    let hash = Hash::from_bytes(id.0);
    let conn = client::connect(core, node, iroh_blobs::ALPN)
        .await
        .map_err(Failed::NoConnection)?;
    let result = download(core, node, &conn, hash, id, size)
        .await
        .map_err(Failed::NotFetched);
    conn.close(0u32.into(), b"done");
    result
}

async fn download(
    core: &Arc<Core>,
    node: &NodeKey,
    conn: &iroh::endpoint::Connection,
    hash: Hash,
    id: BlobId,
    size: u64,
) -> Result<(), NetError> {
    let (claimed, _) = get_verified_size(conn, &hash)
        .await
        .map_err(|e| failed(node, &id, e))?;
    if claimed != size {
        return Err(bad(
            node,
            format!("the package is {claimed} bytes, and {size} were wanted"),
        ));
    }
    let path = core.dir.join(format!(
        "fetch-{}-{}.tmp",
        std::process::id(),
        TEMPS.fetch_add(1, Ordering::SeqCst)
    ));
    let temp = Temp(path.clone());
    // a local failure names this machine's step, never the temp file
    let io = |step: &'static str| {
        let (dir, hex) = (core.dir.clone(), hex8(&id));
        move |e: std::io::Error| NetError::Io {
            path: dir.clone(),
            cause: format!("package {hex} was not stored on this machine: {step}: {e}"),
        }
    };
    #[cfg(unix)]
    let create = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path);
    #[cfg(windows)]
    let create = watane_log_windows_fs::create_file(&path);
    let mut file = create.map_err(io("could not create its temp file"))?;
    // the size is verified above, so this holds at most `size` bytes
    let bytes = get_blob(conn.clone(), hash)
        .bytes()
        .await
        .map_err(|e| failed(node, &id, e))?;
    if bytes.len() as u64 != size {
        return Err(bad(node, "the package is not the size that was wanted"));
    }
    file.write_all(&bytes)
        .map_err(io("could not write its temp file"))?;
    file.sync_all()
        .map_err(io("could not sync its temp file"))?;
    drop(file);
    // checked again here, whatever iroh-blobs said
    let (p, expected) = (path.clone(), id.0);
    let ok = tokio::task::spawn_blocking(move || {
        let mut hasher = blake3::Hasher::new();
        let file = std::fs::File::open(&p)?;
        let n = std::io::copy(&mut std::io::BufReader::new(file), &mut hasher)?;
        Ok::<_, std::io::Error>((n, *hasher.finalize().as_bytes() == expected))
    })
    .await
    .map_err(|_| NetError::Io {
        path: core.dir.clone(),
        cause: format!(
            "package {} was not stored on this machine: the check did not finish",
            hex8(&id)
        ),
    })?
    .map_err(io("could not read its temp file"))?;
    if ok.0 != size || !ok.1 {
        return Err(mismatch(node, &id));
    }
    let store_path = path.clone();
    let stored = sync::holder(core, move |h| h.store_blob(&id, &store_path)).await?;
    drop(temp);
    stored.map_err(|cause| NetError::Io {
        path: core.dir.clone(),
        cause: format!(
            "package {} was not stored on this machine: {cause}",
            hex8(&id)
        ),
    })
}
