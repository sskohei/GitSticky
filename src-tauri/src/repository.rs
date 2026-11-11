//! All managed changes use a single Git tree/commit and a non-forced ref update.
//! The commit's sole parent is the observed head; concurrent branch updates fail.
use crate::{
    auth::Credentials,
    crypto::{decrypt_sync, encrypt_sync, NoteCipher, SyncKeys},
    errors::{AppError, Result},
    github::{private_repository, read_blob, request, url, value},
    models::Settings,
    sync_security::{
        integrity_error, verify_checkpoint, Checkpoint, Inventory, MANIFEST_PATH, MAX_NOTES,
    },
};
use reqwest::Method;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tauri::State;
use zeroize::Zeroizing;
#[cfg(test)]
#[path = "repository_tests.rs"]
mod tests;

pub struct Snapshot {
    head: String,
    tree: String,
    pub files: BTreeMap<String, String>,
    manifest_sha: Option<String>,
}
#[derive(Deserialize)]
struct Tree {
    tree: Vec<Entry>,
    truncated: bool,
}
#[derive(Deserialize)]
struct Entry {
    path: String,
    sha: String,
    mode: String,
    #[serde(rename = "type")]
    kind: String,
}
fn sha(value: &Value, pointer: &str) -> Result<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|sha| crate::sync_security::valid_sha(sha))
        .map(str::to_owned)
        .ok_or_else(integrity_error)
}
pub async fn raw_snapshot(settings: &Settings, credentials: &Credentials) -> Result<Snapshot> {
    let branch = value(
        request(
            credentials,
            Method::GET,
            url(settings, &["branches", &settings.branch_name])?,
            None,
            "branch",
        )
        .await?,
    )
    .await?;
    let head = sha(&branch, "/commit/sha")?;
    let tree_sha = sha(&branch, "/commit/commit/tree/sha")?;
    let mut tree_url = url(settings, &["git", "trees", &tree_sha])?;
    tree_url.query_pairs_mut().append_pair("recursive", "1");
    let tree: Tree = serde_json::from_value(
        value(request(credentials, Method::GET, tree_url, None, "repository").await?).await?,
    )
    .map_err(|_| integrity_error())?;
    if tree.truncated {
        return Err(integrity_error());
    }
    let mut files = BTreeMap::new();
    let mut manifest_sha = None;
    for entry in tree.tree {
        let id = crate::github::managed_id(&entry.path);
        if id.is_some() || entry.path == MANIFEST_PATH {
            if entry.kind != "blob"
                || !["100644", "100755"].contains(&entry.mode.as_str())
                || !crate::sync_security::valid_sha(&entry.sha)
            {
                return Err(integrity_error());
            }
            if let Some(id) = id {
                if files.insert(id, entry.sha).is_some() {
                    return Err(integrity_error());
                }
            } else {
                manifest_sha = Some(entry.sha);
            }
        }
    }
    if files.len() > MAX_NOTES {
        return Err(AppError::new(
            "validation",
            "同期するメモは10,000件以下にしてください。",
        ));
    }
    Ok(Snapshot {
        head,
        tree: tree_sha,
        files,
        manifest_sha,
    })
}
async fn inventory(
    snapshot: &Snapshot,
    settings: &Settings,
    key: &NoteCipher,
    credentials: &Credentials,
    known: Option<&Checkpoint>,
) -> Result<(Inventory, Option<Checkpoint>)> {
    match &snapshot.manifest_sha {
        Some(sha) => {
            let content = read_blob(settings, sha, credentials).await?;
            let inventory = Inventory::open(&content, key, settings)?;
            inventory.validate(&snapshot.files)?;
            let checkpoint = Checkpoint {
                generation: inventory.generation,
                sha: sha.clone(),
            };
            verify_checkpoint(known, Some(&checkpoint))?;
            Ok((inventory, Some(checkpoint)))
        }
        None => {
            verify_checkpoint(known, None)?;
            Ok((
                Inventory {
                    generation: 0,
                    notes: snapshot.files.clone(),
                },
                None,
            ))
        }
    }
}
pub async fn validate_candidate(
    settings: &Settings,
    key: &NoteCipher,
    credentials: &Credentials,
    known: Option<Checkpoint>,
) -> Result<Option<Checkpoint>> {
    let snapshot = raw_snapshot(settings, credentials).await?;
    Ok(
        inventory(&snapshot, settings, key, credentials, known.as_ref())
            .await?
            .1,
    )
}
async fn protected_snapshot(
    settings: &Settings,
    keys: &SyncKeys,
    credentials: &Credentials,
) -> Result<(Snapshot, Inventory)> {
    private_repository(settings, credentials).await?;
    let key = keys.key(settings).await?;
    let snapshot = raw_snapshot(settings, credentials).await?;
    let known = keys.checkpoint(settings).await?;
    let (mut inventory, checkpoint) =
        inventory(&snapshot, settings, &key, credentials, known.as_ref()).await?;
    if let Some(checkpoint) = checkpoint {
        keys.observe(settings, checkpoint).await?;
        return Ok((snapshot, inventory));
    }
    // Establish an inventory once, only after every existing note authenticates.
    // No local or remote note is changed by this first-trust operation.
    for (id, sha) in &snapshot.files {
        let content = read_blob(settings, sha, credentials).await?;
        keys.decrypt(settings, id, &content).await?;
    }
    inventory.advance()?;
    commit(
        settings,
        credentials,
        keys,
        &snapshot,
        &inventory,
        &key,
        Vec::new(),
        "StickyNote: protect inventory",
    )
    .await?;
    // Re-fetch to bind subsequent reads to the committed inventory.
    let snapshot = raw_snapshot(settings, credentials).await?;
    let (inventory, checkpoint) = self::inventory(
        &snapshot,
        settings,
        &key,
        credentials,
        keys.checkpoint(settings).await?.as_ref(),
    )
    .await?;
    keys.observe(settings, checkpoint.ok_or_else(integrity_error)?)
        .await?;
    Ok((snapshot, inventory))
}
async fn upload(settings: &Settings, credentials: &Credentials, content: &str) -> Result<String> {
    let result = value(
        request(
            credentials,
            Method::POST,
            url(settings, &["git", "blobs"])?,
            Some(json!({"content":content,"encoding":"utf-8"})),
            "repository",
        )
        .await?,
    )
    .await?;
    sha(&result, "/sha")
}
fn change(path: &str, sha: Option<&str>) -> Value {
    json!({"path":path,"mode":"100644","type":"blob","sha":sha})
}
#[allow(clippy::too_many_arguments)]
async fn commit(
    settings: &Settings,
    credentials: &Credentials,
    keys: &SyncKeys,
    snapshot: &Snapshot,
    inventory: &Inventory,
    key: &NoteCipher,
    mut changes: Vec<Value>,
    message: &str,
) -> Result<()> {
    let manifest = inventory.seal(key, settings)?;
    let manifest_sha = upload(settings, credentials, &manifest).await?;
    changes.push(change(MANIFEST_PATH, Some(&manifest_sha)));
    let tree = value(
        request(
            credentials,
            Method::POST,
            url(settings, &["git", "trees"])?,
            Some(json!({"base_tree":snapshot.tree,"tree":changes})),
            "repository",
        )
        .await?,
    )
    .await?;
    let tree_sha = sha(&tree, "/sha")?;
    let commit = value(
        request(
            credentials,
            Method::POST,
            url(settings, &["git", "commits"])?,
            Some(json!({"message":message,"tree":tree_sha,"parents":[snapshot.head]})),
            "repository",
        )
        .await?,
    )
    .await?;
    let commit_sha = sha(&commit, "/sha")?;
    // Recheck privacy immediately before publishing the prepared commit.
    private_repository(settings, credentials).await?;
    request(
        credentials,
        Method::PATCH,
        url(settings, &["git", "refs", "heads", &settings.branch_name])?,
        Some(json!({"sha":commit_sha,"force":false})),
        "repository",
    )
    .await?;
    // A crash/storage failure here is safe: the next read accepts a newer manifest.
    keys.observe(
        settings,
        Checkpoint {
            generation: inventory.generation,
            sha: manifest_sha,
        },
    )
    .await
}
pub async fn list(
    settings: &Settings,
    credentials: &Credentials,
    keys: &SyncKeys,
) -> Result<Vec<crate::github::RemoteFile>> {
    let _operation = keys.operation.lock().await;
    let (snapshot, _) = protected_snapshot(settings, keys, credentials).await?;
    Ok(snapshot
        .files
        .into_iter()
        .map(|(id, sha)| crate::github::RemoteFile { id, sha })
        .collect())
}
pub async fn read(
    settings: &Settings,
    id: &str,
    sha: &str,
    credentials: &Credentials,
    keys: &SyncKeys,
) -> Result<crate::github::ReadNote> {
    let _operation = keys.operation.lock().await;
    let (snapshot, _) = protected_snapshot(settings, keys, credentials).await?;
    if snapshot.files.get(id).map(String::as_str) != Some(sha) {
        return Err(AppError::new(
            "conflict",
            "同期先が更新されました。再度同期してください。",
        ));
    }
    let content = read_blob(settings, sha, credentials).await?;
    let (content, encrypted) = keys.decrypt(settings, id, &content).await?;
    Ok(crate::github::ReadNote { content, encrypted })
}
pub async fn write(
    settings: &Settings,
    id: &str,
    content: Option<&str>,
    expected: Option<&str>,
    credentials: &Credentials,
    keys: &SyncKeys,
) -> Result<Option<String>> {
    let _operation = keys.operation.lock().await;
    crate::github::note_url(settings, id)?;
    let (snapshot, mut inventory) = protected_snapshot(settings, keys, credentials).await?;
    if snapshot.files.get(id).map(String::as_str) != expected {
        return Err(AppError::new(
            "conflict",
            "同期先が更新されました。再取得してください。",
        ));
    }
    let key = keys.key(settings).await?;
    let new_sha = match content {
        Some(content) => {
            let encrypted = encrypt_sync(&key, settings, id, content)?;
            let sha = upload(settings, credentials, &encrypted).await?;
            inventory.notes.insert(id.into(), sha.clone());
            Some(sha)
        }
        None => {
            inventory.notes.remove(id);
            None
        }
    };
    if inventory.notes.len() > MAX_NOTES {
        return Err(AppError::new(
            "validation",
            "メモの件数が上限に達しました。",
        ));
    }
    inventory.advance()?;
    let changes = vec![change(&format!("notes/{id}.md"), new_sha.as_deref())];
    commit(
        settings,
        credentials,
        keys,
        &snapshot,
        &inventory,
        &key,
        changes,
        "StickyNote: update notes",
    )
    .await?;
    Ok(new_sha)
}

