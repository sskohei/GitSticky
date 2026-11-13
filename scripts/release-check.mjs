import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

// Resolve from this script so CI and local invocation use the same files.
const root = fileURLToPath(new URL('../', import.meta.url));
const read = (path) => readFileSync(resolve(root, path), 'utf8');
const pkg = JSON.parse(read('package.json'));
const config = JSON.parse(read('src-tauri/tauri.conf.json'));
const cargo = read('src-tauri/Cargo.toml');
const cargoVersion = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const lockedPackage = read('src-tauri/Cargo.lock')
  .split('[[package]]')
  .find((entry) => /^name = "sticky-note"$/m.test(entry));
const lockedVersion = lockedPackage?.match(/^version = "([^"]+)"/m)?.[1];
const version = config.version;
const errors = [];

// The production feed uses releases/latest, so this workflow publishes stable versions only.
if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
  errors.push('tauri.conf.json must use a stable x.y.z version.');
}
for (const [path, actual] of [
  ['package.json', pkg.version],
  ['Cargo.toml', cargoVersion],
  ['Cargo.lock', lockedVersion],
]) {
  if (actual !== version) errors.push(`${path} version ${actual} does not match ${version}.`);
}
if (process.env.GITHUB_REF_TYPE === 'tag' && process.env.GITHUB_REF_NAME !== `v${version}`) {
  errors.push(`Release tag must be v${version}.`);
}
if (!config.bundle.active || !config.bundle.targets.includes('nsis')) {
  errors.push('The Windows NSIS bundle must be enabled.');
}
if (!config.bundle.license || !config.bundle.licenseFile || !config.bundle.homepage) {
  errors.push('Bundle license, licenseFile and homepage must be configured.');
} else {
  read(`src-tauri/${config.bundle.licenseFile}`);
}
const endpoints = config.plugins?.updater?.endpoints;
if (config.bundle.windows?.certificateThumbprint || config.bundle.windows?.signCommand) {
  errors.push('Windows Authenticode signing must remain disabled for personal distribution.');
}
if (config.plugins?.updater?.requireSignedVersion !== true) {
  errors.push('Updater signatures must bind the announced version.');
}
if (!endpoints?.length || endpoints.some((endpoint) => !endpoint.startsWith('https://'))) {
  errors.push('Updater endpoints must use HTTPS.');
}
if (errors.length) {
  console.error(errors.join('\n'));
  process.exitCode = 1;
} else {
  console.log(`Release configuration OK: StickyNote v${version} (Windows NSIS).`);
}
