$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($env:STICKYNOTE_UPDATER_PUBLIC_KEY)) {
  throw 'Set STICKYNOTE_UPDATER_PUBLIC_KEY to the contents of the updater .pub file.'
}
try {
  $stickyPublicKey = $env:STICKYNOTE_UPDATER_PUBLIC_KEY.Trim()
  $stickyDecodedKey = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($stickyPublicKey))
  $stickyKeyLines = $stickyDecodedKey.Trim() -split '\r?\n'
  $stickyKeyBytes = [Convert]::FromBase64String($stickyKeyLines[1])
  if (!$stickyKeyLines[0].StartsWith('untrusted comment: ') -or $stickyKeyBytes.Length -ne 42 -or
      $stickyKeyBytes[0] -ne 0x45 -or $stickyKeyBytes[1] -notin @(0x64, 0x44)) {
    throw 'Invalid public key format.'
  }
} catch { throw 'STICKYNOTE_UPDATER_PUBLIC_KEY must contain the Tauri updater .pub file contents.' }
$stickyRoot = Split-Path -Parent $PSScriptRoot
$stickyConfig = @{
  bundle = @{ createUpdaterArtifacts = $true }
  plugins = @{ updater = @{ pubkey = $stickyPublicKey } }
}
$stickyJson = $stickyConfig | ConvertTo-Json -Depth 5
[IO.File]::WriteAllText((Join-Path $stickyRoot 'src-tauri/tauri.updater.local.json'), $stickyJson, [Text.UTF8Encoding]::new($false))
