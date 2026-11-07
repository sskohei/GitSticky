import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from 'react';
import CodeMirror from '@uiw/react-codemirror';
import { markdown } from '@codemirror/lang-markdown';
import { EditorView } from '@codemirror/view';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { X, Pin, Trash2, Eye, Pencil, Columns2, Check, Loader2 } from 'lucide-react';
import { Modal } from '../../components/Modal';
import { Button } from '../../components/ui/button';
import { TagEditor } from './TagEditor';
import type { LocalDatabase } from '../../db';
import { title } from '../../lib/notes';
import { useApp } from '../../stores/app';
import { colors, errorMessage, type Note } from '../../types';
export interface EditorHandle {
  flush: () => Promise<void>;
}
const extensions = [markdown(), EditorView.lineWrapping];
export const Editor = forwardRef<
  EditorHandle,
  { note: Note; db: LocalDatabase; onClose: () => void }
>(function Editor({ note, db, onClose }, ref) {
  const [draft, setDraft] = useState(note);
  const current = useRef(note);
  const generation = useRef(0);
  const saved = useRef(0);
  const queue = useRef(Promise.resolve());
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [state, setState] = useState('ローカル保存済み');
  const [error, setError] = useState('');
  const [view, setView] = useState<'edit' | 'split' | 'preview'>('split');
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [tagsOpen, setTagsOpen] = useState(false);
  const [closing, setClosing] = useState(false);
  const flush = useCallback(async () => {
    if (timer.current) clearTimeout(timer.current);
    const work = async () => {
      if (saved.current === generation.current) return;
      setState('保存中');
      try {
        // Closing must also retain input entered while the preceding write was pending.
        do {
          const version = generation.current;
          const value = {
            ...current.current,
            title: title(current.current.title),
            updated_at: new Date().toISOString(),
          };
          await db.save(value);
          saved.current = version;
        } while (saved.current !== generation.current);
        setError('');
        setState(saved.current === generation.current ? 'ローカル保存済み' : '未保存');
        await useApp.getState().refresh(db);
      } catch (e) {
        setError(errorMessage(e));
        setState('保存エラー');
        throw e;
      }
    };
    queue.current = queue.current.catch(() => {}).then(work);
    await queue.current;
  }, [db]);
  useImperativeHandle(ref, () => ({ flush }), [flush]);
  const persisted = useApp((s) => s.notes.find((n) => n.id === note.id));
  useEffect(() => {
    if (!persisted || saved.current !== generation.current) return;
    current.current = persisted;
    setDraft(persisted);
  }, [persisted]);
  const update = (patch: Partial<Note>) => {
    current.current = { ...current.current, ...patch };
    generation.current++;
    setDraft(current.current);
    setState('未保存');
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      void flush().catch(() => {});
    }, 500);
  };
  const close = useCallback(async () => {
    setClosing(true);
    try {
      await flush();
      onClose();
    } catch {
      setClosing(false);
      if (/Android/i.test(navigator.userAgent) && !history.state?.stickyEditor) {
        history.pushState({ stickyEditor: note.id }, '');
      }
    }
  }, [flush, onClose, note.id]);
  useEffect(() => {
    if (!/Android/i.test(navigator.userAgent)) return;
    let active = true;
    let pushed = false;
    queueMicrotask(() => {
      if (!active) return;
      history.pushState({ stickyEditor: note.id }, '');
      pushed = true;
    });
    const back = () => {
      void close();
    };
    window.addEventListener('popstate', back);
    return () => {
      active = false;
      window.removeEventListener('popstate', back);
      if (pushed && history.state?.stickyEditor === note.id) history.back();
    };
  }, [close, note.id]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
        event.preventDefault();
        void flush().catch(() => {});
      }
    };
    const hide = () => {
      if (document.visibilityState === 'hidden') void flush().catch(() => {});
    };
    const unload = (event: BeforeUnloadEvent) => {
      if (saved.current !== generation.current) {
        event.preventDefault();
      }
    };
    window.addEventListener('keydown', key);
    document.addEventListener('visibilitychange', hide);
    window.addEventListener('beforeunload', unload);
    return () => {
      if (timer.current) clearTimeout(timer.current);
      window.removeEventListener('keydown', key);
      document.removeEventListener('visibilitychange', hide);
      window.removeEventListener('beforeunload', unload);
    };
  }, [flush]);
  const remove = async () => {
    update({ deleted_at: new Date().toISOString() });
    await close();
  };
  return (
    <Modal
      title="メモを編集"
      onEscapeKeyDown={(event) => {
        if (tagsOpen) {
          event.preventDefault();
          setTagsOpen(false);
        }
      }}
      onClose={() => {
        void close();
      }}
      className={'editor note-' + draft.color}
    >
      <div className="editor-top">
        <span className="eyebrow">YOUR LITTLE SPACE</span>
        <Button
          variant="ghost"
          aria-label="閉じる"
          disabled={closing}
          onClick={() => {
            void close();
          }}
        >
          <X size={20} />
        </Button>
      </div>
      <input
        className="title-input"
        aria-label="メモのタイトル"
        value={draft.title}
        maxLength={80}
        onChange={(e) => update({ title: e.target.value })}
      />
      <div className="editor-toolbar">
        <div className="editor-tools-left">
          <div className="colors" aria-label="付箋の色">
            {colors.map((c) => (
              <button
                key={c}
                className={'swatch note-' + c}
                aria-label={c}
                aria-pressed={draft.color === c}
                onClick={() => update({ color: c })}
              >
                {draft.color === c && <Check size={15} />}
              </button>
            ))}
          </div>
          <TagEditor
            tags={draft.tags}
            onChange={(tags) => update({ tags })}
            open={tagsOpen}
            onOpenChange={setTagsOpen}
          />
        </div>
        <div className="row">
          <Button
            variant="ghost"
            aria-label="ピン留め"
            aria-pressed={draft.pinned}
            onClick={() => update({ pinned: !draft.pinned })}
          >
            <Pin size={18} fill={draft.pinned ? 'currentColor' : 'none'} />
          </Button>
        </div>
      </div>
      <div className="editor-viewbar">
        <div className="editor-modes" role="group" aria-label="エディタの表示方法">
          <Button
            variant="ghost"
            aria-label="編集のみ"
            aria-pressed={view === 'edit'}
            onClick={() => setView('edit')}
          >
            <Pencil size={15} /> 編集
          </Button>
          <Button
            variant="ghost"
            aria-label="編集とプレビュー"
            aria-pressed={view === 'split'}
            onClick={() => setView('split')}
          >
            <Columns2 size={15} /> 分割
          </Button>
          <Button
            variant="ghost"
            aria-label="プレビューのみ"
            aria-pressed={view === 'preview'}
            onClick={() => setView('preview')}
          >
            <Eye size={15} /> プレビュー
          </Button>
        </div>
        <span className="editor-count">{draft.body_md.length.toLocaleString('ja-JP')} 文字</span>
      </div>
      <div className={'editor-body editor-view-' + view}>
        <section
          className="editor-pane source-pane"
          aria-label="Markdown編集"
          hidden={view === 'preview'}
        >
          <div className="editor-pane-heading">
            <span>MARKDOWN</span>
            <span>編集</span>
          </div>
          <div className="editor-pane-content">
            <CodeMirror
              aria-label="Markdown本文"
              value={draft.body_md}
              extensions={extensions}
              theme={document.documentElement.classList.contains('dark') ? 'dark' : 'light'}
              onChange={(value) => update({ body_md: value })}
              basicSetup={{ lineNumbers: false, foldGutter: false, highlightActiveLine: false }}
              placeholder="ここから、思いついたことを。Markdownが使えます。"
            />
          </div>
        </section>
        <section
          className="editor-pane preview-pane"
          aria-label="Markdownプレビュー"
          hidden={view === 'edit'}
        >
          <div className="editor-pane-heading">
            <span>PREVIEW</span>
            <span>入力に合わせて更新</span>
          </div>
          <div className="editor-pane-content markdown">
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              skipHtml
              components={{
                img: () => <span>［外部画像］</span>,
                a: ({ children, href }) => (
                  <a href={href} target="_blank" rel="noopener noreferrer">
                    {children}
                  </a>
                ),
              }}
            >
              {draft.body_md || '本文を書き始めると、ここにプレビューが表示されます。'}
            </ReactMarkdown>
          </div>
        </section>
      </div>
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      <footer className="editor-footer">
        <span className="save-state" role="status">
          {state === '保存中' ? <Loader2 size={14} className="spin" /> : <Check size={14} />}{' '}
          {state}
          <small>GitHub同期とは別の保存状態です</small>
        </span>
        {confirmDelete ? (
          <div className="row">
            <span>このメモを削除？</span>
            <Button variant="ghost" onClick={() => setConfirmDelete(false)}>
              戻る
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                void remove();
              }}
            >
              削除する
            </Button>
          </div>
        ) : (
          <Button variant="ghost" aria-label="メモを削除" onClick={() => setConfirmDelete(true)}>
            <Trash2 size={18} />
          </Button>
        )}
      </footer>
    </Modal>
  );
});
