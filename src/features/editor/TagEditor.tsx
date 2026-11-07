import { useEffect, useId, useRef, useState } from 'react';
import { Plus, Tags, X } from 'lucide-react';
import { Button } from '../../components/ui/button';
import {
  availableTags,
  MAX_NOTE_TAGS,
  MAX_TAG_LENGTH,
  normalizeTags,
  tagKey,
} from '../../lib/tags';
import { useApp } from '../../stores/app';
import { errorMessage } from '../../types';

export function TagEditor({
  tags,
  onChange,
  open,
  onOpenChange,
}: {
  tags: string[];
  onChange: (tags: string[]) => void;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const notes = useApp((state) => state.notes);
  const suggestions = availableTags(notes).filter(
    (tag) => !tags.some((value) => tagKey(value) === tagKey(tag)),
  );
  const [input, setInput] = useState('');
  const [error, setError] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const wasOpen = useRef(false);
  const id = useId();
  useEffect(() => {
    if (open) inputRef.current?.focus();
    else if (wasOpen.current) triggerRef.current?.focus();
    wasOpen.current = open;
  }, [open]);
  const close = () => {
    onOpenChange(false);
    setInput('');
    setError('');
    triggerRef.current?.focus();
  };
  const add = (value: string) => {
    try {
      const existing = availableTags(notes).find((tag) => tagKey(tag) === tagKey(value));
      const next = normalizeTags([...tags, existing ?? value]);
      onChange(next);
      setInput('');
      setError('');
      inputRef.current?.focus();
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  return (
    <div className="editor-tag-controls">
      <button
        ref={triggerRef}
        type="button"
        className="button ghost tag-trigger"
        aria-label="タグを編集"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => {
          if (open) close();
          else {
            setInput('');
            setError('');
            onOpenChange(true);
          }
        }}
      >
        <Tags size={16} /> タグ{tags.length > 0 && ` (${tags.length})`}
      </button>
      {!open &&
        tags.slice(0, 2).map((tag) => (
          <span className="tag-chip" key={tag} title={tag}>
            {tag}
          </span>
        ))}
      {!open && tags.length > 2 && <span className="tag-overflow">+{tags.length - 2}</span>}
      {open && (
        <section className="tag-editor-panel" id={id} aria-label="メモのタグ">
          <div className="tag-panel-heading">
            <strong>タグ</strong>
            <Button variant="ghost" aria-label="タグ編集を閉じる" onClick={close}>
              <X size={16} />
            </Button>
          </div>
          <div className="tag-list" aria-label="付いているタグ">
            {tags.length ? (
              tags.map((tag) => (
                <span className="tag-chip" key={tag}>
                  <span title={tag}>{tag}</span>
                  <button
                    aria-label={`タグ「${tag}」を外す`}
                    onClick={() => onChange(tags.filter((value) => value !== tag))}
                  >
                    <X size={13} />
                  </button>
                </span>
              ))
            ) : (
              <p className="tag-hint">タグはまだありません。</p>
            )}
          </div>
          <form
            className="tag-add-form"
            onSubmit={(event) => {
              event.preventDefault();
              if (input.trim()) add(input);
            }}
          >
            <input
              ref={inputRef}
              aria-label="追加するタグ"
              aria-describedby={`${id}-hint${error ? ` ${id}-error` : ''}`}
              aria-invalid={!!error}
              placeholder="新しいタグを入力…"
              value={input}
              onChange={(event) => {
                setInput(event.target.value);
                setError('');
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && event.nativeEvent.isComposing) event.preventDefault();
              }}
            />
            <Button
              type="submit"
              variant="outline"
              disabled={!input.trim()}
              aria-label="タグを追加"
            >
              <Plus size={16} /> 追加
            </Button>
          </form>
          <p className="tag-hint" id={`${id}-hint`}>
            1タグ{MAX_TAG_LENGTH}文字・1メモ{MAX_NOTE_TAGS}個まで
          </p>
          {error && (
            <p role="alert" className="tag-error" id={`${id}-error`}>
              {error}
            </p>
          )}
          {suggestions.length > 0 && (
            <div className="tag-suggestions">
              <span className="tag-hint">既存のタグ</span>
              <div className="tag-list">
                {suggestions
                  .filter((tag) => tagKey(tag).includes(tagKey(input)))
                  .map((tag) => (
                    <button
                      className="tag-chip tag-suggestion"
                      type="button"
                      key={tag}
                      onClick={() => add(tag)}
                      aria-label={`タグ「${tag}」を追加`}
                    >
                      <Plus size={12} /> {tag}
                    </button>
                  ))}
              </div>
            </div>
          )}
        </section>
      )}
    </div>
  );
}
