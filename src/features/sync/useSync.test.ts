import { act, renderHook } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { MemoryDatabase } from '../../test/fakes';
import { newNote } from '../../lib/notes';
import { useApp } from '../../stores/app';
import { defaultSettings } from '../../types';
import { useSync } from './useSync';
import { github } from './github';
vi.mock('./github', () => ({
  github: {
    list: vi.fn(async () => []),
    read: vi.fn(),
    write: vi.fn(async () => 'sha'),
    remove: vi.fn(),
  },
}));
afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
  useApp.setState({ notes: [], settings: defaultSettings });
});
it('only syncs on manual request when automatic sync is off', async () => {
  vi.useFakeTimers();
  const db = new MemoryDatabase();
  await db.save(newNote());
  await useApp.getState().refresh(db);
  const flush = vi.fn(async () => {});
  const { result } = renderHook(() => useSync(db, true, flush));

  await act(async () => {
    window.dispatchEvent(new Event('online'));
    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(15000);
  });
  expect(flush).not.toHaveBeenCalled();
  expect(github.list).not.toHaveBeenCalled();

  await act(() => result.current.sync());
  expect(github.list).toHaveBeenCalledTimes(1);
});

it('suspends synchronization while connection settings and credentials are being edited', async () => {
  const db = new MemoryDatabase();
  await useApp.getState().refresh(db);
  const flush = vi.fn(async () => {});
  const { rerender } = renderHook(({ paused }) => useSync(db, true, flush, paused), {
    initialProps: { paused: true },
  });
  expect(github.list).not.toHaveBeenCalled();
  await act(async () => {
    await db.settings({
      ...db.data.settings,
      auto_sync_enabled: true,
      repository_name: 'new-target',
    });
    await useApp.getState().refresh(db);
  });
  expect(github.list).not.toHaveBeenCalled();
  await act(async () => {
    rerender({ paused: false });
  });
  expect(github.list).toHaveBeenCalledTimes(1);
  expect(github.list).toHaveBeenCalledWith(
    expect.objectContaining({ repository_name: 'new-target' }),
  );
  expect(flush).toHaveBeenCalledTimes(1);
});
