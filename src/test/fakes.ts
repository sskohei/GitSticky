import type { LocalDatabase } from '../db';
import {
  AppError,
  defaultSettings,
  type Note,
  type Settings,
  type Snapshot,
  type SyncState,
} from '../types';
import type { GitHub, RemoteFile } from '../features/sync/github';
import { serialize } from '../features/sync/format';
export class MemoryDatabase implements LocalDatabase {
  data: Snapshot = {
    notes: [],
    states: [],
    settings: { ...defaultSettings, repository_owner: 'owner', repository_name: 'notes' },
  };
  async snapshot() {
    return structuredClone(this.data);
  }
  async save(note: Note) {
    const old = this.data.notes.find((n) => n.id === note.id);
    const value = {
      ...structuredClone(note),
      local_revision: (old?.local_revision ?? 0) + 1,
      dirty: true,
    };
    this.data.notes = this.data.notes.filter((n) => n.id !== note.id).concat(value);
  }
  async settings(settings: Settings) {
    this.data.settings = structuredClone(settings);
  }
  async apply(
    id: string,
    revision: number | null,
    note: Note | null,
    state: SyncState | null,
    copy: Note | null,
  ) {
    const current = this.data.notes.find((n) => n.id === id);
    if ((current?.local_revision ?? null) !== revision) return false;
    this.data.notes = this.data.notes.filter((n) => n.id !== id);
    if (note) this.data.notes.push(structuredClone(note));
    if (copy) this.data.notes.push(structuredClone(copy));
    if (state) this.state(state);
    return true;
  }
  state(s: SyncState) {
    this.data.states = this.data.states
      .filter((v) => v.note_id !== s.note_id || v.connection_id !== s.connection_id)
      .concat(structuredClone(s));
  }
  async ack(id: string, revision: number, state: SyncState, deleted: boolean) {
    this.state(state);
    const note = this.data.notes.find((n) => n.id === id);
    if (note?.local_revision === revision) {
      if (deleted) this.data.notes = this.data.notes.filter((n) => n.id !== id);
      else note.dirty = false;
    }
  }
}
export class FakeGitHub implements GitHub {
  files = new Map<string, { content: string; sha: string }>();
  counter = 0;
  offline = false;
  onWrite: (() => Promise<void>) | null = null;
  conflictOnce = false;
  set(note: Note) {
    this.files.set(note.id, { content: serialize(note), sha: String(++this.counter) });
  }
  async list() {
    if (this.offline) throw new AppError('network', 'オフライン');
    return [...this.files].map(([id, v]) => ({ id, sha: v.sha }));
  }
  async read(_s: Settings, f: RemoteFile) {
    return this.files.get(f.id)!.content;
  }
  async write(_s: Settings, id: string, content: string, sha: string | null) {
    if (this.offline) throw new AppError('network', 'オフライン');
    if (this.conflictOnce) {
      this.conflictOnce = false;
      throw new AppError('conflict', '競合');
    }
    if ((this.files.get(id)?.sha ?? null) !== sha) throw new AppError('conflict', '競合');
    await this.onWrite?.();
    const next = String(++this.counter);
    this.files.set(id, { content, sha: next });
    return next;
  }
  async remove(_s: Settings, id: string, sha: string) {
    if (this.files.get(id)?.sha !== sha) throw new AppError('conflict', '競合');
    this.files.delete(id);
  }
}
