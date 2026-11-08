import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { Board } from './Board';
import { newNote } from '../../lib/notes';
import { MAX_NOTE_TAGS } from '../../lib/tags';

describe('tag filters', () => {
  const both = { ...newNote(), title: '両方のメモ', tags: ['仕事', 'アイデア', '企画'] };
  const one = { ...newNote(), title: '仕事のみ', tags: ['仕事'] };
  const neither = { ...newNote(), title: '未分類' };
  const props = { notes: [both, one, neither], query: '', onOpen: () => {}, onCreate: () => {} };
  it('creates with all selected tags and no tags after switching to untagged or all', () => {
    const onCreate = vi.fn();
    render(<Board {...props} onCreate={onCreate} />);
    fireEvent.click(screen.getByRole('button', { name: '新しいメモ' }));
    expect(onCreate).toHaveBeenLastCalledWith([]);
    fireEvent.click(screen.getByRole('button', { name: 'タグ「仕事」で絞り込み' }));
    fireEvent.click(screen.getByRole('button', { name: 'このタグで新しいメモ' }));
    expect(onCreate).toHaveBeenLastCalledWith(['仕事']);
    fireEvent.click(screen.getByRole('button', { name: 'タグ「アイデア」で絞り込み' }));
    fireEvent.click(screen.getByRole('button', { name: '選択中のタグで新しいメモ' }));
    expect(onCreate).toHaveBeenLastCalledWith(['仕事', 'アイデア']);
    fireEvent.click(screen.getByRole('button', { name: 'タグなし' }));
    fireEvent.click(screen.getByRole('button', { name: '新しいメモ' }));
    expect(onCreate).toHaveBeenLastCalledWith([]);
    fireEvent.click(screen.getByRole('button', { name: 'すべて' }));
    fireEvent.click(screen.getByRole('button', { name: '新しいメモ' }));
    expect(onCreate).toHaveBeenLastCalledWith([]);
  });
  it('allows creation during keyword search with zero results and retains tag filters', () => {
    const onCreate = vi.fn();
    const { rerender } = render(<Board {...props} query="一致しない検索語" onCreate={onCreate} />);
    fireEvent.click(screen.getByRole('button', { name: '新しいメモ' }));
    expect(onCreate).toHaveBeenLastCalledWith([]);
    fireEvent.click(screen.getByRole('button', { name: 'タグ「仕事」で絞り込み' }));
    expect(screen.getByText('メモが見つかりません')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'このタグで新しいメモ' }));
    expect(onCreate).toHaveBeenLastCalledWith(['仕事']);
    rerender(<Board {...props} notes={[neither]} onCreate={onCreate} />);
    expect(screen.getByRole('button', { name: 'タグ「仕事」で絞り込み' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(screen.getByRole('button', { name: 'このタグで新しいメモ' })).toBeEnabled();
  });
  it('disables creation while saving or when the selected tags exceed the per-note limit', () => {
    const onCreate = vi.fn();
    const { rerender } = render(<Board {...props} onCreate={onCreate} creating />);
    fireEvent.click(screen.getByRole('button', { name: '新しいメモ' }));
    expect(onCreate).not.toHaveBeenCalled();
    const tags = Array.from({ length: MAX_NOTE_TAGS + 1 }, (_, i) => `タグ${i}`);
    rerender(<Board {...props} notes={tags.map((tag) => newNote([tag]))} onCreate={onCreate} />);
    for (const tag of tags) {
      fireEvent.click(screen.getByRole('button', { name: `タグ「${tag}」で絞り込み` }));
    }
    const create = screen.getByRole('button', { name: '選択中のタグで新しいメモ' });
    expect(create).toBeDisabled();
    expect(create).toHaveAccessibleDescription(
      `メモを作成するには、選択するタグを${MAX_NOTE_TAGS}個以下にしてください。`,
    );
    fireEvent.click(create);
    expect(onCreate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: `タグ「${tags[0]}」で絞り込み` }));
    expect(create).toBeEnabled();
    fireEvent.click(create);
    expect(onCreate).toHaveBeenLastCalledWith(tags.slice(1));
  });
  it('filters by all selected tags, switches to untagged, and clears filters', () => {
    render(<Board {...props} />);
    const bothCard = screen.getByRole('button', { name: /両方のメモ/ });
    expect(within(bothCard).getByText('+1')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'タグ「仕事」で絞り込み' }));
    expect(screen.queryByRole('button', { name: /未分類/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /仕事のみ/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'タグ「アイデア」で絞り込み' }));
    expect(screen.getByRole('button', { name: /両方のメモ/ })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /仕事のみ/ })).not.toBeInTheDocument();
    expect(
      screen.getByText('選択したタグがすべて付いたメモを表示しています。'),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'タグなし' }));
    expect(screen.getByRole('button', { name: /未分類/ })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /両方のメモ/ })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'すべて' }));
    expect(screen.getByRole('button', { name: /仕事のみ/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /両方のメモ/ })).toBeInTheDocument();
  });
  it('keeps a now-unused filter removable and combines keyword search', () => {
    const { rerender } = render(<Board {...props} query="両方" />);
    fireEvent.click(screen.getByRole('button', { name: 'タグ「仕事」で絞り込み' }));
    expect(screen.getByRole('button', { name: /両方のメモ/ })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /仕事のみ/ })).not.toBeInTheDocument();
    rerender(<Board {...props} notes={[neither]} />);
    expect(screen.getByText('メモが見つかりません')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'タグ「仕事」で絞り込み' }));
    expect(screen.getByRole('button', { name: /未分類/ })).toBeInTheDocument();
  });
});
