import { create } from 'zustand';
import type { LocalDatabase } from '../db';
import { defaultSettings, type Note, type Settings } from '../types';
interface AppState {
  notes: Note[];
  settings: Settings;
  query: string;
  setQuery: (query: string) => void;
  refresh: (db: LocalDatabase) => Promise<void>;
}
export const useApp = create<AppState>((set) => ({
  notes: [],
  settings: defaultSettings,
  query: '',
  setQuery: (query) => set({ query }),
  refresh: async (db) => {
    const s = await db.snapshot();
    set({ notes: s.notes, settings: s.settings });
  },
}));
