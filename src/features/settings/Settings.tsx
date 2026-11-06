import { useCallback, useEffect, useRef, useState } from 'react';
import { X, Github, Check, RefreshCw, ExternalLink } from 'lucide-react';
import { Modal } from '../../components/Modal';
import { Button } from '../../components/ui/button';
import type { LocalDatabase } from '../../db';
import { useApp } from '../../stores/app';
import { useAuth } from '../../stores/auth';
import { AppError, connectionId, errorMessage, type Settings } from '../../types';
import { native } from '../../lib/native';

interface DeviceSession {
  session_id: string;
  user_code: string;
  verification_uri: string;
  expires_at: number;
  interval_ms: number;
}
interface PollStatus {
  connected: boolean;
  retry_after_ms: number;
}
interface Repository {
  id: number;
  owner: string;
  name: string;
  default_branch: string;
}
export function SettingsDialog({ db, onClose }: { db: LocalDatabase; onClose: () => void }) {
  const settings = useApp((s) => s.settings);
  const auth = useAuth();
  const [form, setForm] = useState(settings);
  const [repositories, setRepositories] = useState<Repository[]>([]);
  const [branches, setBranches] = useState<string[]>([]);
  const [branchesLoading, setBranchesLoading] = useState(false);
  const [branchesError, setBranchesError] = useState('');
  const [session, setSession] = useState<DeviceSession | null>(null);
  const [starting, setStarting] = useState(false);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [success, setSuccess] = useState(false);
  const [legacyPass, setLegacyPass] = useState('');
  const mounted = useRef(true);
  const attempt = useRef(0);
  const repositoryRequest = useRef(0);
  const branchRequest = useRef(0);
  const tell = (e: unknown) => {
    if (mounted.current) {
      setSuccess(false);
      setMessage(errorMessage(e));
    }
  };
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      void native('auth_cancel', { sessionId: null }).catch(() => {});
    };
  }, []);
  const loadRepositories = useCallback(async () => {
    const request = ++repositoryRequest.current;
    setLoading(true);
    try {
      const result = await native<Repository[]>('github_repositories');
      if (mounted.current && request === repositoryRequest.current) setRepositories(result);
    } catch (e) {
      if (mounted.current && request === repositoryRequest.current) {
        setSuccess(false);
        setMessage(errorMessage(e));
        await useAuth
          .getState()
          .refresh()
          .catch(() => {});
      }
    } finally {
      if (mounted.current && request === repositoryRequest.current) setLoading(false);
    }
  }, []);
  useEffect(() => {
    if (auth.status.connected) void loadRepositories();
    else {
      setRepositories([]);
      setBranches([]);
    }
  }, [auth.status.connected, loadRepositories]);
  const loadBranches = useCallback(async (value: Settings) => {
    const request = ++branchRequest.current;
    setBranches([]);
    setBranchesError('');
    setBranchesLoading(false);
    if (!value.repository_owner || !value.repository_name) return;
    setBranchesLoading(true);
    try {
      const names = await native<string[]>('github_branches', { settings: value });
      if (mounted.current && request === branchRequest.current) setBranches(names);
    } catch (e) {
      if (mounted.current && request === branchRequest.current) {
        setBranchesError(errorMessage(e));
      }
    } finally {
      if (mounted.current && request === branchRequest.current) setBranchesLoading(false);
    }
  }, []);
  useEffect(() => {
    if (auth.status.connected) void loadBranches({ ...settings });
  }, [auth.status.connected, settings, loadBranches]);
  useEffect(() => {
    if (!session) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const result = await native<PollStatus>('auth_poll', { sessionId: session.session_id });
        if (!active || !mounted.current) return;
        if (result.connected) {
          setSession(null);
          await useAuth.getState().refresh();
          setSuccess(true);
          setMessage('GitHubに接続しました。同期するリポジトリを選択してください。');
          return;
        }
        timer = setTimeout(
          () => {
            void poll();
          },
          Math.max(1000, result.retry_after_ms),
        );
      } catch (e) {
        if (!active || !mounted.current) return;
        if (e instanceof AppError && (e.code === 'network' || e.code === 'rate_limit')) {
          setMessage(errorMessage(e));
          const delay = e.retry_at
            ? Math.max(1000, e.retry_at - Date.now())
            : Math.max(5000, session.interval_ms);
          timer = setTimeout(() => {
            void poll();
          }, delay);
        } else {
          setSession(null);
          tell(e);
        }
      }
    };
    timer = setTimeout(() => {
      void poll();
    }, session.interval_ms);
    return () => {
      active = false;
      clearTimeout(timer);
      void native('auth_cancel', { sessionId: session.session_id }).catch(() => {});
    };
  }, [session]);
  const begin = async () => {
    const generation = ++attempt.current;
    setStarting(true);
    setSuccess(false);
    setMessage('');
    try {
      const value = await native<DeviceSession>('auth_begin');
      if (!mounted.current || generation !== attempt.current) {
        void native('auth_cancel', { sessionId: value.session_id }).catch(() => {});
        return;
      }
      setSession(value);
      await native('auth_open_browser', { target: 'device' }).catch((e) => {
        if (mounted.current && generation === attempt.current) tell(e);
      });
    } catch (e) {
      if (mounted.current && generation === attempt.current) tell(e);
    } finally {
      if (mounted.current && generation === attempt.current) setStarting(false);
    }
  };
  const cancel = () => {
    attempt.current++;
    setStarting(false);
    setSession(null);
    void native('auth_cancel', { sessionId: session?.session_id ?? null }).catch(tell);
    setMessage('接続をキャンセルしました。');
  };
  const selectRepository = (id: string) => {
    const repository = repositories.find((r) => String(r.id) === id);
    if (!repository) return;
    const value = {
      ...form,
      repository_owner: repository.owner,
      repository_name: repository.name,
      branch_name: repository.default_branch,
    };
    setForm(value);
    void loadBranches(value);
  };
  const save = async (test: boolean) => {
    setBusy(true);
    setSuccess(false);
    setMessage('');
    const value = {
      ...form,
      last_successful_sync_at:
        connectionId(form) === connectionId(settings) ? settings.last_successful_sync_at : null,
    };
    try {
      if (test) await native('github_test', { settings: value });
      await db.settings(value);
      await useApp.getState().refresh(db);
      setSuccess(true);
      setMessage(
        test
          ? value.auto_sync_enabled
            ? '同期先を保存しました。設定を閉じると自動同期します。'
            : '同期先を保存しました。ホーム画面から手動で同期できます。'
          : '表示・自動同期の設定を保存しました。',
      );
    } catch (e) {
      tell(e);
      if (e instanceof AppError && e.code === 'auth')
        await useAuth
          .getState()
          .refresh()
          .catch(() => {});
    } finally {
      setBusy(false);
    }
  };
  const disconnect = async () => {
    setBusy(true);
    try {
      await native('auth_disconnect');
      await useAuth.getState().refresh();
      setRepositories([]);
      setBranches([]);
      setSuccess(true);
      setMessage('この端末の接続を解除しました。メモはこの端末に残っています。');
    } catch (e) {
      tell(e);
    } finally {
      setBusy(false);
    }
  };
  const migrate = async () => {
    setBusy(true);
    try {
      await native('auth_import_legacy', { passphrase: legacyPass });
      setLegacyPass('');
      await useAuth.getState().refresh();
      setSuccess(true);
      setMessage(
        '以前の認証を端末の保護領域へ移行しました。次回からパスフレーズの入力は不要です。',
      );
    } catch (e) {
      tell(e);
    } finally {
      setBusy(false);
    }
  };
  const selected = repositories.find(
    (r) => r.owner === form.repository_owner && r.name === form.repository_name,
  );
  return (
    <Modal
      title="設定"
      onClose={() => {
        if (!busy) onClose();
      }}
      className="settings"
    >
      <header className="modal-header">
        <div>
          <span className="eyebrow">MAKE IT YOURS</span>
          <h2>設定</h2>
        </div>
        <Button variant="ghost" aria-label="設定を閉じる" disabled={busy} onClick={onClose}>
          <X size={20} />
        </Button>
      </header>
      <div className="settings-content">
        <h3>表示</h3>
        <label>
          テーマ
          <select
            value={form.theme}
            onChange={(e) => setForm({ ...form, theme: e.target.value as Settings['theme'] })}
          >
            <option value="system">システムに合わせる</option>
            <option value="light">ライト</option>
            <option value="dark">ダーク</option>
          </select>
        </label>
        <label className="checkbox">
          <input
            type="checkbox"
            checked={form.auto_sync_enabled}
            onChange={(e) => setForm({ ...form, auto_sync_enabled: e.target.checked })}
          />
          自動同期を有効にする
        </label>
        <h3>
          <Github size={19} />
          GitHubとの同期
        </h3>
        <p className="hint">
          自分のリポジトリにメモを保存します。ログイン情報は端末で保護され、次回起動時も接続を維持します。
        </p>
        {auth.error && (
          <div>
            <p role="alert" className="error">
              {auth.error}
            </p>
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => {
                void disconnect();
              }}
            >
              端末の認証をリセット
            </Button>
            <p className="hint">認証情報だけを削除します。メモは保持されます。</p>
          </div>
        )}
        {auth.status.connected ? (
          <>
            <p className="auth-connected">
              <Check size={17} />
              {auth.status.mode === 'legacy_pat' ? '以前の認証で接続済み' : 'GitHubに接続済み'}
            </p>
            <div className="row auth-actions">
              {auth.status.install_url && (
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() => {
                    void native('auth_open_browser', { target: 'install' }).catch(tell);
                  }}
                >
                  <ExternalLink size={16} />
                  リポジトリの許可
                </Button>
              )}
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() => {
                  void disconnect();
                }}
              >
                この端末の接続を解除
              </Button>
            </div>
            <label>
              同期するリポジトリ
              <select
                aria-label="同期するリポジトリ"
                value={selected?.id ?? ''}
                disabled={loading || busy}
                onChange={(e) => selectRepository(e.target.value)}
              >
                <option value="">{loading ? '取得中…' : 'リポジトリを選択'}</option>
                {repositories.map((r) => (
                  <option key={r.id} value={r.id}>
                    {r.owner}/{r.name}
                  </option>
                ))}
              </select>
            </label>
            <Button
              variant="ghost"
              disabled={loading || busy}
              onClick={() => {
                void loadRepositories();
              }}
            >
              <RefreshCw size={15} className={loading ? 'spin' : ''} />
              一覧を更新
            </Button>
            {!loading && !repositories.length && (
              <p className="hint">
                許可されたリポジトリがありません。「リポジトリの許可」で対象を選び、一覧を更新してください。
              </p>
            )}
            {selected && (
              <label>
                ブランチ
                <select
                  aria-label="同期するブランチ"
                  value={form.branch_name}
                  onChange={(e) => setForm({ ...form, branch_name: e.target.value })}
                  disabled={busy || branchesLoading || !branches.length}
                >
                  {!branches.includes(form.branch_name) && (
                    <option value={form.branch_name}>{form.branch_name}</option>
                  )}
                  {branches.map((name) => (
                    <option key={name} value={name}>
                      {name}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {selected && (
              <>
                {branchesLoading ? (
                  <p role="status" className="hint">
                    ブランチを取得しています…
                  </p>
                ) : branchesError ? (
                  <p role="alert" className="error">
                    ブランチを取得できませんでした。{branchesError}
                  </p>
                ) : !branches.length ? (
                  <p role="status" className="hint">
                    ブランチがありません。空のリポジトリの場合は、GitHubでREADMEなどを作成して最初のコミットを保存し、「ブランチを再取得」を押してください。
                  </p>
                ) : !branches.includes(form.branch_name) ? (
                  <p role="status" className="hint">
                    一覧から接続するブランチを選択してください。
                  </p>
                ) : null}
                <Button
                  variant="ghost"
                  disabled={busy || branchesLoading}
                  onClick={() => {
                    void loadBranches(form);
                  }}
                >
                  ブランチを再取得
                </Button>
              </>
            )}
            {connectionId(form) !== connectionId(settings) && settings.repository_owner && (
              <p className="hint">
                現在のローカルメモも新しい接続先と統合されます。以前の接続先のSHAは再利用しません。
              </p>
            )}
            <Button
              disabled={
                busy || branchesLoading || !selected || !branches.includes(form.branch_name)
              }
              onClick={() => {
                void save(true);
              }}
            >
              {busy ? '確認中…' : 'このリポジトリに接続'}
            </Button>
          </>
        ) : (
          <>
            {auth.status.needs_reconnect && (
              <p className="hint">
                認証の有効期限が切れたか、権限が取り消されています。再接続してください。メモは保持されています。
              </p>
            )}
            {!auth.status.configured && (
              <p className="hint">
                このビルドのGitHub接続は準備中です。配布元でのGitHub
                App設定が必要です。ローカルメモはそのまま利用できます。
              </p>
            )}
            {!session ? (
              <Button
                disabled={starting || busy || !auth.status.configured}
                onClick={() => {
                  void begin();
                }}
              >
                <Github size={18} />
                {starting ? '接続を準備中…' : 'GitHubで接続'}
              </Button>
            ) : (
              <div className="device-flow" role="status">
                <p>GitHubの画面で、このコードを入力してください。</p>
                <strong className="device-code">{session.user_code}</strong>
                <p className="hint">
                  {session.verification_uri}
                  <br />
                  有効期限 {new Date(session.expires_at * 1000).toLocaleTimeString('ja-JP')}
                </p>
                <Button
                  variant="outline"
                  onClick={() => {
                    void native('auth_open_browser', { target: 'device' }).catch(tell);
                  }}
                >
                  <ExternalLink size={16} />
                  GitHubの認証画面を開く
                </Button>
                <p className="hint">認可を待っています。認証後、この画面は自動で切り替わります。</p>
              </div>
            )}
            {(session || starting) && (
              <Button variant="ghost" onClick={cancel}>
                接続をキャンセル
              </Button>
            )}
            <Button
              variant="ghost"
              disabled={busy || starting || Boolean(session)}
              onClick={() => {
                void useAuth.getState().refresh().catch(tell);
              }}
            >
              接続状態を再確認
            </Button>
            {auth.status.legacy_available && (
              <details className="legacy-auth">
                <summary>以前の保管庫から認証を移行</summary>
                <p className="hint">
                  以前の保管庫は削除しません。今回だけパスフレーズを入力してください。
                </p>
                <label>
                  以前のパスフレーズ
                  <input
                    type="password"
                    autoComplete="off"
                    value={legacyPass}
                    onChange={(e) => setLegacyPass(e.target.value)}
                  />
                </label>
                <Button
                  variant="outline"
                  disabled={busy || legacyPass.length < 8 || Boolean(session)}
                  onClick={() => {
                    void migrate();
                  }}
                >
                  以前の認証を移行
                </Button>
              </details>
            )}
          </>
        )}
        {message && (
          <p role="status" className={success ? 'success' : 'error'}>
            {success && <Check size={16} />} {message}
          </p>
        )}
      </div>
      <footer className="settings-footer">
        <Button
          variant="outline"
          disabled={busy || connectionId(form) !== connectionId(settings)}
          onClick={() => {
            void save(false);
          }}
        >
          表示・自動同期の設定を保存
        </Button>
      </footer>
    </Modal>
  );
}
