import { useCallback, useEffect, useRef, useState } from 'react';
import type { LocalDatabase } from '../../db';
import { useApp } from '../../stores/app';
import { useAuth } from '../../stores/auth';
import { AppError, errorMessage } from '../../types';
import { SyncEngine } from './engine';
import { github } from './github';
export function useSync(
  db: LocalDatabase,
  unlocked: boolean,
  flush: () => Promise<void>,
  paused = false,
) {
  const engine = useRef(new SyncEngine(db, github));
  const settings = useApp((s) => s.settings);
  const notes = useApp((s) => s.notes);
  const [status, setStatus] = useState('未接続');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const retryAt = useRef(0);
  const sync = useCallback(async () => {
    if (busyRef.current || paused) return;
    const settings = useApp.getState().settings;
    if (!navigator.onLine) {
      setStatus('オフライン');
      return;
    }
    if (!settings.repository_owner || !unlocked) {
      setStatus(settings.repository_owner ? 'GitHubの接続が必要' : '未接続');
      return;
    }
    if (Date.now() < retryAt.current) {
      setStatus('同期エラー');
      setMessage('API制限の解除までお待ちください。');
      return;
    }
    busyRef.current = true;
    setBusy(true);
    setStatus('同期中');
    setMessage('');
    try {
      await flush();
      const r = await engine.current.run(settings);
      await useApp.getState().refresh(db);
      setStatus(r.errors.length ? '同期エラー' : r.conflicts ? '編集競合あり' : '同期済み');
      setMessage(
        r.errors.join('\n') ||
          (r.conflicts
            ? `${r.conflicts}件の競合を保護しました。競合コピーと元のメモを確認してください。`
            : ''),
      );
    } catch (e) {
      if (e instanceof AppError && e.code === 'auth') {
        await useAuth
          .getState()
          .refresh()
          .catch(() => {});
      }
      setStatus(e instanceof AppError && e.code === 'network' ? 'オフライン' : '同期エラー');
      setMessage(errorMessage(e));
      if (e instanceof AppError && e.retry_at) retryAt.current = e.retry_at;
      await useApp
        .getState()
        .refresh(db)
        .catch(() => {});
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }, [db, unlocked, flush, paused]);
  useEffect(() => {
    if (paused || !settings.auto_sync_enabled || !notes.some((n) => n.dirty)) return;
    const timer = setTimeout(() => {
      void sync();
    }, 15000);
    return () => clearTimeout(timer);
  }, [notes, settings.auto_sync_enabled, sync, paused]);
  useEffect(() => {
    if (paused || !settings.auto_sync_enabled) return;
    const resume = () => {
      if (document.visibilityState === 'visible') void sync();
    };
    const online = () => {
      void sync();
    };
    const offline = () => setStatus('オフライン');
    void sync();
    document.addEventListener('visibilitychange', resume);
    window.addEventListener('online', online);
    window.addEventListener('offline', offline);
    return () => {
      document.removeEventListener('visibilitychange', resume);
      window.removeEventListener('online', online);
      window.removeEventListener('offline', offline);
    };
  }, [
    settings.auto_sync_enabled,
    settings.repository_owner,
    settings.repository_name,
    settings.branch_name,
    sync,
    paused,
  ]);
  return { sync, status, message, busy };
}
