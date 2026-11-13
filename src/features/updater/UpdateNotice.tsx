import { useEffect, useRef, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { Button } from '../../components/ui/button';
import { Modal } from '../../components/Modal';
import { native } from '../../lib/native';

export const UPDATE_INTERVAL = 60 * 60 * 1000;

export function UpdateNotice({
  disabled,
  flush,
  onInstalling,
}: {
  disabled: boolean;
  flush: () => Promise<void>;
  onInstalling: (installing: boolean) => void;
}) {
  const [update, setUpdate] = useState<Update | null>(null);
  const [installing, setInstalling] = useState(false);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const busy = useRef(false);
  const current = useRef<Update | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let checking = false;
    let enabled = false;
    const poll = async () => {
      if (!enabled || disposed || checking || busy.current || current.current || !navigator.onLine)
        return;
      checking = true;
      try {
        const available = await check({ timeout: 15000 });
        if (disposed) await available?.close();
        else if (available) {
          current.current = available;
          setUpdate(available);
        }
      } catch {
        // Offline, missing releases, and temporary server failures retry on the next check.
      } finally {
        checking = false;
      }
    };
    void native<boolean>('app_update_supported')
      .then((supported) => {
        enabled = supported === true;
        void poll();
      })
      .catch(() => {});
    const timer = setInterval(() => void poll(), UPDATE_INTERVAL);
    const resume = () => {
      if (document.visibilityState === 'visible') void poll();
    };
    window.addEventListener('online', resume);
    document.addEventListener('visibilitychange', resume);
    return () => {
      disposed = true;
      clearInterval(timer);
      window.removeEventListener('online', resume);
      document.removeEventListener('visibilitychange', resume);
      const resource = current.current;
      current.current = null;
      void resource?.close().catch(() => {});
    };
  }, []);

  const install = async () => {
    if (!update || disabled || busy.current) return;
    busy.current = true;
    setInstalling(true);
    onInstalling(true);
    setError('');
    setStatus('メモを保存しています…');
    try {
      await flush();
      setStatus('更新をダウンロードしています…');
      let total = 0;
      let downloaded = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') total = event.data.contentLength ?? 0;
        if (event.event === 'Progress') {
          downloaded += event.data.chunkLength;
          if (total > 0)
            setStatus(
              `更新をダウンロードしています… ${Math.min(100, Math.floor((downloaded / total) * 100))}%`,
            );
        }
        if (event.event === 'Finished') setStatus('更新をインストールしています…');
      });
      // On Windows the updater exits the app and the NSIS installer relaunches it.
    } catch {
      setError('更新できませんでした。通信とメモの保存状態を確認して、もう一度お試しください。');
      setInstalling(false);
      onInstalling(false);
      busy.current = false;
    }
  };

  if (!update) return null;
  return (
    <>
      <aside className="update-notice" aria-label="アプリの更新">
        <div role="status">
          <strong>新しいバージョン {update.version} が利用できます</strong>
          <p>更新するとアプリが再起動します。</p>
          {error && <p role="alert">{error}</p>}
        </div>
        <Button disabled={disabled || installing} onClick={() => void install()}>
          更新する
        </Button>
      </aside>
      {installing && (
        <Modal
          title="アプリを更新"
          onClose={() => {}}
          onEscapeKeyDown={(event) => event.preventDefault()}
          className="update-progress"
        >
          <h2>アプリを更新しています</h2>
          <p role="status">{status}</p>
          <p>完了するとアプリが再起動します。このままお待ちください。</p>
        </Modal>
      )}
    </>
  );
}
