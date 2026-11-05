CREATE TABLE notes (
 id TEXT PRIMARY KEY, title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 80),
 body_md TEXT NOT NULL, color TEXT NOT NULL CHECK(color IN ('yellow','pink','green','blue','purple','gray')),
 pinned BOOLEAN NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
 deleted_at TEXT, local_revision INTEGER NOT NULL, dirty BOOLEAN NOT NULL
);
CREATE TABLE sync_states (
 note_id TEXT NOT NULL, connection_id TEXT NOT NULL, last_synced_sha TEXT,
 last_synced_revision INTEGER NOT NULL, last_synced_at TEXT NOT NULL, sync_error TEXT,
 PRIMARY KEY(note_id,connection_id)
);
CREATE TABLE app_settings (id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL);
CREATE INDEX notes_sort ON notes(pinned DESC,updated_at DESC);
