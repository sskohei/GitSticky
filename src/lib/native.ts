import { invoke } from '@tauri-apps/api/core';
import { AppError } from '../types';
export async function native<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    if (typeof e === 'object' && e && 'code' in e && 'message' in e) {
      const value = e as { code: AppError['code']; message: string; retry_at?: number };
      throw new AppError(value.code, value.message, value.retry_at ?? null);
    }
    throw e;
  }
}
