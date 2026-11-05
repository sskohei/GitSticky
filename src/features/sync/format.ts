import { parseDocument, stringify } from 'yaml';
import { AppError, colors, type Note } from '../../types';
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export function managedId(path: string) {
  const match = /^notes\/([^/]+)\.md$/.exec(path);
  return match && uuid.test(match[1]) ? match[1] : null;
}
export function serialize(note: Note) {
  return (
    '---\n' +
    stringify(
      {
        schemaVersion: 1,
        id: note.id,
        title: note.title,
        color: note.color,
        pinned: note.pinned,
        createdAt: note.created_at,
        updatedAt: note.updated_at,
      },
      { lineWidth: 0 },
    ) +
    '---\n' +
    note.body_md
  );
}
export function parse(content: string, id: string): Note {
  const fail = () => {
    throw new AppError(
      'invalid_markdown',
      `メモ ${id} のFront Matterが不正です。ファイルは変更していません。`,
    );
  };
  const match = /^\uFEFF?---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)([\s\S]*)$/.exec(content);
  if (!match || content.length > 5_000_000) return fail();
  try {
    const doc = parseDocument(match[1], { uniqueKeys: true });
    if (doc.errors.length) return fail();
    const m: unknown = doc.toJS({ maxAliasCount: 0 });
    if (!m || typeof m !== 'object' || Array.isArray(m)) return fail();
    const v = m as Record<string, unknown>;
    if (
      v.schemaVersion !== 1 ||
      v.id !== id ||
      !uuid.test(id) ||
      typeof v.title !== 'string' ||
      !v.title.trim() ||
      Array.from(v.title).length > 80 ||
      !colors.includes(v.color as Note['color']) ||
      typeof v.pinned !== 'boolean' ||
      typeof v.createdAt !== 'string' ||
      typeof v.updatedAt !== 'string' ||
      !Number.isFinite(Date.parse(v.createdAt)) ||
      !Number.isFinite(Date.parse(v.updatedAt))
    )
      return fail();
    return {
      id,
      title: v.title,
      body_md: match[2],
      color: v.color as Note['color'],
      pinned: v.pinned,
      created_at: v.createdAt,
      updated_at: v.updatedAt,
      deleted_at: null,
      local_revision: 1,
      dirty: false,
    };
  } catch {
    return fail();
  }
}
export function same(a: Note, b: Note) {
  return (
    a.title === b.title &&
    a.body_md === b.body_md &&
    a.color === b.color &&
    a.pinned === b.pinned &&
    a.created_at === b.created_at &&
    a.updated_at === b.updated_at &&
    !a.deleted_at &&
    !b.deleted_at
  );
}
