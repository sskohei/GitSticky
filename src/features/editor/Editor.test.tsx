import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { Editor } from './Editor';
import { MemoryDatabase } from '../../test/fakes';
import { newNote } from '../../lib/notes';
vi.mock('@uiw/react-codemirror', () => ({
  default: ({ value, onChange }: { value: string; onChange: (value: string) => void }) => (
    <textarea aria-label="Markdown本文" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}));
describe('note editing', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());
  it('autosaves title, Japanese markdown, pin and color after 500ms', async () => {
    const db = new MemoryDatabase();
    const note = newNote();
    await db.save(note);
    render(<Editor note={note} db={db} onClose={() => {}} />);
    fireEvent.change(screen.getByLabelText('メモのタイトル'), {
      target: { value: '日本語タイトル' },
    });
    fireEvent.change(screen.getByLabelText('Markdown本文'), {
      target: { value: '# 日本語\n次の行' },
    });
    fireEvent.click(screen.getByLabelText('pink'));
    fireEvent.click(screen.getByLabelText('ピン留め'));
    await act(() => vi.advanceTimersByTimeAsync(500));
    expect(db.data.notes[0]).toMatchObject({
      title: '日本語タイトル',
      body_md: '# 日本語\n次の行',
      color: 'pink',
      pinned: true,
      dirty: true,
    });
    expect(screen.getByRole('status')).toHaveTextContent('ローカル保存済み');
  });
  it('flushes before close and uses tombstones for deletion', async () => {
    const db = new MemoryDatabase();
    const note = newNote();
    await db.save(note);
    const close = vi.fn();
    render(<Editor note={note} db={db} onClose={close} />);
    fireEvent.change(screen.getByLabelText('メモのタイトル'), { target: { value: '閉じる前' } });
    await act(async () => {
      fireEvent.click(screen.getByLabelText('閉じる'));
    });
    expect(close).toHaveBeenCalled();
    expect(db.data.notes[0].title).toBe('閉じる前');
    fireEvent.click(screen.getByLabelText('メモを削除'));
    await act(async () => {
      fireEvent.click(screen.getByText('削除する'));
    });
    expect(db.data.notes[0].deleted_at).not.toBeNull();
  });
  it('keeps editor open and text intact when persistence fails', async () => {
    const db = new MemoryDatabase();
    const note = newNote();
    const close = vi.fn();
    db.save = async () => {
      throw new Error('ディスクがいっぱいです');
    };
    render(<Editor note={note} db={db} onClose={close} />);
    fireEvent.change(screen.getByLabelText('メモのタイトル'), {
      target: { value: '失わない入力' },
    });
    await act(async () => {
      fireEvent.click(screen.getByLabelText('閉じる'));
    });
    expect(close).not.toHaveBeenCalled();
    expect(screen.getByLabelText('メモのタイトル')).toHaveValue('失わない入力');
    expect(screen.getByRole('alert')).toHaveTextContent('ディスクがいっぱい');
  });
  it('flushes input entered while closing is waiting for an earlier write', async () => {
    const db = new MemoryDatabase();
    const note = newNote();
    await db.save(note);
    const save = db.save.bind(db);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    let first = true;
    db.save = async (value) => {
      if (first) {
        first = false;
        await gate;
      }
      await save(value);
    };
    const close = vi.fn();
    render(<Editor note={note} db={db} onClose={close} />);
    fireEvent.change(screen.getByLabelText('メモのタイトル'), { target: { value: '最初の入力' } });
    await act(async () => {
      fireEvent.click(screen.getByLabelText('閉じる'));
    });
    fireEvent.change(screen.getByLabelText('Markdown本文'), {
      target: { value: '保存待ち中の追加入力' },
    });
    await act(async () => {
      release();
    });
    expect(close).toHaveBeenCalled();
    expect(db.data.notes[0].body_md).toBe('保存待ち中の追加入力');
  });
  it('does not execute HTML or preserve dangerous markdown URLs', () => {
    const note = {
      ...newNote(),
      body_md: '<script>alert(1)</script>\n\n[危険](javascript:alert(1))',
    };
    render(<Editor note={note} db={new MemoryDatabase()} onClose={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: 'プレビューのみ' }));
    expect(document.querySelector('script')).toBeNull();
    expect(document.querySelector('.markdown a')?.getAttribute('href')).not.toMatch(/^javascript:/);
  });
  it('shows a live GFM preview beside the editor before the autosave timer fires', () => {
    render(<Editor note={newNote()} db={new MemoryDatabase()} onClose={() => {}} />);
    const source = screen.getByLabelText('Markdown本文');
    const preview = screen.getByRole('region', { name: 'Markdownプレビュー' });
    expect(screen.getByRole('button', { name: '編集とプレビュー' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    fireEvent.change(source, {
      target: {
        value:
          '# 同時プレビュー\n\n**日本語**\n\n- [x] 確認済み\n\n| A | B |\n| - | - |\n| 1 | 2 |',
      },
    });
    expect(source).toBeVisible();
    expect(within(preview).getByRole('heading', { name: '同時プレビュー' })).toBeVisible();
    expect(within(preview).getByText('日本語').tagName).toBe('STRONG');
    expect(within(preview).getByRole('checkbox')).toBeChecked();
    expect(within(preview).getByRole('table')).toBeVisible();
    fireEvent.change(source, { target: { value: '# 更新後' } });
    expect(within(preview).queryByText('同時プレビュー')).not.toBeInTheDocument();
    expect(within(preview).getByRole('heading', { name: '更新後' })).toBeVisible();
  });
  it('retains the editor instance and unsaved text while switching display modes', async () => {
    const db = new MemoryDatabase();
    const note = newNote();
    await db.save(note);
    render(<Editor note={note} db={db} onClose={() => {}} />);
    const source = screen.getByLabelText('Markdown本文');
    fireEvent.change(source, { target: { value: '# 保持する入力' } });
    fireEvent.click(screen.getByRole('button', { name: 'プレビューのみ' }));
    expect(source).not.toBeVisible();
    expect(screen.getByRole('heading', { name: '保持する入力' })).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: '編集のみ' }));
    expect(screen.getByLabelText('Markdown本文')).toBe(source);
    expect(source).toHaveValue('# 保持する入力');
    expect(screen.queryByRole('region', { name: 'Markdownプレビュー' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '編集とプレビュー' }));
    expect(source).toBeVisible();
    expect(screen.getByRole('heading', { name: '保持する入力' })).toBeVisible();
    await act(() => vi.advanceTimersByTimeAsync(500));
    expect(db.data.notes[0].body_md).toBe('# 保持する入力');
  });
});
