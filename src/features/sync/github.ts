import { type Settings } from '../../types';
import { native } from '../../lib/native';
export { native } from '../../lib/native';
export interface RemoteFile {
  id: string;
  sha: string;
}
export interface GitHub {
  list(settings: Settings): Promise<RemoteFile[]>;
  read(settings: Settings, file: RemoteFile): Promise<string>;
  write(settings: Settings, id: string, content: string, sha: string | null): Promise<string>;
  remove(settings: Settings, id: string, sha: string): Promise<void>;
}
export const github: GitHub = {
  list: (settings) => native('github_list', { settings }),
  read: (settings, file) => native('github_read', { settings, sha: file.sha }),
  write: (settings, id, content, sha) => native('github_write', { settings, id, content, sha }),
  remove: (settings, id, sha) => native('github_remove', { settings, id, sha }),
};
