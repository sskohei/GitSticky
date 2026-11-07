import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { Editor } from './Editor';
import { MemoryDatabase } from '../../test/fakes';
import { newNote } from '../../lib/notes';
import { useApp } from '../../stores/app';
vi.mock('@uiw/react-codemirror', () => ({
  default: ({ value, onChange }: { value: string; onChange: (value: string) => void }) => (
    <textarea aria-label="Markdown本文" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}));
describe('note editing', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useApp.setState({ notes: [] });
  });
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
  it('adds Japanese and existing tags, deduplicates them, and autosaves removal', async () => {
    const db = new MemoryDatabase();
    const note = { ...newNote(), body_md: '本文は保持' };
    const existing = { ...newNote(), tags: ['アイデア'] };
    await db.save(note);
    await db.save(existing);
    useApp.setState({ notes: [note, existing] });
    render(<Editor note={note} db={db} onClose={() => {}} />);
    expect(screen.queryByLabelText('追加するタグ')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'タグを編集' }));
    fireEvent.change(screen.getByLabelText('追加するタグ'), { target: { value: ' 仕事 ' } });
    fireEvent.click(screen.getByRole('button', { name: 'タグを追加' }));
    fireEvent.change(screen.getByLabelText('追加するタグ'), { target: { value: '仕事' } });
    fireEvent.click(screen.getByRole('button', { name: 'タグを追加' }));
    fireEvent.click(screen.getByRole('button', { name: 'タグ「アイデア」を追加' }));
    await act(() => vi.advanceTimersByTimeAsync(500));
    expect(db.data.notes.find((value) => value.id === note.id)?.tags).toEqual(['仕事', 'アイデア']);
    fireEvent.click(screen.getByRole('button', { name: 'タグ「仕事」を外す' }));
    fireEvent.click(screen.getByRole('button', { name: 'タグ「アイデア」を外す' }));
    fireEvent.click(screen.getByRole('button', { name: 'タグ編集を閉じる' }));
    await act(() => vi.advanceTimersByTimeAsync(500));
    expect(db.data.notes.find((value) => value.id === note.id)).toMatchObject({
      tags: [],
      body_md: '本文は保持',
    });
    expect(screen.queryByLabelText('追加するタグ')).not.toBeInTheDocument();
  });
  it('reports invalid tags without changing saved data, and Escape closes only tag editing', async () => {
    const db = new MemoryDatabase();
    const note = { ...newNote(), tags: ['仕事'] };
    await db.save(note);
    const onClose = vi.fn();
    render(<Editor note={note} db={db} onClose={onClose} />);
    fireEvent.click(screen.getByRole('button', { name: 'タグを編集' }));
    fireEvent.change(screen.getByLabelText('追加するタグ'), { target: { value: 'あ'.repeat(31) } });
    fireEvent.click(screen.getByRole('button', { name: 'タグを追加' }));
    expect(screen.getByRole('alert')).toHaveTextContent('30文字');
    await act(() => vi.advanceTimersByTimeAsync(500));
    expect(db.data.notes[0].tags).toEqual(['仕事']);
    // Radix listens at the document, before React's bubbling key handlers in the WebView.
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByLabelText('追加するタグ')).not.toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: 'メモを編集' })).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'タグを編集' })).toHaveFocus();
  });
});
