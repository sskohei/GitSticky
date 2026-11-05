import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { SettingsDialog } from './Settings';
import { native } from '../../lib/native';
import { useAuth, disconnected, type AuthStatus } from '../../stores/auth';
import { useApp } from '../../stores/app';
import { defaultSettings, AppError } from '../../types';
import { MemoryDatabase } from '../../test/fakes';
import { newNote } from '../../lib/notes';
vi.mock('../../lib/native', () => ({ native: vi.fn() }));
const call = vi.mocked(native);
const session = {
  session_id: 'session-1',
  user_code: 'ABCD-EFGH',
  verification_uri: 'https://github.com/login/device',
  expires_at: Math.floor(Date.now() / 1000) + 900,
  interval_ms: 5000,
};
let connected = false;
let pollCount = 0;
let authStatus: AuthStatus;
beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  connected = false;
  pollCount = 0;
  authStatus = {
    ...disconnected,
    configured: true,
    install_url: 'https://github.com/apps/test/installations/new',
  };
  useAuth.setState({ status: authStatus, error: '' });
  useApp.setState({ notes: [], settings: defaultSettings, query: '' });
  call.mockImplementation(async <T,>(command: string): Promise<T> => {
    switch (command) {
      case 'auth_status':
        return { ...authStatus, connected } as T;
      case 'auth_begin':
        return session as T;
      case 'auth_poll':
        pollCount++;
        if (pollCount >= 2) connected = true;
        return { connected, retry_after_ms: 10000 } as T;
      case 'auth_open_browser':
      case 'auth_cancel':
      case 'github_test':
        return undefined as T;
      case 'auth_disconnect':
        connected = false;
        return undefined as T;
      case 'github_repositories':
        return [{ id: 12, owner: 'octocat', name: 'notes', default_branch: 'main' }] as T;
      case 'github_branches':
        return ['main', 'memo-branch'] as T;
      default:
        throw new Error('Unexpected command: ' + command);
    }
  });
});
afterEach(() => vi.useRealTimers());
it('uses a browser code, respects poll delays, then lets the user select a repository', async () => {
  const db = new MemoryDatabase();
  render(<SettingsDialog db={db} onClose={() => {}} />);
  expect(screen.queryByLabelText(/Personal Access Token/)).not.toBeInTheDocument();
  expect(screen.queryByLabelText(/パスフレーズ/)).not.toBeInTheDocument();
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'GitHubで接続' }));
  });
  expect(screen.getByText('ABCD-EFGH')).toBeInTheDocument();
  expect(call).toHaveBeenCalledWith('auth_open_browser', { target: 'device' });
  await act(() => vi.advanceTimersByTimeAsync(4999));
  expect(pollCount).toBe(0);
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(pollCount).toBe(1);
  await act(() => vi.advanceTimersByTimeAsync(9999));
  expect(pollCount).toBe(1);
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(pollCount).toBe(2);
  expect(screen.getByText('GitHubに接続済み')).toBeInTheDocument();
  await act(async () => {
    fireEvent.change(screen.getByLabelText('同期するリポジトリ'), { target: { value: '12' } });
  });
  await act(async () => {
    fireEvent.change(screen.getByLabelText('同期するブランチ'), {
      target: { value: 'memo-branch' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'このリポジトリに接続' }));
  });
  expect(db.data.settings).toMatchObject({
    repository_owner: 'octocat',
    repository_name: 'notes',
    branch_name: 'memo-branch',
  });
});
it('cancels polling when the user cancels the connection', async () => {
  render(<SettingsDialog db={new MemoryDatabase()} onClose={() => {}} />);
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'GitHubで接続' }));
  });
  fireEvent.click(screen.getByRole('button', { name: '接続をキャンセル' }));
  await act(() => vi.advanceTimersByTimeAsync(20000));
  expect(pollCount).toBe(0);
  expect(screen.queryByText('ABCD-EFGH')).not.toBeInTheDocument();
  expect(call).toHaveBeenCalledWith('auth_cancel', { sessionId: 'session-1' });
});
it('does not open a browser or start polling when a late start response arrives after closing', async () => {
  let resolve!: (value: typeof session) => void;
  const pending = new Promise<typeof session>((r) => {
    resolve = r;
  });
  const previous = call.getMockImplementation()!;
  call.mockImplementation(<T,>(command: string, args?: Record<string, unknown>) =>
    command === 'auth_begin' ? (pending as Promise<T>) : (previous(command, args) as Promise<T>),
  );
  const { unmount } = render(<SettingsDialog db={new MemoryDatabase()} onClose={() => {}} />);
  fireEvent.click(screen.getByRole('button', { name: 'GitHubで接続' }));
  unmount();
  await act(async () => {
    resolve(session);
  });
  expect(call).not.toHaveBeenCalledWith('auth_open_browser', { target: 'device' });
  expect(call).toHaveBeenCalledWith('auth_cancel', { sessionId: 'session-1' });
});
it('displays the unconfigured state without attempting an invalid OAuth request', () => {
  useAuth.setState({ status: { ...authStatus, configured: false } });
  render(<SettingsDialog db={new MemoryDatabase()} onClose={() => {}} />);
  expect(screen.getByRole('button', { name: 'GitHubで接続' })).toBeDisabled();
  expect(screen.getByText(/配布元でのGitHub App設定が必要/)).toBeInTheDocument();
  expect(call).not.toHaveBeenCalledWith('auth_begin');
});
it('shows expiration errors and allows a fresh login', async () => {
  const previous = call.getMockImplementation()!;
  call.mockImplementation(<T,>(command: string, args?: Record<string, unknown>) =>
    command === 'auth_poll'
      ? Promise.reject(new AppError('auth', 'コードの期限が切れました'))
      : (previous(command, args) as Promise<T>),
  );
  render(<SettingsDialog db={new MemoryDatabase()} onClose={() => {}} />);
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'GitHubで接続' }));
  });
  await act(() => vi.advanceTimersByTimeAsync(5000));
  expect(screen.getByText('コードの期限が切れました')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'GitHubで接続' })).toBeEnabled();
});
it('restores a connected session and disconnects without changing local notes', async () => {
  connected = true;
  useAuth.setState({ status: { ...authStatus, connected: true, mode: 'oauth' } });
  const db = new MemoryDatabase();
  await db.save({ ...newNote(), body_md: '接続を解除しても残す本文' });
  const before = await db.snapshot();
  await act(async () => {
    render(<SettingsDialog db={db} onClose={() => {}} />);
  });
  expect(screen.getByText('GitHubに接続済み')).toBeInTheDocument();
  expect(call).not.toHaveBeenCalledWith('auth_begin');
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'この端末の接続を解除' }));
  });
  expect(db.data.notes).toEqual(before.notes);
  expect(screen.getByRole('button', { name: 'GitHubで接続' })).toBeInTheDocument();
});
