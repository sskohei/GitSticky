use crate::{
    commands::ack,
    models::{Note, SyncState},
};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
fn cipher() -> crate::crypto::NoteCipher {
    crate::crypto::NoteCipher::new([42; 32])
}
async fn apply(
    p: &SqlitePool,
    id: &str,
    revision: Option<i64>,
    note: Option<Note>,
    state: Option<SyncState>,
    copy: Option<Note>,
) -> crate::errors::Result<bool> {
    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('notes')")
        .fetch_all(p)
        .await?;
    if !columns.iter().any(|(name,)| name == "encrypted") {
        sqlx::query("ALTER TABLE notes ADD COLUMN encrypted BOOLEAN NOT NULL DEFAULT 0")
            .execute(p)
            .await?;
    }
    crate::commands::apply(p, id, revision, note, state, copy, &cipher()).await
}
async fn read_note(p: &SqlitePool) -> Note {
    let note: Note = sqlx::query_as("SELECT id,title,body_md,color,pinned,tags,created_at,updated_at,deleted_at,local_revision,dirty FROM notes").fetch_one(p).await.unwrap();
    if note.title == "暗号化されたメモ" {
        crate::database::decode_note(&cipher(), note).unwrap()
    } else {
        note
    }
}
async fn database() -> SqlitePool {
    let p = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/001_initial.sql"))
        .execute(&p)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/002_note_tags.sql"))
        .execute(&p)
        .await
        .unwrap();
    sqlx::query(crate::backups::SCHEMA)
        .execute(&p)
        .await
        .unwrap();
    p
}
fn note() -> Note {
    Note {
        id: "550e8400-e29b-41d4-a716-446655440000".into(),
        title: "日本語".into(),
        body_md: "本文\n次の行".into(),
        color: "yellow".into(),
        pinned: false,
        tags: vec!["仕事".into(), "アイデア".into()],
        created_at: "2026-11-04T00:00:00Z".into(),
        updated_at: "2026-11-04T00:00:00Z".into(),
        deleted_at: None,
        local_revision: 2,
        dirty: true,
    }
}
fn state(n: &Note) -> SyncState {
    SyncState {
        note_id: n.id.clone(),
        connection_id: "repo".into(),
        last_synced_sha: Some("sha".into()),
        last_synced_revision: 1,
        last_synced_at: "now".into(),
        sync_error: None,
    }
}
#[tokio::test]
async fn remote_update_and_deletion_keep_encrypted_restorable_copies() {
    let p = database().await;
    let original = note();
    apply(&p, &original.id, None, Some(original.clone()), None, None)
        .await
        .unwrap();
    let changed = Note {
        body_md: "更新後の本文".into(),
        local_revision: 3,
        ..original.clone()
    };
    apply(&p, &original.id, Some(2), Some(changed), None, None)
        .await
        .unwrap();
    apply(&p, &original.id, Some(3), None, None, None)
        .await
        .unwrap();
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id,content FROM note_backups ORDER BY id")
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(!rows[0].1.contains(&original.title));
    assert!(!rows[0].1.contains(&original.body_md));
    let db = crate::database::EncryptedDatabase {
        pool: p.clone(),
        cipher: cipher(),
    };
    let restored = crate::backups::restored_note(&db, rows[0].0).await.unwrap();
    assert_eq!(restored.body_md, original.body_md);
    assert_eq!(restored.tags, original.tags);
    assert_ne!(restored.id, original.id);
    assert!(restored.deleted_at.is_none());
    assert!(restored.dirty);
    assert!(crate::backups::restored_note(
        &crate::database::EncryptedDatabase {
            pool: p.clone(),
            cipher: crate::crypto::NoteCipher::random()
        },
        rows[0].0
    )
    .await
    .is_err());
}
#[tokio::test]
async fn backup_and_remote_write_roll_back_together_on_invalid_update() {
    let p = database().await;
    let original = note();
    apply(&p, &original.id, None, Some(original.clone()), None, None)
        .await
        .unwrap();
    let invalid = Note {
        title: String::new(),
        ..original.clone()
    };
    assert!(apply(&p, &original.id, Some(2), Some(invalid), None, None)
        .await
        .is_err());
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM note_backups")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(read_note(&p).await.body_md, original.body_md);
}
#[tokio::test]
async fn acknowledgement_does_not_clear_new_edits() {
    let p = database().await;
    let n = note();
    apply(&p, &n.id, None, Some(n.clone()), None, None)
        .await
        .unwrap();
    ack(&p, &n.id, 1, &state(&n), false).await.unwrap();
    let saved = read_note(&p).await;
    assert!(saved.dirty);
    assert_eq!(saved.body_md, "本文\n次の行");
}
#[tokio::test]
async fn conflict_copy_and_remote_apply_are_atomic() {
    let p = database().await;
    let n = note();
    apply(&p, &n.id, None, Some(n.clone()), None, None)
        .await
        .unwrap();
    let mut invalid = n.clone();
    invalid.id = uuid::Uuid::new_v4().to_string();
    invalid.title = "".into();
    assert!(
        apply(&p, &n.id, Some(2), None, Some(state(&n)), Some(invalid))
            .await
            .is_err()
    );
    let saved = read_note(&p).await;
    assert_eq!(saved.body_md, n.body_md);
    let mut copy = n.clone();
    copy.id = uuid::Uuid::new_v4().to_string();
    let mut remote = n.clone();
    remote.body_md = "リモート".into();
    remote.local_revision = 3;
    assert!(apply(
        &p,
        &n.id,
        Some(2),
        Some(remote),
        Some(state(&n)),
        Some(copy)
    )
    .await
    .unwrap());
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM notes")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count.0, 2);
}
#[tokio::test]
async fn stale_download_cannot_overwrite_a_new_revision() {
    let p = database().await;
    let n = note();
    apply(&p, &n.id, None, Some(n.clone()), None, None)
        .await
        .unwrap();
    assert!(!apply(&p, &n.id, Some(1), None, None, None).await.unwrap());
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM notes")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn reopening_sqlite_preserves_japanese_markdown() {
    let path = std::env::temp_dir().join(format!("stickynote-db-test-{}.db", uuid::Uuid::new_v4()));
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let p = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/001_initial.sql"))
        .execute(&p)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/002_note_tags.sql"))
        .execute(&p)
        .await
        .unwrap();
    let n = note();
    apply(&p, &n.id, None, Some(n.clone()), None, None)
        .await
        .unwrap();
    p.close().await;
    let reopened = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let saved = read_note(&reopened).await;
    assert_eq!(saved.body_md, n.body_md);
    assert_eq!(saved.title, n.title);
    assert_eq!(saved.tags, n.tags);
    reopened.close().await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn tag_migration_preserves_existing_notes_without_tags() {
    let p = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/001_initial.sql"))
        .execute(&p)
        .await
        .unwrap();
    let n = note();
    sqlx::query("INSERT INTO notes VALUES(?,?,?,?,?,?,?,?,?,?)")
        .bind(&n.id)
        .bind(&n.title)
        .bind(&n.body_md)
        .bind(&n.color)
        .bind(n.pinned)
        .bind(&n.created_at)
        .bind(&n.updated_at)
        .bind(&n.deleted_at)
        .bind(n.local_revision)
        .bind(n.dirty)
        .execute(&p)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/002_note_tags.sql"))
        .execute(&p)
        .await
        .unwrap();
    let saved = read_note(&p).await;
    assert!(saved.tags.is_empty());
    assert_eq!(saved.body_md, n.body_md);
    assert_eq!(saved.local_revision, n.local_revision);
    assert_eq!(saved.dirty, n.dirty);
    // Production initializes encryption and backup storage before any db_apply.
    sqlx::query("ALTER TABLE notes ADD COLUMN encrypted BOOLEAN NOT NULL DEFAULT 0")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query(crate::backups::SCHEMA)
        .execute(&p)
        .await
        .unwrap();
    let encrypted = crate::database::encode_note(&cipher(), &saved).unwrap();
    sqlx::query("UPDATE notes SET title=?,body_md=?,tags='[]',encrypted=1 WHERE id=?")
        .bind(encrypted.title)
        .bind(encrypted.body_md)
        .bind(&saved.id)
        .execute(&p)
        .await
        .unwrap();
    let mut tagged = saved;
    tagged.tags = n.tags;
    tagged.local_revision += 1;
    assert!(apply(
        &p,
        &n.id,
        Some(n.local_revision),
        Some(tagged.clone()),
        None,
        None
    )
    .await
    .unwrap());
    let saved = read_note(&p).await;
    assert_eq!(saved.tags, tagged.tags);
}

#[tokio::test]
async fn invalid_tags_are_rejected_without_overwriting_saved_notes() {
    let p = database().await;
    let n = note();
    apply(&p, &n.id, None, Some(n.clone()), None, None)
        .await
        .unwrap();
    for tags in [
        vec!["".into()],
        vec![" 仕事".into()],
        vec!["あ".repeat(31)],
        vec!["改行\n禁止".into()],
        vec!["Work".into(), "work".into()],
        (0..21).map(|index| format!("tag{index}")).collect(),
    ] {
        let mut invalid = n.clone();
        invalid.tags = tags;
        invalid.body_md = "上書きしない".into();
        assert!(
            apply(&p, &n.id, Some(n.local_revision), Some(invalid), None, None)
                .await
                .is_err()
        );
        let saved = read_note(&p).await;
        assert_eq!(saved.tags, n.tags);
        assert_eq!(saved.body_md, n.body_md);
    }
}
