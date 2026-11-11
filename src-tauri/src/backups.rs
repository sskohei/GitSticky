use crate::{
    crypto::NoteCipher,
    database::EncryptedDatabase,
    errors::{AppError, Result},
    models::Note,
};
use serde::Serialize;
use sqlx::{Sqlite, Transaction};
use tauri::{AppHandle, Manager};
use zeroize::Zeroizing;

pub const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS note_backups (id INTEGER PRIMARY KEY AUTOINCREMENT, content TEXT NOT NULL, saved_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')))";
pub async fn archive(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    cipher: &NoteCipher,
) -> Result<()> {
    let previous: Option<Note> = sqlx::query_as("SELECT id,title,body_md,color,pinned,tags,created_at,updated_at,deleted_at,local_revision,dirty FROM notes WHERE id=?").bind(id).fetch_optional(&mut **tx).await?;
    if let Some(previous) = previous {
        let note = crate::database::decode_note(cipher, previous)?;
        let backup_id = uuid::Uuid::new_v4().to_string();
        let raw = Zeroizing::new(serde_json::to_vec(&(backup_id, note)).map_err(|_| error())?);
        let content = cipher.seal(&raw, "GitSticky:backup:v1")?;
        sqlx::query("INSERT INTO note_backups(content) VALUES(?)")
            .bind(content)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM note_backups WHERE id NOT IN (SELECT id FROM note_backups ORDER BY id DESC LIMIT 100)").execute(&mut **tx).await?;
    }
    Ok(())
}
fn error() -> AppError {
    AppError::new("database", "復旧用コピーを読み取れません。")
}
fn decode(cipher: &NoteCipher, content: &str) -> Result<Note> {
    let raw = cipher.open(content, "GitSticky:backup:v1")?;
    let (_, note): (String, Note) = serde_json::from_slice(&raw).map_err(|_| error())?;
    Ok(note)
}
#[derive(Serialize)]
pub struct BackupSummary {
    id: i64,
    title: String,
    saved_at: String,
}
async fn database(app: &AppHandle) -> Result<EncryptedDatabase> {
    Ok(app
        .state::<crate::database::Database>()
        .get(app)
        .await?
        .clone())
}
#[tauri::command]
pub async fn db_backups(app: AppHandle) -> Result<Vec<BackupSummary>> {
    let db = database(&app).await?;
    let rows: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id,content,saved_at FROM note_backups ORDER BY id DESC")
            .fetch_all(&db.pool)
            .await?;
    rows.into_iter()
        .map(|(id, content, saved_at)| {
            Ok(BackupSummary {
                id,
                title: decode(&db.cipher, &content)?.title,
                saved_at,
            })
        })
        .collect()
}
pub async fn restored_note(db: &EncryptedDatabase, id: i64) -> Result<Note> {
    let (content,): (String,) = sqlx::query_as("SELECT content FROM note_backups WHERE id=?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await?
        .ok_or_else(error)?;
    let mut note = decode(&db.cipher, &content)?;
    note.id = uuid::Uuid::new_v4().to_string();
    note.deleted_at = None;
    note.local_revision = 1;
    note.dirty = true;
    Ok(note)
}
#[tauri::command]
pub async fn db_restore_backup(app: AppHandle, id: i64) -> Result<()> {
    let note = restored_note(&database(&app).await?, id).await?;
    crate::commands::db_save(app, note).await
}