#[tauri::command]
pub async fn security_rotate_sync_key(
    settings: Settings,
    key: Option<String>,
    keys: State<'_, SyncKeys>,
    credentials: State<'_, Credentials>,
) -> Result<()> {
    rotate(&settings, key, &keys, &credentials).await
}
pub async fn rotate(
    settings: &Settings,
    key: Option<String>,
    keys: &SyncKeys,
    credentials: &Credentials,
) -> Result<()> {
    let _operation = keys.operation.lock().await;
    private_repository(settings, credentials).await?;
    let (old, pending) = keys.rotation_keys(settings).await?;
    let candidate = match (pending, key.map(Zeroizing::new)) {
        (Some(pending), None) => pending,
        (Some(pending), Some(key)) if pending.encoded().as_str() == key.trim() => pending,
        (Some(_), Some(_)) => {
            return Err(AppError::new(
                "locked",
                "交換中の鍵が異なります。入力を消して交換を再開してください。",
            ))
        }
        (None, Some(key)) => NoteCipher::from_encoded(&key)?,
        (None, None) => {
            return Err(AppError::new(
                "validation",
                "交換する新しい鍵を入力してください。",
            ))
        }
    };
    let snapshot = raw_snapshot(settings, credentials).await?;
    let known = keys.checkpoint(settings).await?;
    // An ambiguous ref response or a crash after the atomic switch resumes here.
    if let Ok((_, Some(checkpoint))) =
        inventory(&snapshot, settings, &candidate, credentials, known.as_ref()).await
    {
        // The candidate is already the live key; every managed file must match it.
        for (id, sha) in &snapshot.files {
            let content = read_blob(settings, sha, credentials).await?;
            if !decrypt_sync(&candidate, settings, id, &content)?.1 {
                return Err(integrity_error());
            }
        }
        keys.begin_rotation(settings, &candidate).await?;
        keys.observe(settings, checkpoint).await?;
        return keys.complete_rotation(settings).await;
    }
    let (mut inventory, _) =
        inventory(&snapshot, settings, &old, credentials, known.as_ref()).await?;
    let mut notes = Vec::new();
    let mut total = 0usize;
    for (id, sha) in &snapshot.files {
        let content = read_blob(settings, sha, credentials).await?;
        let (plaintext, encrypted) = decrypt_sync(&old, settings, id, &content)?;
        if !encrypted {
            return Err(AppError::new(
                "locked",
                "先に通常の同期を完了し、すべてのメモを暗号化してください。",
            ));
        }
        total = total
            .checked_add(plaintext.len())
            .ok_or_else(integrity_error)?;
        if total > 64_000_000 {
            return Err(AppError::new(
                "validation",
                "一括鍵交換の上限（本文合計64MB）を超えています。",
            ));
        }
        notes.push((id.clone(), Zeroizing::new(plaintext)));
    }
    // Persist both keys before uploading anything. A failed save leaves Git untouched.
    keys.begin_rotation(settings, &candidate).await?;
    let mut changes = Vec::new();
    for (id, content) in notes {
        let encrypted = encrypt_sync(&candidate, settings, &id, &content)?;
        let sha = upload(settings, credentials, &encrypted).await?;
        inventory.notes.insert(id.clone(), sha.clone());
        changes.push(change(&format!("notes/{id}.md"), Some(&sha)));
    }
    inventory.advance()?;
    commit(
        settings,
        credentials,
        keys,
        &snapshot,
        &inventory,
        &candidate,
        changes,
        "StickyNote: rotate encryption key",
    )
    .await?;
    keys.complete_rotation(settings).await
}
