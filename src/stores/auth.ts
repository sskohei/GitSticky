import { create } from 'zustand';
import { native } from '../lib/native';
import { errorMessage } from '../types';
export interface AuthStatus {
  connected: boolean;
  mode: 'oauth' | 'legacy_pat' | null;
  configured: boolean;
  needs_reconnect: boolean;
  expires_at: number | null;
  install_url: string | null;
  legacy_available: boolean;
}
export const disconnected: AuthStatus = {
  connected: false,
  mode: null,
  configured: false,
  needs_reconnect: false,
  expires_at: null,
  install_url: null,
  legacy_available: false,
};
interface AuthStore {
  status: AuthStatus;
  error: string;
  refresh: () => Promise<AuthStatus>;
}
export const useAuth = create<AuthStore>((set) => ({
  status: disconnected,
  error: '',
  refresh: async () => {
    try {
      const status = await native<AuthStatus>('auth_status');
      set({ status, error: '' });
      return status;
    } catch (e) {
      set((state) => ({ status: { ...state.status, connected: false }, error: errorMessage(e) }));
      throw e;
    }
  },
}));
