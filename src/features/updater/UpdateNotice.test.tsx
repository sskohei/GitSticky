import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { check, type Update, type DownloadEvent } from '@tauri-apps/plugin-updater';
import { native } from '../../lib/native';
import { UpdateNotice, UPDATE_INTERVAL } from './UpdateNotice';

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }));
vi.mock('../../lib/native', () => ({ native: vi.fn() }));

describe('app updates', () => {
  const flush = vi.fn<() => Promise<void>>();
  const onInstalling = vi.fn();
  const install = vi.fn();
  const close = vi.fn();
  const available = { version: '0.2.0', downloadAndInstall: install, close } as unknown as Update;
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(native).mockResolvedValue(true);
    vi.mocked(check).mockResolvedValue(available);
    flush.mockResolvedValue();
    close.mockResolvedValue(undefined);
    install.mockImplementation(() => new Promise(() => {}));
  });
  afterEach(() => vi.useRealTimers());
  const mount = (disabled = false) =>
    render(<UpdateNotice disabled={disabled} flush={flush} onInstalling={onInstalling} />);

  it('only installs after a click and a successful save, and prevents double clicks', async () => {
    let saved!: () => void;
    flush.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          saved = resolve;
        }),
    );
    mount();
    const button = await screen.findByRole('button', { name: '更新する' });
    expect(install).not.toHaveBeenCalled();
    fireEvent.click(button);
    fireEvent.click(button);
    expect(flush).toHaveBeenCalledTimes(1);
    expect(install).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog')).toBeVisible();
    await act(async () => saved());
    expect(install).toHaveBeenCalledTimes(1);
    const progress = install.mock.calls[0][0] as (event: DownloadEvent) => void;
    act(() => {
      progress({ event: 'Started', data: { contentLength: 100 } });
      progress({ event: 'Progress', data: { chunkLength: 50 } });
    });
    expect(screen.getByText('更新をダウンロードしています… 50%')).toBeVisible();
  });

  it('does not install if saving fails and allows retrying', async () => {
    flush.mockRejectedValueOnce(new Error('disk full'));
    mount();
    fireEvent.click(await screen.findByRole('button', { name: '更新する' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('更新できませんでした');
    expect(install).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '更新する' }));
    await waitFor(() => expect(install).toHaveBeenCalledTimes(1));
  });

  it('recovers after download or signature failure', async () => {
    install.mockRejectedValueOnce(new Error('invalid signature'));
    mount();
    fireEvent.click(await screen.findByRole('button', { name: '更新する' }));
    expect(await screen.findByRole('alert')).toBeVisible();
    expect(onInstalling).toHaveBeenLastCalledWith(false);
    fireEvent.click(screen.getByRole('button', { name: '更新する' }));
    await waitFor(() => expect(install).toHaveBeenCalledTimes(2));
  });

  it('blocks installation while editing or syncing', async () => {
    mount(true);
    expect(await screen.findByRole('button', { name: '更新する' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: '更新する' }));
    expect(flush).not.toHaveBeenCalled();
  });

  it('skips checks on unsupported or unconfigured builds', async () => {
    vi.mocked(native).mockResolvedValue(false);
    mount();
    await act(async () => {});
    expect(check).not.toHaveBeenCalled();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('retries a failed check periodically and releases the resource on unmount', async () => {
    vi.useFakeTimers();
    vi.mocked(check).mockRejectedValueOnce(new Error('offline'));
    const view = mount();
    await act(async () => {});
    expect(check).toHaveBeenCalledTimes(1);
    await act(async () => vi.advanceTimersByTimeAsync(UPDATE_INTERVAL));
    expect(screen.getByRole('button', { name: '更新する' })).toBeVisible();
    view.unmount();
    expect(close).toHaveBeenCalledTimes(1);
    await act(async () => vi.advanceTimersByTimeAsync(UPDATE_INTERVAL));
    expect(check).toHaveBeenCalledTimes(2);
  });

  it('stays hidden when already up to date', async () => {
    vi.mocked(check).mockResolvedValue(null);
    mount();
    await act(async () => {});
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('disposes an update received after unmounting', async () => {
    let finish!: (update: Update) => void;
    vi.mocked(check).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const view = mount();
    await act(async () => {});
    view.unmount();
    await act(async () => finish(available));
    expect(close).toHaveBeenCalledTimes(1);
  });
});
