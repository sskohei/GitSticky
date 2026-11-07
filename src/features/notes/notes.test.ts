import { describe, expect, it } from 'vitest';
import { newNote, selectNotes, title } from '../../lib/notes';
import { MemoryDatabase } from '../../test/fakes';
describe('local notes', () => {
  it('creates an offline note with a UUID and default title', async () => {
    const db = new MemoryDatabase();
    const n = newNote();
    await db.save(n);
    expect((await db.snapshot()).notes[0].title).toBe('無題のメモ');
    expect(n.id).toMatch(/^[\da-f-]{36}$/);
  });
  it('searches body/title locally and orders pinned then recent', () => {
    const a = { ...newNote(), title: '昔のメモ', updated_at: '2026-01-01' },
      b = { ...newNote(), title: '新しいメモ', updated_at: '2026-11-01' },
      c = { ...newNote(), body_md: '本文の検索語', pinned: true };
    expect(selectNotes([a, b, c], '').map((n) => n.id)).toEqual([c.id, b.id, a.id]);
    expect(selectNotes([a, b, c], '検索語')).toEqual([c]);
    expect(selectNotes([a, b, c], '新しい')).toEqual([b]);
    expect(selectNotes([{ ...a, deleted_at: 'today' }], '')).toEqual([]);
  });
  it('normalizes empty and long titles', () => {
    expect(title('  ')).toBe('無題のメモ');
    expect(Array.from(title('あ'.repeat(100)))).toHaveLength(80);
  });
  it('requires every selected tag and combines tags with keyword search', () => {
    const both = { ...newNote(), title: '企画', tags: ['仕事', 'アイデア'] };
    const work = { ...newNote(), title: '企画', tags: ['仕事'] };
    const idea = { ...newNote(), title: '趣味', tags: ['アイデア'] };
    const untagged = { ...newNote(), title: '企画' };
    const notes = [both, work, idea, untagged];
    expect(selectNotes(notes, '', { tags: ['仕事', 'アイデア'] })).toEqual([both]);
    expect(selectNotes(notes, '趣味', { tags: ['仕事', 'アイデア'] })).toEqual([]);
    expect(
      selectNotes(notes, '仕事')
        .map((note) => note.id)
        .sort(),
    ).toEqual([both.id, work.id].sort());
    expect(selectNotes(notes, '', { untagged: true })).toEqual([untagged]);
    expect(selectNotes([{ ...both, tags: ['Work'] }], '', { tags: ['work'] })).toHaveLength(1);
    expect(selectNotes([{ ...both, deleted_at: 'today' }], '', { tags: ['仕事'] })).toEqual([]);
  });
});
