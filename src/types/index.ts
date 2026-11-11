export const colors = ['yellow', 'pink', 'green', 'blue', 'purple', 'gray'] as const;
export type Color = (typeof colors)[number];
export interface Note {
  id: string;
  title: string;
  body_md: string;
  color: Color;
  pinned: boolean;
  tags: string[];
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
  local_revision: number;
  dirty: boolean;
}
export interface SyncState {
  note_id: string;
  connection_id: string;
  last_synced_sha: string | null;
  last_synced_revision: number;
  last_synced_at: string;
  sync_error: string | null;
}
export interface Settings {
  repository_owner: string;
  repository_name: string;
  branch_name: string;
  auto_sync_enabled: boolean;
  theme: 'light' | 'dark' | 'system';
  last_successful_sync_at: string | null;
}
export const defaultSettings: Settings = {
  repository_owner: '',
  repository_name: '',
  branch_name: 'main',
  auto_sync_enabled: false,
  theme: 'system',
  last_successful_sync_at: null,
};
export const connectionId = (s: Settings) =>
  JSON.stringify([
    s.repository_owner.toLowerCase(),
    s.repository_name.toLowerCase(),
    s.branch_name,
  ]);
export interface Snapshot {
  notes: Note[];
  states: SyncState[];
  settings: Settings;
}
export type ErrorCode =
  | 'network'
  | 'auth'
  | 'permission'
  | 'repository'
  | 'branch'
  | 'rate_limit'
  | 'conflict'
  | 'invalid_markdown'
  | 'remote_deleted'
  | 'database'
  | 'locked'
  | 'secure_storage'
  | 'auth_config'
  | 'auth_cancelled'
  | 'validation';
export class AppError extends Error {
  constructor(
    public code: ErrorCode,
    message: string,
    public retry_at: number | null = null,
  ) {
    super(message);
    this.name = 'AppError';
  }
}
export function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === 'object' && e && 'message' in e) return String(e.message);
  return '処理に失敗しました。保存済みのメモは保持されています。';
}
