import { describe, expect, it } from 'vitest';
import { availableTags, normalizeTags } from './tags';
import { newNote } from './notes';

describe('tags', () => {
  it('trims Japanese names, deduplicates ASCII case and retains the original display name', () => {
    expect(normalizeTags([' 仕事 ', '仕事', 'Work', 'work', 'アイデア'])).toEqual([
      '仕事',
      'Work',
      'アイデア',
    ]);
  });
  it('validates type, empty names, controls, Unicode length and tag count', () => {
    for (const invalid of [
      null,
      '仕事',
      [1],
      ['  '],
      ['a\nb'],
      ['a\tb'],
      ['あ'.repeat(31)],
      Array.from({ length: 21 }, (_, index) => `tag${index}`),
    ]) {
      expect(() => normalizeTags(invalid)).toThrow();
    }
    expect(normalizeTags(['😀'.repeat(30)])).toHaveLength(1);
    expect(normalizeTags([])).toEqual([]);
  });
  it('suggests deduplicated tags from live notes only', () => {
    const notes = [
      { ...newNote(), tags: ['仕事', 'Work'] },
      { ...newNote(), tags: ['work', '個人'] },
      { ...newNote(), tags: ['削除済み'], deleted_at: 'today' },
    ];
    expect(new Set(availableTags(notes))).toEqual(new Set(['仕事', 'Work', '個人']));
  });
});
