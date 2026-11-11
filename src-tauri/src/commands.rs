use crate::{
    errors::{AppError, Result},
    models::*,
};
use sqlx::{Sqlite, SqlitePool, Transaction};
use tauri::{AppHandle, Manager};
async fn database(app: &AppHandle) -> Result<crate::database::EncryptedDatabase> {
    let state = app.state::<crate::database::Database>();
    Ok(state.get(app).await?.clone())
}
fn validate(n: &Note) -> Result<()> {
    if uuid::Uuid::parse_str(&n.id).is_err()
        || n.title.trim().is_empty()
        || n.title.chars().count() > 80
        || !["yellow", "pink", "green", "blue", "purple", "gray"].contains(&n.color.as_str())
        || n.tags.len() > 20
        || n.tags.iter().enumerate().any(|(index, tag)| {
            tag.is_empty()
                || tag.trim() != tag
                || tag.chars().count() > 30
                || tag.chars().any(char::is_control)
                || n.tags[..index]
                    .iter()
                    .any(|previous| previous.eq_ignore_ascii_case(tag))
        })
    {
        return Err(AppError::new("validation", "メモの形式が不正です。"));
    }
    Ok(())
}
async fn put(
    tx: &mut Transaction<'_, Sqlite>,
    note: &Note,
    cipher: &crate::crypto::NoteCipher,
) -> Result<()> {
    validate(note)?;
    let encrypted = crate::database::encode_note(cipher, note)?;
    let n = &encrypted;
    let tags = serde_json::to_string(&n.tags)
        .map_err(|_| AppError::new("validation", "タグの形式が不正です。"))?;
    sqlx::query("INSERT INTO notes (id,title,body_md,color,pinned,created_at,updated_at,deleted_at,local_revision,dirty,tags,encrypted) VALUES(?,?,?,?,?,?,?,?,?,?,?,1) ON CONFLICT(id) DO UPDATE SET title=excluded.title,body_md=excluded.body_md,color=excluded.color,pinned=excluded.pinned,created_at=excluded.created_at,updated_at=excluded.updated_at,deleted_at=excluded.deleted_at,local_revision=excluded.local_revision,dirty=excluded.dirty,tags=excluded.tags,encrypted=1")
 .bind(&n.id).bind(&n.title).bind(&n.body_md).bind(&n.color).bind(n.pinned).bind(&n.created_at).bind(&n.updated_at).bind(&n.deleted_at).bind(n.local_revision).bind(n.dirty).bind(tags).execute(&mut **tx).await?;
    Ok(())
}
async fn put_state(tx: &mut Transaction<'_, Sqlite>, s: &SyncState) -> Result<()> {
    sqlx::query("INSERT INTO sync_states VALUES(?,?,?,?,?,?) ON CONFLICT(note_id,connection_id) DO UPDATE SET last_synced_sha=excluded.last_synced_sha,last_synced_revision=excluded.last_synced_revision,last_synced_at=excluded.last_synced_at,sync_error=excluded.sync_error")
 .bind(&s.note_id).bind(&s.connection_id).bind(&s.last_synced_sha).bind(s.last_synced_revision).bind(&s.last_synced_at).bind(&s.sync_error).execute(&mut **tx).await?;
    Ok(())
}
#[tauri::command]
pub async fn db_snapshot(app: AppHandle) -> Result<Snapshot> {
    let db = database(&app).await?;
    let p = &db.pool;
    let mut tx = p.begin().await?;
    let notes = sqlx::query_as::<_, Note>("SELECT id,title,body_md,color,pinned,tags,created_at,updated_at,deleted_at,local_revision,dirty FROM notes")
        .fetch_all(&mut *tx)
        .await?
        .into_iter().map(|note| crate::database::decode_note(&db.cipher, note)).collect::<Result<Vec<_>>>()?;
    let states = sqlx::query_as::<_, SyncState>("SELECT * FROM sync_states")
        .fetch_all(&mut *tx)
        .await?;
    let raw: Option<(String,)> = sqlx::query_as("SELECT value FROM app_settings WHERE id=1")
        .fetch_optional(&mut *tx)
        .await?;
    let settings = match raw {
        Some((v,)) => serde_json::from_str(&v)
            .map_err(|_| AppError::new("database", "設定を読み取れません。"))?,
        None => Settings::default(),
    };
    tx.commit().await?;
    Ok(Snapshot {
        notes,
        states,
        settings,
    })
}
#[tauri::command]
pub async fn db_save(app: AppHandle, mut note: Note) -> Result<()> {
    let db = database(&app).await?;
    let p = &db.pool;
    let mut tx = p.begin().await?;
    sqlx::query("UPDATE notes SET dirty=dirty WHERE id=?")
        .bind(&note.id)
        .execute(&mut *tx)
        .await?;
    let rev: Option<(i64,)> = sqlx::query_as("SELECT local_revision FROM notes WHERE id=?")
        .bind(&note.id)
        .fetch_optional(&mut *tx)
        .await?;
    // Editor changes are local intent; always advance from the persisted revision.
    note.local_revision = rev.map_or(1, |r| r.0 + 1);
    note.dirty = true;
    put(&mut tx, &note, &db.cipher).await?;
    tx.commit().await?;
    Ok(())
}
#[tauri::command]
pub async fn db_settings(app: AppHandle, settings: Settings) -> Result<()> {
    let raw = serde_json::to_string(&settings)
        .map_err(|_| AppError::new("validation", "設定が不正です。"))?;
    let db = database(&app).await?;
    let p = &db.pool;
    let mut tx = p.begin().await?;
    let previous: Option<(String,)> = sqlx::query_as("SELECT value FROM app_settings WHERE id=1")
        .fetch_optional(&mut *tx)
        .await?;
    if let Some((previous,)) = previous {
        let previous: Settings = serde_json::from_str(&previous)
            .map_err(|_| AppError::new("database", "設定を読み取れません。"))?;
        if previous.repository_owner.to_lowercase() != settings.repository_owner.to_lowercase()
            || previous.repository_name.to_lowercase() != settings.repository_name.to_lowercase()
            || previous.branch_name != settings.branch_name
        {
            sqlx::query("UPDATE notes SET dirty=1")
                .execute(&mut *tx)
                .await?;
        }
    }
    sqlx::query(
        "INSERT INTO app_settings VALUES(1,?) ON CONFLICT(id) DO UPDATE SET value=excluded.value",
    )
    .bind(raw)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
#[tauri::command]
pub async fn db_apply(
    app: AppHandle,
    id: String,
    revision: Option<i64>,
    note: Option<Note>,
    state: Option<SyncState>,
    copy: Option<Note>,
) -> Result<bool> {
    let db = database(&app).await?;
    let p = &db.pool;
    apply(p, &id, revision, note, state, copy, &db.cipher).await
}
pub async fn apply(
    p: &SqlitePool,
    id: &str,
    revision: Option<i64>,
    note: Option<Note>,
    state: Option<SyncState>,
    copy: Option<Note>,
    cipher: &crate::crypto::NoteCipher,
) -> Result<bool> {
    let mut tx = p.begin().await?;
    // Acquire a write reservation before checking the revision.
    sqlx::query("UPDATE notes SET dirty=dirty WHERE id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let current: Option<(i64,)> = sqlx::query_as("SELECT local_revision FROM notes WHERE id=?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    if current.map(|r| r.0) != revision {
        return Ok(false);
    }
    // The recovery copy and the remote update/deletion commit or roll back together.
    crate::backups::archive(&mut tx, id, cipher).await?;
    if let Some(c) = copy {
        put(&mut tx, &c, cipher).await?;
    }
    if let Some(n) = note {
        put(&mut tx, &n, cipher).await?;
    } else {
        sqlx::query("DELETE FROM notes WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(s) = state {
        put_state(&mut tx, &s).await?;
    }
    tx.commit().await?;
    Ok(true)
}
#[tauri::command]
pub async fn db_ack(
    app: AppHandle,
    id: String,
    revision: i64,
    state: SyncState,
    deleted: bool,
) -> Result<()> {
    ack(&database(&app).await?.pool, &id, revision, &state, deleted).await
}
pub async fn ack(
    p: &SqlitePool,
    id: &str,
    revision: i64,
    state: &SyncState,
    deleted: bool,
) -> Result<()> {
    let mut tx = p.begin().await?;
    put_state(&mut tx, state).await?;
    if deleted {
        sqlx::query("DELETE FROM notes WHERE id=? AND local_revision=? AND deleted_at IS NOT NULL")
            .bind(id)
            .bind(revision)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("UPDATE notes SET dirty=0 WHERE id=? AND local_revision=?")
            .bind(id)
            .bind(revision)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
