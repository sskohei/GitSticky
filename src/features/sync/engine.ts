import type { LocalDatabase } from '../../db';
import { title } from '../../lib/notes';
import { AppError, connectionId, type Note, type Settings, type SyncState } from '../../types';
import { parse, same, serialize } from './format';
import type { GitHub } from './github';
export interface SyncResult {
  conflicts: number;
  errors: string[];
}
export class SyncEngine {
  private running: Promise<SyncResult> | null = null;
  constructor(
    private db: LocalDatabase,
    private github: GitHub,
  ) {}
  run(settings: Settings) {
    if (this.running) return this.running;
    this.running = this.execute(settings).finally(() => {
      this.running = null;
    });
    return this.running;
  }
  private async execute(settings: Settings): Promise<SyncResult> {
    const result: SyncResult = { conflicts: 0, errors: [] };
    const cid = connectionId(settings);
    // The complete listing and all downloads must succeed before absence means deletion.
    const files = await this.github.list(settings);
    const remote = new Map<string, { sha: string; note: Note | null }>();
    for (const f of files) {
      try {
        remote.set(f.id, { sha: f.sha, note: parse(await this.github.read(settings, f), f.id) });
      } catch (e) {
        if (e instanceof AppError && e.code === 'invalid_markdown') {
          remote.set(f.id, { sha: f.sha, note: null });
          result.errors.push(e.message);
        } else throw e;
      }
    }
    const initial = await this.db.snapshot();
    const ids = new Set([...initial.notes.map((n) => n.id), ...remote.keys()]);
    for (const id of ids) {
      for (let attempt = 0; attempt < 3; attempt++) {
        const current = await this.db.snapshot();
        const local = current.notes.find((n) => n.id === id);
        const base = current.states.find((s) => s.note_id === id && s.connection_id === cid);
        const r = remote.get(id);
        if (r && !r.note) break; // Never overwrite or delete a malformed remote file.
        const state = (sha: string | null, revision: number): SyncState => ({
          note_id: id,
          connection_id: cid,
          last_synced_sha: sha,
          last_synced_revision: revision,
          last_synced_at: new Date().toISOString(),
          sync_error: null,
        });
        const apply = async (note: Note | null, sha: string | null, copy: Note | null = null) =>
          this.db.apply(
            id,
            local?.local_revision ?? null,
            note,
            state(sha, note?.local_revision ?? local?.local_revision ?? 0),
            copy,
          );
        const copy = () => ({
          ...local!,
          id: crypto.randomUUID(),
          title: title(Array.from(local!.title).slice(0, 73).join('') + '（競合コピー）'),
          deleted_at: null,
          dirty: true,
          local_revision: 1,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        });
        try {
          if (!local) {
            if (r?.note) {
              if (!(await apply(r.note, r.sha))) continue;
            }
            break;
          }
          const localChanged = !base || local.local_revision !== base.last_synced_revision;
          const remoteChanged = r
            ? !base || r.sha !== base.last_synced_sha
            : Boolean(base?.last_synced_sha);
          if (r?.note && same(local, r.note)) {
            await this.db.ack(id, local.local_revision, state(r.sha, local.local_revision), false);
            break;
          }
          if (remoteChanged) {
            if (localChanged) {
              const protectedCopy = local.deleted_at ? null : copy();
              const next = r?.note ? { ...r.note, local_revision: local.local_revision + 1 } : null;
              if (!(await apply(next, r?.sha ?? null, protectedCopy))) continue;
              result.conflicts++;
              break;
            }
            if (
              !(await apply(
                r?.note ? { ...r.note, local_revision: local.local_revision + 1 } : null,
                r?.sha ?? null,
              ))
            )
              continue;
            break;
          }
          if (!localChanged) break;
          if (local.deleted_at) {
            if (r) await this.github.remove(settings, id, r.sha);
            await this.db.ack(id, local.local_revision, state(null, local.local_revision), true);
            break;
          }
          const sha = await this.github.write(settings, id, serialize(local), r?.sha ?? null);
          await this.db.ack(id, local.local_revision, state(sha, local.local_revision), false);
          break;
        } catch (e) {
          if (e instanceof AppError && e.code === 'conflict' && attempt < 2) {
            const fresh = (await this.github.list(settings)).find((f) => f.id === id);
            if (fresh)
              remote.set(id, {
                sha: fresh.sha,
                note: parse(await this.github.read(settings, fresh), id),
              });
            else remote.delete(id);
            continue;
          }
          throw e;
        }
      }
    }
    if (!result.errors.length) {
      const latest = await this.db.snapshot();
      await this.db.settings({
        ...latest.settings,
        last_successful_sync_at: new Date().toISOString(),
      });
    }
    return result;
  }
}
