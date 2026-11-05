import { type Note } from '../types';
export function title(value: string) {
  return Array.from(value.trim() || '無題のメモ')
    .slice(0, 80)
    .join('');
}
export function newNote(): Note {
  const now = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    title: '無題のメモ',
    body_md: '',
    color: 'yellow',
    pinned: false,
    created_at: now,
    updated_at: now,
    deleted_at: null,
    local_revision: 1,
    dirty: true,
  };
}
export function selectNotes(notes: Note[], query: string) {
  const q = query.toLocaleLowerCase();
  return notes
    .filter((n) => !n.deleted_at && (n.title + '\n' + n.body_md).toLocaleLowerCase().includes(q))
    .sort(
      (a, b) =>
        Number(b.pinned) - Number(a.pinned) ||
        b.updated_at.localeCompare(a.updated_at) ||
        a.id.localeCompare(b.id),
    );
}
