import { type Settings } from '../../types';
import { native } from '../../lib/native';
export { native } from '../../lib/native';
export interface RemoteFile {
  id: string;
  sha: string;
  needsEncryption?: boolean;
}
export interface GitHub {
  finishMigration?(settings: Settings): Promise<void>;
  list(settings: Settings): Promise<RemoteFile[]>;
  read(settings: Settings, file: RemoteFile): Promise<string>;
  write(settings: Settings, id: string, content: string, sha: string | null): Promise<string>;
  remove(settings: Settings, id: string, sha: string): Promise<void>;
}
export const github: GitHub = {
  finishMigration: (settings) => native('security_finish_migration', { settings }),
  list: (settings) => native('github_list', { settings }),
  read: async (settings, file) => {
    const result = await native<{ content: string; encrypted: boolean }>('github_read', {
      settings,
      id: file.id,
      sha: file.sha,
    });
    file.needsEncryption = !result.encrypted;
    return result.content;
  },
  write: (settings, id, content, sha) => native('github_write', { settings, id, content, sha }),
  remove: (settings, id, sha) => native('github_remove', { settings, id, sha }),
};
