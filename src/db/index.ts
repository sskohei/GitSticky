import { invoke } from '@tauri-apps/api/core';
import type { Note, Settings, Snapshot, SyncState } from '../types';
export interface LocalDatabase {
  snapshot(): Promise<Snapshot>;
  save(note: Note): Promise<void>;
  settings(value: Settings): Promise<void>;
  apply(
    id: string,
    revision: number | null,
    note: Note | null,
    state: SyncState | null,
    copy: Note | null,
  ): Promise<boolean>;
  ack(id: string, revision: number, state: SyncState, deleted: boolean): Promise<void>;
}
// Rust owns the encrypted database and its migrations.
export class NativeDatabase implements LocalDatabase {
  async snapshot() {
    return invoke<Snapshot>('db_snapshot');
  }
  async save(note: Note) {
    await invoke('db_save', { note });
  }
  async settings(value: Settings) {
    await invoke('db_settings', { settings: value });
  }
  async apply(
    id: string,
    revision: number | null,
    note: Note | null,
    state: SyncState | null,
    copy: Note | null,
  ) {
    return invoke<boolean>('db_apply', { id, revision, note, state, copy });
  }
  async ack(id: string, revision: number, state: SyncState, deleted: boolean) {
    await invoke('db_ack', { id, revision, state, deleted });
  }
}
