import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { isTauri } from '@tauri-apps/api/core';
import { App } from './App';
import { NativeDatabase } from './db';
import './styles.css';
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {isTauri() ? (
      <App db={new NativeDatabase()} />
    ) : (
      <div className="browser-message">
        <h1>StickyNote.</h1>
        <p>メモの保存と同期にはTauriアプリを起動してください。</p>
        <code>pnpm tauri dev</code>
        <p>ブラウザ単体ではSQLiteや保管庫にアクセスできません。</p>
      </div>
    )}
  </StrictMode>,
);
