import cli from '@tauri-apps/cli';

// Pass the password in memory, never in an OS command line or a checked-in file.
if (!process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD || !process.argv[2]) {
  throw new Error('Key path and password environment variable are required.');
}
await cli.run([
  'signer',
  'generate',
  '--ci',
  '--password',
  process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD,
  '--write-keys',
  process.argv[2],
]);
