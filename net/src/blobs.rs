//! Packages over iroh-blobs (TECHNICAL 5.8). Serve: the store in
//! `<home>/net/blobs/` holds each package by reference to the holder's file,
//! added when a trusted machine asks for its hash and the holder gives a
//! path. Fetch: download by hash, verified while it streams, to a temp file,
//! check again, hand to the holder, remove the temp file.

use crate::{BlobId, Core, NetError, client, key::NodeKey, sync};
use iroh_blobs::{
    BlobFormat, Hash,
    api::{
        Store,
        blobs::{AddPathOptions, ImportMode},
    },
    get::{
        self,
        request::{get_blob, get_verified_size},
    },
    provider::events::{AbortReason, EventMask, EventSender, ProviderMessage, RequestMode},
    store::fs::FsStore,
};
use std::{
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// Open the store of `dir` and the hook that decides each request.
pub(crate) async fn open(
    core: Arc<Core>,
    dir: &Path,
) -> Result<(FsStore, EventSender, tokio::task::JoinHandle<()>), String> {
    let store = FsStore::load(dir).await.map_err(|e| e.to_string())?;
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
            let (core, api) = (core.clone(), api.clone());
            tokio::spawn(async move {
                match message {
                    ProviderMessage::GetRequestReceived(msg) => {
                        let ok = admit(&core, &api, &[msg.inner.request.hash]).await;
                        msg.tx.send(verdict(ok)).await.ok();
                    }
                    ProviderMessage::GetManyRequestReceived(msg) => {
                        let ok = admit(&core, &api, &msg.inner.request.hashes).await;
                        msg.tx.send(verdict(ok)).await.ok();
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

/// Fetch one wanted package from `node` and hand it to the holder.
pub(crate) async fn fetch(
    core: &Arc<Core>,
    node: &NodeKey,
    id: BlobId,
    size: u64,
) -> Result<(), NetError> {
    let limits = core.limits();
    if size > limits.blob {
        return Ok(());
    }
    let work = fetch_inner(core, node, id, size);
    tokio::time::timeout(limits.blob_deadline, work)
        .await
        .map_err(|_| NetError::Timeout { node: *node })?
}

fn failed(node: &NodeKey, e: get::GetError) -> NetError {
    match e {
        get::GetError::Decode { .. } => bad(node, format!("the package does not verify: {e}")),
        e => NetError::Unreachable {
            node: *node,
            cause: e.to_string(),
        },
    }
}

async fn fetch_inner(
    core: &Arc<Core>,
    node: &NodeKey,
    id: BlobId,
    size: u64,
) -> Result<(), NetError> {
    let hash = Hash::from_bytes(id.0);
    let conn = client::connect(core, node, iroh_blobs::ALPN).await?;
    let result = download(core, node, &conn, hash, id, size).await;
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
        .map_err(|e| failed(node, e))?;
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
    let io = |e: std::io::Error| NetError::Io {
        path: path.clone(),
        cause: e.to_string(),
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(io)?;
    // the size is verified above, so this holds at most `size` bytes
    let bytes = get_blob(conn.clone(), hash)
        .bytes()
        .await
        .map_err(|e| failed(node, e))?;
    if bytes.len() as u64 != size {
        return Err(bad(node, "the package is not the size that was wanted"));
    }
    file.write_all(&bytes).map_err(io)?;
    file.sync_all().map_err(io)?;
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
    .map_err(|_| bad(node, "the check failed"))?
    .map_err(io)?;
    if ok.0 != size || !ok.1 {
        return Err(bad(node, "the package does not match its hash and size"));
    }
    let stored = sync::holder(core, move |h| h.store_blob(&id, &path)).await?;
    drop(temp);
    // a holder that cannot keep it is its own problem; the next sync asks again
    let _ = stored;
    Ok(())
}
