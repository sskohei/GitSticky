use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body_md: String,
    pub color: String,
    pub pinned: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub local_revision: i64,
    pub dirty: bool,
}
#[derive(Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SyncState {
    pub note_id: String,
    pub connection_id: String,
    pub last_synced_sha: Option<String>,
    pub last_synced_revision: i64,
    pub last_synced_at: String,
    pub sync_error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub repository_owner: String,
    pub repository_name: String,
    pub branch_name: String,
    pub auto_sync_enabled: bool,
    pub theme: String,
    pub last_successful_sync_at: Option<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            repository_owner: String::new(),
            repository_name: String::new(),
            branch_name: "main".into(),
            auto_sync_enabled: true,
            theme: "system".into(),
            last_successful_sync_at: None,
        }
    }
}
#[derive(Serialize)]
pub struct Snapshot {
    pub notes: Vec<Note>,
    pub states: Vec<SyncState>,
    pub settings: Settings,
}
