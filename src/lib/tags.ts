import { AppError, type Note } from '../types';

export const MAX_TAG_LENGTH = 30;
export const MAX_NOTE_TAGS = 20;

// ASCII case folding is shared with the native validation, including on Android.
export function tagKey(value: string) {
  return value.trim().replace(/[A-Z]/g, (letter) => letter.toLowerCase());
}

export function normalizeTags(value: unknown): string[] {
  if (!Array.isArray(value)) throw new AppError('validation', 'タグの形式が不正です。');
  const unique = new Map<string, string>();
  for (const item of value) {
    if (typeof item !== 'string') throw new AppError('validation', 'タグの形式が不正です。');
    const tag = item.trim();
    if (!tag || Array.from(tag).length > MAX_TAG_LENGTH || /\p{Cc}/u.test(tag)) {
      throw new AppError(
        'validation',
        `タグは改行を含まない1〜${MAX_TAG_LENGTH}文字で入力してください。`,
      );
    }
    if (!unique.has(tagKey(tag))) unique.set(tagKey(tag), tag);
  }
  if (unique.size > MAX_NOTE_TAGS) {
    throw new AppError('validation', `1つのメモに付けられるタグは${MAX_NOTE_TAGS}個までです。`);
  }
  return [...unique.values()];
}

export function availableTags(notes: Note[]) {
  const unique = new Map<string, string>();
  for (const note of notes) {
    if (note.deleted_at) continue;
    for (const tag of note.tags) {
      if (!unique.has(tagKey(tag))) unique.set(tagKey(tag), tag);
    }
  }
  return [...unique.values()].sort((a, b) => a.localeCompare(b, 'ja'));
}
