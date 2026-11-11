use crate::{
    crypto::NoteCipher,
    errors::{AppError, Result},
    models::Note,
    secure_store::{NativeStore, SecretStore},
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use tauri::{AppHandle, Manager};
use tokio::sync::OnceCell;
use zeroize::Zeroizing;

#[derive(Default)]
pub struct Database(OnceCell<EncryptedDatabase>);
#[derive(Clone)]
pub struct EncryptedDatabase {
    pub pool: SqlitePool,
    pub cipher: NoteCipher,
}
impl Database {
    pub async fn get(&self, app: &AppHandle) -> Result<&EncryptedDatabase> {
        self.0
            .get_or_try_init(|| async {
                let dir = app.path().app_config_dir().map_err(|_| error())?;
                std::fs::create_dir_all(&dir).map_err(|_| error())?;
                let options = SqliteConnectOptions::new()
                    .filename(dir.join("stickynote.db"))
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Delete)
                    .pragma("secure_delete", "ON")
                    .pragma("temp_store", "MEMORY");
                let pool = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(options)
                    .await?;
                match initialize(&pool, &NativeStore(app.clone(), "notes-key")).await {
                    Ok(cipher) => Ok(EncryptedDatabase { pool, cipher }),
                    Err(e) => {
                        pool.close().await;
                        Err(e)
                    }
                }
            })
            .await
    }
}
fn error() -> AppError {
    AppError::new(
        "secure_storage",
        "メモの保護領域を開けません。鍵とメモを削除せず再試行してください。",
    )
}

