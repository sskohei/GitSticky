import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { StickyNote, Search, RefreshCw, Settings, Plus, Cloud, X } from 'lucide-react';
import { Button } from './components/ui/button';
import type { LocalDatabase } from './db';
import { Board } from './features/notes/Board';
import { type EditorHandle } from './features/editor/Editor';
import { useSync } from './features/sync/useSync';
import { UpdateNotice } from './features/updater/UpdateNotice';
import { newNote } from './lib/notes';
import { useApp } from './stores/app';
import { useAuth } from './stores/auth';
import { errorMessage, type Note } from './types';
const Editor = lazy(() => import('./features/editor/Editor').then((m) => ({ default: m.Editor })));
const SettingsDialog = lazy(() =>
  import('./features/settings/Settings').then((m) => ({ default: m.SettingsDialog })),
);
export function App({ db }: { db: LocalDatabase }) {
  const { notes, settings, query, setQuery, refresh } = useApp();
  const [editor, setEditor] = useState<Note | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const auth = useAuth();
  const unlocked = auth.status.connected;
  const [error, setError] = useState('');
  const [ready, setReady] = useState(false);
  const [creating, setCreating] = useState(false);
  const [installing, setInstalling] = useState(false);
  const editorRef = useRef<EditorHandle>(null);
  const flush = useCallback(async () => {
    await editorRef.current?.flush();
  }, []);
  const closeEditor = useCallback(() => setEditor(null), []);
  const sync = useSync(db, unlocked, flush, showSettings || installing);
  useEffect(() => {
    void useAuth
      .getState()
      .refresh()
      .catch(() => {});
    void refresh(db)
      .then(() => setReady(true))
      .catch((e) => setError(errorMessage(e)));
  }, [db, refresh]);
  useEffect(() => {
    const media = matchMedia('(prefers-color-scheme: dark)');
    const theme = () =>
      document.documentElement.classList.toggle(
        'dark',
        settings.theme === 'dark' || (settings.theme === 'system' && media.matches),
      );
    theme();
    media.addEventListener('change', theme);
    return () => media.removeEventListener('change', theme);
  }, [settings.theme]);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onCloseRequested(async (event) => {
        try {
          // Tauri awaits this handler before destroying the window.
          await flush();
        } catch (e) {
          event.preventDefault();
          setError(errorMessage(e));
        }
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [flush]);
  const create = async (tags: string[] = []) => {
    if (creating) return;
    setCreating(true);
    try {
      const n = newNote(tags);
      await db.save(n);
      await refresh(db);
      setEditor(n);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setCreating(false);
    }
  };
  const pending = notes.filter((n) => n.dirty).length;
  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand">
          <span className="brand-mark">
            <StickyNote size={24} />
          </span>
          <div>
            <h1>
              StickyNote<span>.</span>
            </h1>
            <p>アイデアに、居場所を。</p>
          </div>
        </div>
        <div className="header-actions">
          <Button
            variant="ghost"
            aria-label="GitHubと同期"
            disabled={!ready || sync.busy}
            onClick={() => {
              void sync.sync();
            }}
          >
            <RefreshCw size={19} className={sync.busy ? 'spin' : ''} />
            <span className="desktop-label">同期</span>
          </Button>
          <Button
            variant="ghost"
            aria-label="設定"
            disabled={!ready || sync.busy}
            onClick={() => setShowSettings(true)}
          >
            <Settings size={21} />
          </Button>
        </div>
      </header>
      <main>
        <UpdateNotice
          disabled={!ready || sync.busy || creating || showSettings || !!editor}
          flush={flush}
          onInstalling={setInstalling}
        />
        <section className="intro">
          <div>
            <span className="eyebrow">A PLACE FOR YOUR THOUGHTS</span>
            <h2>わたしのメモ</h2>
          </div>
          <Button
            disabled={!ready || creating}
            onClick={() => {
              void create();
            }}
          >
            <Plus size={19} />
            新しいメモ
          </Button>
        </section>
        <div className="board-tools">
          <label className="search">
            <Search size={19} />
            <input
              aria-label="メモを検索"
              placeholder="メモを検索…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            {query && (
              <button aria-label="検索をクリア" onClick={() => setQuery('')}>
                <X size={16} />
              </button>
            )}
          </label>
          <div className="sync-status">
            <Cloud size={17} />
            <span>
              {sync.busy
                ? sync.status
                : !navigator.onLine
                  ? 'オフライン'
                  : sync.status === '編集競合あり' || sync.status === '同期エラー'
                    ? sync.status
                    : pending
                      ? '未同期変更あり'
                      : sync.status}
              <small>
                {pending}件未同期 ·{' '}
                {settings.last_successful_sync_at
                  ? '最終同期 ' + new Date(settings.last_successful_sync_at).toLocaleString('ja-JP')
                  : '最終同期なし'}
              </small>
            </span>
          </div>
        </div>
        {(error || sync.message) && (
          <div className="notice" role="alert">
            {error || sync.message}
          </div>
        )}
        {ready ? (
          <Board
            notes={notes}
            query={query}
            onOpen={setEditor}
            creating={creating}
            onCreate={(tags) => {
              void create(tags);
            }}
          />
        ) : (
          <p className="empty">
            {error ? '起動できませんでした。再起動して再試行してください。' : 'メモを開いています…'}
          </p>
        )}
      </main>
      <footer className="app-footer">
        <span>あなたのメモは、この端末に保存されます。</span>
        <span>Made for little ideas.</span>
      </footer>
      <Suspense
        fallback={
          <div className="notice" role="status">
            画面を準備しています…
          </div>
        }
      >
        {editor && (
          <Editor key={editor.id} ref={editorRef} note={editor} db={db} onClose={closeEditor} />
        )}
        {showSettings && <SettingsDialog db={db} onClose={() => setShowSettings(false)} />}
      </Suspense>
    </div>
  );
}
