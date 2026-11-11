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
  private: boolean;
}
interface Backup {
  id: number;
  title: string;
  saved_at: string;
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
  const [syncKey, setSyncKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [keyReady, setKeyReady] = useState(false);
  const [keyLoading, setKeyLoading] = useState(false);
  const [rotationPending, setRotationPending] = useState(false);
  const [rotating, setRotating] = useState(false);
  const [keyBackedUp, setKeyBackedUp] = useState(false);
  const [backups, setBackups] = useState<Backup[] | null>(null);
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
  useEffect(() => {
    let active = true;
    setSyncKey('');
    setShowKey(false);
    setKeyReady(false);
    setRotationPending(false);
    setRotating(false);
    setKeyBackedUp(false);
    setKeyLoading(false);
    if (!form.repository_owner || !form.repository_name) return;
    setKeyLoading(true);
    void native<{ sync_ready: boolean; rotation_pending?: boolean }>('security_status', {
      settings: form,
    })
      .then((status) => {
        if (active) {
          setKeyReady(status.sync_ready);
          setRotationPending(Boolean(status.rotation_pending));
        }
      })
      .catch((error) => {
        if (active) tell(error);
      })
      .finally(() => {
        if (active) setKeyLoading(false);
      });
    return () => {
      active = false;
    };
    // Key availability belongs to the selected connection, independent of display preferences.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [form.repository_owner, form.repository_name, form.branch_name]);
  const saveKey = async () => {
    setBusy(true);
    try {
      await native('security_set_sync_key', { settings: form, key: syncKey });
      setSyncKey('');
      setShowKey(false);
      setKeyReady(true);
      setSuccess(true);
      setMessage('暗号鍵を端末の保護領域に登録しました。');
    } catch (e) {
      tell(e);
    } finally {
      setBusy(false);
    }
  };
  const rotateKey = async () => {
    setBusy(true);
    setSuccess(false);
    try {
      await native('security_rotate_sync_key', {
        settings: form,
        key: rotationPending ? null : syncKey,
      });
      setSyncKey('');
      setShowKey(false);
      setRotating(false);
      setRotationPending(false);
      setKeyBackedUp(false);
      setKeyReady(true);
      setSuccess(true);
      setMessage('鍵を交換しました。ほかの端末では新しい鍵を再登録してから同期してください。');
    } catch (e) {
      tell(e);
      try {
        const status = await native<{ sync_ready: boolean; rotation_pending: boolean }>(
          'security_status',
          { settings: form },
        );
        setRotationPending(status.rotation_pending);
        setKeyReady(status.sync_ready);
      } catch {
        /* Keep the entered recovery key if secure storage is unavailable. */
      }
    } finally {
      setBusy(false);
    }
  };
  const loadBackups = async () => {
    setBusy(true);
    try {
      setBackups(await native<Backup[]>('db_backups'));
    } catch (e) {
      tell(e);
    } finally {
      setBusy(false);
    }
  };
  const restoreBackup = async (id: number) => {
    setBusy(true);
    try {
      await native('db_restore_backup', { id });
      await useApp.getState().refresh(db);
      setSuccess(true);
      setMessage('復旧用コピーを新しいメモとして復元しました。');
    } catch (e) {
      tell(e);
    } finally {
      setBusy(false);
    }
  };
  const loadRepositories = useCallback(async () => {
    const request = ++repositoryRequest.current;
    setLoading(true);
    try {
      const result = await native<Repository[]>('github_repositories');
      if (mounted.current && request === repositoryRequest.current)
        setRepositories(result.filter((repo) => repo.private === true));
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
      if (test && !keyReady) throw new AppError('locked', '先に同期用の暗号鍵を登録してください。');
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
          非公開リポジトリだけに同期します。メモのタイトル・本文・タグは端末内と送信前に暗号化されます。ログイン情報は端末で保護されます。
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
                    {r.owner}/{r.name}（非公開）
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
            {selected && (
              <section aria-label="同期の暗号化">
                <h4>同期用の暗号鍵</h4>
                {keyReady ? (
                  <>
                    <p className="hint">この同期先の暗号鍵は登録済みです。</p>
                    <Button
                      variant="ghost"
                      disabled={busy || rotating}
                      onClick={() => setKeyReady(false)}
                    >
                      同期用の暗号鍵を再登録
                    </Button>
                    <Button
                      variant="ghost"
                      disabled={busy}
                      onClick={() => {
                        setRotating(!rotating);
                        setSyncKey('');
                        setKeyBackedUp(false);
                      }}
                    >
                      {rotating ? '鍵の交換をやめる' : '同期用の暗号鍵を交換'}
                    </Button>
                  </>
                ) : (
                  !rotationPending && (
                    <>
                      <p className="hint">
                        別の端末と同期する場合は、その端末で作成した同じ鍵を入力してください。初めて使う同期先では新しい鍵を作成し、パスワード管理アプリなどへ控えてください。鍵を失うとGitHub上のメモを復元できません。
                      </p>
                      <label>
                        同期用の暗号鍵
                        <input
                          type={showKey ? 'text' : 'password'}
                          autoComplete="off"
                          spellCheck={false}
                          value={syncKey}
                          onChange={(event) => setSyncKey(event.target.value)}
                        />
                      </label>
                      <label className="checkbox">
                        <input
                          type="checkbox"
                          checked={showKey}
                          onChange={(event) => setShowKey(event.target.checked)}
                        />
                        鍵を表示する
                      </label>
                      <div className="row">
                        <Button
                          variant="outline"
                          disabled={busy}
                          onClick={() => {
                            setBusy(true);
                            void native<string>('security_generate_key')
                              .then(setSyncKey)
                              .catch(tell)
                              .finally(() => setBusy(false));
                          }}
                        >
                          新しい鍵を作成
                        </Button>
                        <Button
                          disabled={busy || keyLoading || !syncKey.trim()}
                          onClick={() => {
                            void saveKey();
                          }}
                        >
                          暗号鍵を登録
                        </Button>
                      </div>
                    </>
                  )
                )}
                {(rotating || rotationPending) && (
                  <section aria-label="暗号鍵の交換">
                    <p className="hint">
                      すべての同期メモを新しい鍵でまとめて暗号化し直します。過去のGit履歴は旧鍵で読める状態が残ります。交換後は、ほかの端末にも新しい鍵を登録してください。
                    </p>
                    {rotationPending ? (
                      <p role="status">鍵の交換が中断されています。保存済みの鍵で再開できます。</p>
                    ) : (
                      <>
                        <label>
                          交換する新しい暗号鍵
                          <input
                            type={showKey ? 'text' : 'password'}
                            autoComplete="off"
                            spellCheck={false}
                            value={syncKey}
                            onChange={(event) => {
                              setSyncKey(event.target.value);
                              setKeyBackedUp(false);
                            }}
                          />
                        </label>
                        <label className="checkbox">
                          <input
                            type="checkbox"
                            checked={showKey}
                            onChange={(event) => setShowKey(event.target.checked)}
                          />
                          新しい鍵を表示する
                        </label>
                        <Button
                          variant="outline"
                          disabled={busy}
                          onClick={() => {
                            setBusy(true);
                            void native<string>('security_generate_key')
                              .then((key) => {
                                setSyncKey(key);
                                setKeyBackedUp(false);
                              })
                              .catch(tell)
                              .finally(() => setBusy(false));
                          }}
                        >
                          交換用の鍵を作成
                        </Button>
                        <label className="checkbox">
                          <input
                            type="checkbox"
                            checked={keyBackedUp}
                            onChange={(event) => setKeyBackedUp(event.target.checked)}
                          />
                          新しい鍵をアプリの外に安全に控えました
                        </label>
                      </>
                    )}
                    <Button
                      disabled={busy || (!rotationPending && (!syncKey.trim() || !keyBackedUp))}
                      onClick={() => {
                        void rotateKey();
                      }}
                    >
                      {busy
                        ? '鍵を交換中…'
                        : rotationPending
                          ? '鍵の交換を再開'
                          : 'すべてのメモの鍵を交換'}
                    </Button>
                  </section>
                )}
                <p className="hint">
                  以前の平文メモは同期時に暗号化します。過去のGit履歴には平文が残るため、機密メモは新しい非公開リポジトリへ移すことを推奨します。
                </p>
              </section>
            )}
            <Button
              disabled={
                busy ||
                keyLoading ||
                !keyReady ||
                branchesLoading ||
                !selected ||
                !branches.includes(form.branch_name)
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
        <section aria-label="メモの復旧">
          <h3>メモの復旧</h3>
          <p className="hint">
            リモートの変更・削除を反映する前のメモを、端末内に暗号化して直近100件まで保持します。
          </p>
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => {
              void loadBackups();
            }}
          >
            復旧用コピーを表示
          </Button>
          {backups?.length === 0 && <p>復旧用コピーはありません。</p>}
          {backups?.map((backup) => (
            <div key={backup.id} className="row">
              <span>
                {backup.title}
                <small> {new Date(backup.saved_at).toLocaleString('ja-JP')}</small>
              </span>
              <Button
                variant="ghost"
                disabled={busy}
                aria-label={`${backup.title}を復元`}
                onClick={() => {
                  void restoreBackup(backup.id);
                }}
              >
                新しいメモとして復元
              </Button>
            </div>
          ))}
        </section>
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
