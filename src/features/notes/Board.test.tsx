import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { Board } from './Board';
import { newNote } from '../../lib/notes';

describe('tag filters', () => {
  const both = { ...newNote(), title: '両方のメモ', tags: ['仕事', 'アイデア', '企画'] };
  const one = { ...newNote(), title: '仕事のみ', tags: ['仕事'] };
  const neither = { ...newNote(), title: '未分類' };
  const props = { notes: [both, one, neither], query: '', onOpen: () => {}, onCreate: () => {} };
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
