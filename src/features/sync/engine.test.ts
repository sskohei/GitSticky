import { beforeEach, describe, expect, it } from 'vitest';
import { MemoryDatabase, FakeGitHub } from '../../test/fakes';
import { newNote } from '../../lib/notes';
import { AppError, connectionId } from '../../types';
import { SyncEngine } from './engine';
import { parse, serialize } from './format';
describe('safe bidirectional sync', () => {
  let db: MemoryDatabase;
  let gh: FakeGitHub;
  let engine: SyncEngine;
  beforeEach(() => {
    db = new MemoryDatabase();
    gh = new FakeGitHub();
    engine = new SyncEngine(db, gh);
  });
  const local = () => ({
    ...newNote(),
    title: '日本語のメモ',
    body_md: '# こんにちは\n\n本文です。\n',
  });
  const run = () => engine.run(db.data.settings);
  it('re-encrypts unchanged legacy notes in one sync without manufacturing conflicts', async () => {
    const n = local();
    await db.save(n);
    await run();
    const read = gh.read.bind(gh);
    gh.read = async (settings, file) => {
      const content = await read(settings, file);
      file.needsEncryption = true;
      return content;
    };
    const before = gh.files.get(n.id)!.sha;
    const result = await run();
    expect(result.conflicts).toBe(0);
    expect(gh.files.get(n.id)!.sha).not.toBe(before);
    expect(db.data.notes).toHaveLength(1);
    expect(db.data.notes[0].dirty).toBe(false);
  });
  it('encrypts an imported legacy note during the first sync', async () => {
    const n = local();
    gh.set(n);
    const before = gh.files.get(n.id)!.sha;
    const read = gh.read.bind(gh);
    gh.read = async (settings, file) => {
      file.needsEncryption = true;
      return read(settings, file);
    };
    await run();
    expect(gh.files.get(n.id)!.sha).not.toBe(before);
    expect(db.data.notes[0].dirty).toBe(false);
  });
  it('stops before uploads, deletes or local changes if any encrypted note cannot be decrypted', async () => {
    const n = local();
    await db.save(n);
    gh.set(local());
    const before = structuredClone(db.data);
    const remoteBefore = structuredClone([...gh.files]);
    gh.read = async () => {
      throw new AppError('locked', '暗号鍵が違います。');
    };
    await expect(run()).rejects.toMatchObject({ code: 'locked' });
    expect(db.data).toEqual(before);
    expect([...gh.files]).toEqual(remoteBefore);
  });
  it('merges first sync without dropping unrelated local and remote notes', async () => {
    const a = local(),
      b = local();
    await db.save(a);
    gh.set(b);
    await run();
    expect(db.data.notes).toHaveLength(2);
    expect(gh.files.size).toBe(2);
    expect(db.data.notes.every((n) => !n.dirty)).toBe(true);
  });
  it('uploads local updates and downloads remote updates preserving timestamps', async () => {
    const n = local();
    await db.save(n);
    await run();
    await db.save({ ...n, body_md: 'ローカル' });
    await run();
    expect(parse(gh.files.get(n.id)!.content, n.id).body_md).toBe('ローカル');
    const remote = { ...n, body_md: 'リモート', updated_at: '2026-10-01T00:00:00.000Z' };
    gh.set(remote);
    await run();
    expect(db.data.notes[0].body_md).toBe('リモート');
    expect(db.data.notes[0].updated_at).toBe(remote.updated_at);
  });
  it('protects conflict text once and syncs the copy subsequently', async () => {
    const n = local();
    await db.save(n);
    await run();
    await db.save({ ...n, body_md: '端末A' });
    gh.set({ ...n, body_md: '端末B' });
    expect((await run()).conflicts).toBe(1);
    expect(db.data.notes.map((n) => n.body_md).sort()).toEqual(['端末A', '端末B']);
    expect(db.data.notes.find((v) => v.id !== n.id)!.title).toContain('（競合コピー）');
    await run();
    await run();
    expect(db.data.notes).toHaveLength(2);
    expect(gh.files.size).toBe(2);
  });
  it('keeps edits made during upload dirty with the latest body', async () => {
    const n = local();
    await db.save(n);
    gh.onWrite = async () => {
      await db.save({ ...n, body_md: '同期中の追加入力' });
      gh.onWrite = null;
    };
    await run();
    expect(db.data.notes[0].dirty).toBe(true);
    expect(db.data.notes[0].body_md).toBe('同期中の追加入力');
    await run();
    expect(parse(gh.files.get(n.id)!.content, n.id).body_md).toBe('同期中の追加入力');
    expect(db.data.notes[0].dirty).toBe(false);
  });
  it('reflects a remote deletion when local is unchanged', async () => {
    const n = local();
    await db.save(n);
    await run();
    gh.files.delete(n.id);
    await run();
    expect(db.data.notes).toHaveLength(0);
  });
  it('protects an edited local note after a remote deletion', async () => {
    const n = local();
    await db.save(n);
    await run();
    gh.files.delete(n.id);
    await db.save({ ...n, body_md: '消さないで' });
    expect((await run()).conflicts).toBe(1);
    expect(db.data.notes).toHaveLength(1);
    expect(db.data.notes[0].id).not.toBe(n.id);
    expect(db.data.notes[0].body_md).toBe('消さないで');
    await run();
    expect(db.data.notes).toHaveLength(1);
  });
  it('restores changed remote content instead of deleting it', async () => {
    const n = local();
    await db.save(n);
    await run();
    await db.save({ ...n, deleted_at: new Date().toISOString() });
    gh.set({ ...n, body_md: '相手の更新' });
    expect((await run()).conflicts).toBe(1);
    expect(gh.files.has(n.id)).toBe(true);
    expect(db.data.notes[0].body_md).toBe('相手の更新');
    expect(db.data.notes[0].deleted_at).toBeNull();
  });
  it('retains offline deletion until reconnection, then cleans the tombstone', async () => {
    const n = local();
    await db.save(n);
    await run();
    await db.save({ ...n, deleted_at: new Date().toISOString() });
    gh.offline = true;
    await expect(run()).rejects.toMatchObject({ code: 'network' });
    expect(db.data.notes[0].deleted_at).not.toBeNull();
    gh.offline = false;
    await run();
    expect(db.data.notes).toHaveLength(0);
    expect(gh.files.size).toBe(0);
  });
  it('never interprets a failed or incomplete listing as deletion', async () => {
    const n = local();
    await db.save(n);
    await run();
    const before = await db.snapshot();
    gh.list = async () => {
      throw new AppError('network', 'truncated');
    };
    await expect(run()).rejects.toThrow('truncated');
    expect(db.data.notes).toEqual(before.notes);
  });
  it('does not mutate malformed remote markdown', async () => {
    const n = local();
    await db.save(n);
    await run();
    gh.files.set(n.id, { content: '---\nwrong: [', sha: 'bad' });
    await db.save({ ...n, body_md: '保存する本文' });
    const result = await run();
    expect(result.errors).toHaveLength(1);
    expect(gh.files.get(n.id)!.sha).toBe('bad');
    expect(db.data.notes[0].body_md).toBe('保存する本文');
    expect(db.data.notes[0].dirty).toBe(true);
  });
  it('rechecks after a 409 and does not run twice concurrently', async () => {
    await db.save(local());
    gh.conflictOnce = true;
    const a = run(),
      b = run();
    expect(a).toBe(b);
    await a;
    expect(gh.files.size).toBe(1);
  });
  it('never reuses another repository SHA', async () => {
    await db.save(local());
    await run();
    const oldCid = connectionId(db.data.settings);
    db.data.settings.repository_name = 'other';
    gh.files.clear();
    await run();
    expect(db.data.states).toHaveLength(2);
    expect(db.data.states.some((s) => s.connection_id === oldCid)).toBe(true);
    expect(gh.files.size).toBe(1);
  });
  it('preserves local state on a mid-upload disconnection', async () => {
    const a = local(),
      b = local();
    await db.save(a);
    await db.save(b);
    gh.onWrite = async () => {
      gh.offline = true;
    };
    await expect(run()).rejects.toMatchObject({ code: 'network' });
    expect(db.data.notes).toHaveLength(2);
    expect(db.data.notes.filter((n) => n.dirty)).toHaveLength(1);
    gh.offline = false;
    gh.onWrite = null;
    await run();
    expect(db.data.notes.every((n) => !n.dirty)).toBe(true);
  });
  it('deduplicates identical concurrent contents', async () => {
    const n = local();
    await db.save(n);
    gh.set(n);
    await run();
    expect(db.data.notes).toHaveLength(1);
    expect(db.data.notes[0].dirty).toBe(false);
  });
  it('keeps data for all typed API failures', async () => {
    for (const code of [
      'auth',
      'permission',
      'repository',
      'branch',
      'rate_limit',
      'conflict',
    ] as const) {
      const n = local();
      await db.save(n);
      gh.list = async () => {
        throw new AppError(code, 'error');
      };
      await expect(run()).rejects.toMatchObject({ code });
      expect(db.data.notes.some((v) => v.id === n.id && v.dirty)).toBe(true);
    }
  });
  it('round trips escaped titles and Japanese multiline body', () => {
    const n = { ...local(), title: '引用: "内容" # タイトル' };
    expect(parse(serialize(n), n.id).body_md).toBe(n.body_md);
    expect(parse(serialize(n), n.id).title).toBe(n.title);
  });
  it('syncs tag-only additions and removals to another device without altering the body', async () => {
    const n = local();
    await db.save(n);
    await run();
    await db.save({ ...db.data.notes[0], tags: ['仕事', 'アイデア'] });
    await run();
    expect(parse(gh.files.get(n.id)!.content, n.id).tags).toEqual(['仕事', 'アイデア']);
    const other = new MemoryDatabase();
    const otherEngine = new SyncEngine(other, gh);
    await otherEngine.run(other.data.settings);
    expect(other.data.notes[0]).toMatchObject({ tags: ['仕事', 'アイデア'], body_md: n.body_md });
    await db.save({ ...db.data.notes[0], tags: [] });
    await run();
    await otherEngine.run(other.data.settings);
    expect(other.data.notes[0]).toMatchObject({ tags: [], body_md: n.body_md });
  });
  it('preserves both tag sets in a concurrent tag-only conflict', async () => {
    const n = local();
    await db.save(n);
    await run();
    await db.save({ ...db.data.notes[0], tags: ['仕事'] });
    gh.set({ ...n, tags: ['個人'] });
    expect((await run()).conflicts).toBe(1);
    expect(db.data.notes.find((note) => note.id === n.id)?.tags).toEqual(['個人']);
    expect(db.data.notes.find((note) => note.id !== n.id)?.tags).toEqual(['仕事']);
    await run();
    expect(gh.files.size).toBe(2);
    expect([...gh.files].map(([id, file]) => parse(file.content, id).tags[0]).sort()).toEqual(
      ['仕事', '個人'].sort(),
    );
  });
});
