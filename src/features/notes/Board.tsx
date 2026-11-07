import { Pin, Plus, StickyNote } from 'lucide-react';
import { useState } from 'react';
import type { Note } from '../../types';
import { selectNotes } from '../../lib/notes';
import { availableTags, MAX_NOTE_TAGS, tagKey } from '../../lib/tags';
export function Board({
  notes,
  query,
  onOpen,
  onCreate,
  creating = false,
}: {
  notes: Note[];
  query: string;
  onOpen: (note: Note) => void;
  onCreate: (tags: string[]) => void;
  creating?: boolean;
}) {
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [untagged, setUntagged] = useState(false);
  const filtered = selectedTags.length > 0 || untagged;
  const visible = selectNotes(notes, query, { tags: selectedTags, untagged });
  const tags = availableTags(notes);
  const tooManyTags = selectedTags.length > MAX_NOTE_TAGS;
  // Keep a selected filter removable even if its last note is edited or deleted.
  const choices = [
    ...tags,
    ...selectedTags.filter((tag) => !tags.some((value) => tagKey(value) === tagKey(tag))),
  ];
  const clearFilters = () => {
    setSelectedTags([]);
    setUntagged(false);
  };
  return (
    <>
      <div className="board-tag-filters" role="group" aria-label="タグで絞り込み">
        <button className="tag-filter" aria-pressed={!filtered} onClick={clearFilters}>
          すべて
        </button>
        <button
          className="tag-filter"
          aria-pressed={untagged}
          onClick={() => {
            setUntagged(!untagged);
            setSelectedTags([]);
          }}
        >
          タグなし
        </button>
        {choices.map((tag) => {
          const selected = selectedTags.some((value) => tagKey(value) === tagKey(tag));
          const count = notes.filter(
            (note) => !note.deleted_at && note.tags.some((value) => tagKey(value) === tagKey(tag)),
          ).length;
          return (
            <button
              key={tagKey(tag)}
              className="tag-filter"
              aria-label={`タグ「${tag}」で絞り込み`}
              aria-pressed={selected}
              onClick={() => {
                setUntagged(false);
                setSelectedTags(
                  selected
                    ? selectedTags.filter((value) => tagKey(value) !== tagKey(tag))
                    : [...selectedTags, tag],
                );
              }}
            >
              <span>{tag}</span>
              <small>{count}</small>
            </button>
          );
        })}
      </div>
      {selectedTags.length > 1 && (
        <p className="tag-filter-description">選択したタグがすべて付いたメモを表示しています。</p>
      )}
      {tooManyTags && (
        <p className="tag-filter-description" id="create-tag-limit">
          メモを作成するには、選択するタグを{MAX_NOTE_TAGS}個以下にしてください。
        </p>
      )}
      <div className="board-label">
        <span>
          {query || filtered ? '検索結果' : 'すべてのメモ'} <b>{visible.length}</b>
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
            {n.tags.length > 0 && (
              <div className="note-tags" aria-label="タグ">
                {n.tags.slice(0, 2).map((tag) => (
                  <span className="tag-chip" key={tagKey(tag)} title={tag}>
                    {tag}
                  </span>
                ))}
                {n.tags.length > 2 && (
                  <span className="tag-overflow" title={n.tags.slice(2).join('、')}>
                    +{n.tags.length - 2}
                  </span>
                )}
              </div>
            )}
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
        <button
          className="new-card"
          disabled={creating || tooManyTags}
          aria-describedby={tooManyTags ? 'create-tag-limit' : undefined}
          onClick={() => onCreate([...selectedTags])}
        >
          <span className="plus-circle">
            <Plus size={25} />
          </span>
          <span>
            {selectedTags.length === 0
              ? '新しいメモ'
              : selectedTags.length === 1
                ? 'このタグで新しいメモ'
                : '選択中のタグで新しいメモ'}
          </span>
        </button>
      </div>
      {!visible.length && (
        <div className="empty">
          <StickyNote size={32} />
          <h2>{query || filtered ? 'メモが見つかりません' : '思いついたら、ひとつメモ。'}</h2>
          <p>
            {query || filtered
              ? '検索する言葉や選択したタグを変更してみてください。'
              : '新しいメモから、あなたのノートを始めましょう。'}
          </p>
        </div>
      )}
    </>
  );
}