pub async fn initialize(pool: &SqlitePool, store: &dyn SecretStore) -> Result<NoteCipher> {
    // Recognize existing Tauri SQL databases by schema, preserving its migration ledger.
    let mut tx = pool.begin().await?;
    let exists: (i64,) =
        sqlx::query_as("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='notes'")
            .fetch_one(&mut *tx)
            .await?;
    if exists.0 == 0 {
        sqlx::raw_sql(include_str!("../migrations/001_initial.sql"))
            .execute(&mut *tx)
            .await?;
    }
    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('notes')")
        .fetch_all(&mut *tx)
        .await?;
    if !columns.iter().any(|(name,)| name == "tags") {
        sqlx::raw_sql(include_str!("../migrations/002_note_tags.sql"))
            .execute(&mut *tx)
            .await?;
    }
    if !columns.iter().any(|(name,)| name == "encrypted") {
        sqlx::query("ALTER TABLE notes ADD COLUMN encrypted BOOLEAN NOT NULL DEFAULT 0 CHECK(encrypted IN (0,1))").execute(&mut *tx).await?;
    }
    let encrypted: (i64,) = sqlx::query_as("SELECT count(*) FROM notes WHERE encrypted=1")
        .fetch_one(&mut *tx)
        .await?;
    let cipher = match store.load()? {
        Some(bytes) => NoteCipher::from_encoded(std::str::from_utf8(&bytes).map_err(|_| error())?)?,
        None if encrypted.0 > 0 => return Err(error()),
        None => {
            let cipher = NoteCipher::random();
            store.save(cipher.encoded().as_bytes())?;
            cipher
        }
    };
    let protected = sqlx::query_as::<_, Note>("SELECT id,title,body_md,color,pinned,tags,created_at,updated_at,deleted_at,local_revision,dirty FROM notes WHERE encrypted=1").fetch_all(&mut *tx).await?;
    for note in protected {
        decode_note(&cipher, note)?;
    }
    let notes = sqlx::query_as::<_, Note>("SELECT id,title,body_md,color,pinned,tags,created_at,updated_at,deleted_at,local_revision,dirty FROM notes WHERE encrypted=0").fetch_all(&mut *tx).await?;
    for note in &notes {
        let encrypted = encode_note(&cipher, note)?;
        sqlx::query("UPDATE notes SET title=?,body_md=?,tags='[]',encrypted=1 WHERE id=?")
            .bind(&encrypted.title)
            .bind(&encrypted.body_md)
            .bind(&note.id)
            .execute(&mut *tx)
            .await?;
    }
    // A persistent marker makes cleanup retryable if the process dies after COMMIT.
    sqlx::query("CREATE TABLE IF NOT EXISTS security_migrations (id INTEGER PRIMARY KEY, cleaned BOOLEAN NOT NULL)").execute(&mut *tx).await?;
    if !notes.is_empty() {
        sqlx::query(
            "INSERT INTO security_migrations VALUES(1,0) ON CONFLICT(id) DO UPDATE SET cleaned=0",
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let pending: Option<(bool,)> =
        sqlx::query_as("SELECT cleaned FROM security_migrations WHERE id=1")
            .fetch_optional(pool)
            .await?;
    if pending == Some((false,)) {
        sqlx::query("VACUUM").execute(pool).await?;
        sqlx::query("UPDATE security_migrations SET cleaned=1 WHERE id=1")
            .execute(pool)
            .await?;
    }
    // Fail closed before exposing or modifying a database with a wrong saved key.
    let saved = sqlx::query_as::<_, Note>("SELECT id,title,body_md,color,pinned,tags,created_at,updated_at,deleted_at,local_revision,dirty FROM notes").fetch_all(pool).await?;
    for note in saved {
        decode_note(&cipher, note)?;
    }
    Ok(cipher)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PrivateNote {
    title: String,
    body_md: String,
    tags: Vec<String>,
}
pub fn encode_note(cipher: &NoteCipher, note: &Note) -> Result<Note> {
    let raw = Zeroizing::new(
        serde_json::to_vec(&PrivateNote {
            title: note.title.clone(),
            body_md: note.body_md.clone(),
            tags: note.tags.clone(),
        })
        .map_err(|_| error())?,
    );
    let mut row = note.clone();
    row.title = "暗号化されたメモ".into();
    row.body_md = cipher.seal(&raw, &format!("GitSticky:local:v1:{}", note.id))?;
    row.tags.clear();
    Ok(row)
}
pub fn decode_note(cipher: &NoteCipher, mut row: Note) -> Result<Note> {
    let raw = cipher.open(&row.body_md, &format!("GitSticky:local:v1:{}", row.id))?;
    let private: PrivateNote = serde_json::from_slice(&raw).map_err(|_| error())?;
    row.title = private.title;
    row.body_md = private.body_md;
    row.tags = private.tags;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Store {
        value: Mutex<Option<Vec<u8>>>,
        fail_save: bool,
    }
    impl SecretStore for Store {
        fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
            Ok(self.value.lock().unwrap().clone().map(Zeroizing::new))
        }
        fn save(&self, bytes: &[u8]) -> Result<()> {
            if self.fail_save {
                return Err(error());
            }
            *self.value.lock().unwrap() = Some(bytes.to_vec());
            Ok(())
        }
        fn clear(&self) -> Result<()> {
            *self.value.lock().unwrap() = None;
            Ok(())
        }
    }
    fn store() -> Store {
        Store {
            value: Mutex::new(None),
            fail_save: false,
        }
    }
    async fn legacy(pool: &SqlitePool) {
        sqlx::raw_sql(include_str!("../migrations/001_initial.sql"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/002_note_tags.sql"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO notes VALUES(?,?,?,?,?,?,?,?,?,?,?)")
            .bind("550e8400-e29b-41d4-a716-446655440000")
            .bind("秘密のタイトル")
            .bind("秘密の本文を保持")
            .bind("yellow")
            .bind(false)
            .bind("2026-11-01")
            .bind("2026-11-02")
            .bind(Option::<String>::None)
            .bind(7i64)
            .bind(true)
            .bind("[\"秘密のタグ\"]")
            .execute(pool)
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn migration_encrypts_title_body_tags_and_survives_reopen_without_plaintext_pages() {
        let path =
            std::env::temp_dir().join(format!("sticky-encryption-{}.db", uuid::Uuid::new_v4()));
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Delete)
            .pragma("secure_delete", "ON")
            .pragma("temp_store", "MEMORY");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        legacy(&pool).await;
        let store = store();
        let cipher = initialize(&pool, &store).await.unwrap();
        let row: Note = sqlx::query_as("SELECT id,title,body_md,color,pinned,tags,created_at,updated_at,deleted_at,local_revision,dirty FROM notes").fetch_one(&pool).await.unwrap();
        assert_eq!(row.title, "暗号化されたメモ");
        assert!(row.tags.is_empty());
        let note = decode_note(&cipher, row).unwrap();
        assert_eq!(note.title, "秘密のタイトル");
        assert_eq!(note.body_md, "秘密の本文を保持");
        assert_eq!(note.tags, ["秘密のタグ"]);
        assert_eq!(note.local_revision, 7);
        assert!(note.dirty);
        // JSON escaping must not strand a previously supported Markdown note.
        let large = Note {
            body_md: "\n".repeat(3_100_000),
            ..note.clone()
        };
        let encoded = encode_note(&cipher, &large).unwrap();
        assert_eq!(
            decode_note(&cipher, encoded).unwrap().body_md,
            large.body_md
        );
        pool.close().await;
        let bytes = std::fs::read(&path).unwrap();
        for secret in ["秘密のタイトル", "秘密の本文を保持", "秘密のタグ"] {
            assert!(!bytes.windows(secret.len()).any(|w| w == secret.as_bytes()));
        }
        let reopened = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let cipher = initialize(&reopened, &store).await.unwrap();
        let row = sqlx::query_as("SELECT id,title,body_md,color,pinned,tags,created_at,updated_at,deleted_at,local_revision,dirty FROM notes").fetch_one(&reopened).await.unwrap();
        assert_eq!(decode_note(&cipher, row).unwrap().body_md, note.body_md);
        assert!(initialize(&reopened, &self::store()).await.is_err());
        let wrong = Store {
            value: Mutex::new(Some(NoteCipher::random().encoded().as_bytes().to_vec())),
            fail_save: false,
        };
        assert!(initialize(&reopened, &wrong).await.is_err());
        let row = sqlx::query_as("SELECT id,title,body_md,color,pinned,tags,created_at,updated_at,deleted_at,local_revision,dirty FROM notes").fetch_one(&reopened).await.unwrap();
        assert_eq!(decode_note(&cipher, row).unwrap().body_md, note.body_md);
        reopened.close().await;
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn failed_key_storage_rolls_back_migration_and_can_retry() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        legacy(&pool).await;
        let broken = Store {
            fail_save: true,
            ..store()
        };
        assert!(initialize(&pool, &broken).await.is_err());
        let row: Note = sqlx::query_as("SELECT id,title,body_md,color,pinned,tags,created_at,updated_at,deleted_at,local_revision,dirty FROM notes").fetch_one(&pool).await.unwrap();
        assert_eq!(row.body_md, "秘密の本文を保持");
        assert!(initialize(&pool, &store()).await.is_ok());
    }
}
