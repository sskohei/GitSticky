param([string]$KeyDirectory = (Join-Path ([Environment]::GetFolderPath('UserProfile')) '.tauri/gitsticky'))
$ErrorActionPreference = 'Stop'
$stickyRoot = Split-Path -Parent $PSScriptRoot
. "$PSScriptRoot/updater-credentials.ps1"
$stickyCredentials = Get-StickyUpdaterCredentials -KeyDirectory $KeyDirectory
$stickySaved = @{}
foreach ($stickyName in @('STICKYNOTE_UPDATER_PUBLIC_KEY', 'TAURI_SIGNING_PRIVATE_KEY', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD')) {
  $stickySaved[$stickyName] = [Environment]::GetEnvironmentVariable($stickyName, 'Process')
}
Push-Location $stickyRoot
try {
  $env:STICKYNOTE_UPDATER_PUBLIC_KEY = $stickyCredentials.PublicKey
  $env:TAURI_SIGNING_PRIVATE_KEY = $stickyCredentials.PrivateKey
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $stickyCredentials.Password
  & node scripts/release-check.mjs
  if ($LASTEXITCODE -ne 0) { throw 'Release configuration check failed.' }
  & "$PSScriptRoot/updater-config.ps1"
  # --no-sign would also suppress the updater signature, so do not use it here.
  & node node_modules/@tauri-apps/cli/tauri.js build --ci --bundles nsis --config src-tauri/tauri.updater.local.json -- --locked
  if ($LASTEXITCODE -ne 0) { throw 'Windows build failed.' }
  & "$PSScriptRoot/verify-windows-release.ps1"
} finally {
  Pop-Location
  foreach ($stickyName in $stickySaved.Keys) {
    [Environment]::SetEnvironmentVariable($stickyName, $stickySaved[$stickyName], 'Process')
  }
  $stickyCredentials = $null
}
