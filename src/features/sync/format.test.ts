import { describe, expect, it } from 'vitest';
import { newNote } from '../../lib/notes';
import { parse, same, serialize } from './format';

describe('tag sync format compatibility', () => {
  it('reads existing v1 files as untagged and writes tagged notes as v2', () => {
    const note = { ...newNote(), body_md: '# 本文\n\nタグは本文に混ぜない' };
    const old = serialize(note);
    expect(old).toContain('schemaVersion: 1');
    expect(parse(old, note.id).tags).toEqual([]);
    const tagged = { ...note, tags: ['仕事', 'アイデア', '引用: "内容" # タグ'] };
    const content = serialize(tagged);
    expect(content).toContain('schemaVersion: 2');
    expect(parse(content, note.id)).toMatchObject({ tags: tagged.tags, body_md: note.body_md });
    // The already-released parser only accepts schemaVersion 1: it must refuse tagged files.
    expect(content).not.toMatch(/^schemaVersion: 1$/m);
  });
  it('rejects malformed, unsupported and ambiguous tag metadata', () => {
    const note = newNote();
    const content = serialize({ ...note, tags: ['仕事'] });
    for (const invalid of [
      content.replace('schemaVersion: 2', 'schemaVersion: 3'),
      content.replace('schemaVersion: 2', 'schemaVersion: 1'),
      content.replace('tags:\n  - 仕事', 'tags: null'),
      content.replace('tags:\n  - 仕事', 'tags: [123]'),
      content.replace('tags:\n  - 仕事', 'tags: [""]'),
    ]) {
      expect(() => parse(invalid, note.id)).toThrow();
    }
    expect(() => parse(content.replace('tags:\n  - 仕事\n', ''), note.id)).toThrow();
  });
  it('compares tags as a set while detecting added and removed tags', () => {
    const note = { ...newNote(), tags: ['仕事', 'Work'] };
    expect(same(note, { ...note, tags: ['work', '仕事'] })).toBe(true);
    expect(same(note, { ...note, tags: ['仕事'] })).toBe(false);
    expect(same(note, { ...note, tags: ['仕事', 'Work', '個人'] })).toBe(false);
  });
});
