import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from './App';
import { newNote } from './lib/notes';
import { useApp } from './stores/app';
import { disconnected, useAuth } from './stores/auth';
import { MemoryDatabase } from './test/fakes';
import { defaultSettings } from './types';

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ onCloseRequested: async () => () => {} }),
}));
vi.mock('./lib/native', () => ({ native: async () => disconnected }));
vi.mock('@uiw/react-codemirror', () => ({
  default: ({ value, onChange }: { value: string; onChange: (value: string) => void }) => (
    <textarea aria-label="Markdown本文" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}));

describe('creating notes from a filtered board', () => {
  beforeEach(() => {
    useApp.setState({ notes: [], query: '', settings: defaultSettings });
    useAuth.setState({ status: disconnected });
  });

  it.each([{ tags: ['仕事'] }, { tags: ['仕事', 'アイデア'] }])(
    'persists selected tags, allows editing them, and retains filters: $tags',
    async ({ tags }) => {
      const db = new MemoryDatabase();
      const original = newNote(['仕事', 'アイデア']);
      await db.save(original);
      render(<App db={db} />);
      for (const tag of tags) {
        fireEvent.click(await screen.findByRole('button', { name: `タグ「${tag}」で絞り込み` }));
      }
      await act(async () => {
        fireEvent.click(
          screen.getByRole('button', {
            name: tags.length === 1 ? 'このタグで新しいメモ' : '選択中のタグで新しいメモ',
          }),
        );
      });
      await act(() => vi.dynamicImportSettled());
      const dialog = await screen.findByRole('dialog', { name: 'メモを編集' });
      const created = db.data.notes.find((note) => note.id !== original.id)!;
      expect(created.tags).toEqual(tags);
      expect(created.body_md).toBe('');
      fireEvent.click(within(dialog).getByRole('button', { name: 'タグを編集' }));
      for (const tag of tags) {
        expect(within(dialog).getByRole('button', { name: `タグ「${tag}」を外す` })).toBeVisible();
      }
      fireEvent.click(within(dialog).getByRole('button', { name: `タグ「${tags[0]}」を外す` }));
      fireEvent.change(within(dialog).getByLabelText('追加するタグ'), {
        target: { value: '追加タグ' },
      });
      fireEvent.click(within(dialog).getByRole('button', { name: 'タグを追加' }));
      fireEvent.change(within(dialog).getByLabelText('Markdown本文'), {
        target: { value: 'タグ付きで書く' },
      });
      await act(async () => {
        fireEvent.click(within(dialog).getByRole('button', { name: '閉じる' }));
      });
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
      expect(db.data.notes.find((note) => note.id === created.id)).toMatchObject({
        tags: [...tags.slice(1), '追加タグ'],
        body_md: 'タグ付きで書く',
      });
      for (const tag of tags) {
        expect(screen.getByRole('button', { name: `タグ「${tag}」で絞り込み` })).toHaveAttribute(
          'aria-pressed',
          'true',
        );
      }
    },
    15000,
  );

  it('keeps the header creation untagged while a tag filter and keyword search are active', async () => {
    const db = new MemoryDatabase();
    const original = newNote(['仕事']);
    await db.save(original);
    render(<App db={db} />);
    fireEvent.click(await screen.findByRole('button', { name: 'タグ「仕事」で絞り込み' }));
    fireEvent.change(screen.getByLabelText('メモを検索'), { target: { value: '検索語' } });
    fireEvent.click(screen.getByRole('button', { name: '新しいメモ' }));
    const dialog = await screen.findByRole('dialog', { name: 'メモを編集' });
    expect(db.data.notes.find((note) => note.id !== original.id)).toMatchObject({
      title: '無題のメモ',
      body_md: '',
      tags: [],
    });
    await act(async () => {
      fireEvent.click(within(dialog).getByRole('button', { name: '閉じる' }));
    });
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(screen.getByLabelText('メモを検索')).toHaveValue('検索語');
    expect(screen.getByText('メモが見つかりません')).toBeInTheDocument();
  });
});
