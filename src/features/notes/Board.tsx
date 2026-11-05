import { Pin, Plus, StickyNote } from 'lucide-react';
import type { Note } from '../../types';
import { selectNotes } from '../../lib/notes';
export function Board({
  notes,
  query,
  onOpen,
  onCreate,
}: {
  notes: Note[];
  query: string;
  onOpen: (note: Note) => void;
  onCreate: () => void;
}) {
  const visible = selectNotes(notes, query);
  return (
    <>
      <div className="board-label">
        <span>
          {query ? '検索結果' : 'すべてのメモ'} <b>{visible.length}</b>
        </span>
        <span>小さなひらめきを、大切に。</span>
      </div>
      <div className="note-grid">
        {visible.map((n) => (
          <button className={'note-card note-' + n.color} key={n.id} onClick={() => onOpen(n)}>
            <div className="note-card-top">
              <span className="note-fold" />
              {n.pinned && <Pin size={17} aria-label="ピン留め済み" />}
            </div>
            <h2>{n.title}</h2>
            <footer>
              <time dateTime={n.updated_at}>
                {new Date(n.updated_at).toLocaleDateString('ja-JP', {
                  month: 'short',
                  day: 'numeric',
                })}
              </time>
              {n.dirty && <span className="pending-dot" aria-label="未同期" />}
            </footer>
          </button>
        ))}
        {!query && (
          <button className="new-card" onClick={onCreate}>
            <span className="plus-circle">
              <Plus size={25} />
            </span>
            <span>新しいメモ</span>
          </button>
        )}
      </div>
      {!visible.length && (
        <div className="empty">
          <StickyNote size={32} />
          <h2>{query ? 'メモが見つかりません' : '思いついたら、ひとつメモ。'}</h2>
          <p>
            {query
              ? '別の言葉で検索してみてください。'
              : '新しいメモから、あなたのノートを始めましょう。'}
          </p>
        </div>
      )}
    </>
  );
}
