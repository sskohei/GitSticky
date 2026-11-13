$ErrorActionPreference = 'Stop'
$stickyRoot = Split-Path -Parent $PSScriptRoot
$stickyVersion = (Get-Content "$stickyRoot/src-tauri/tauri.conf.json" -Raw | ConvertFrom-Json).version
$stickyInstaller = Join-Path $stickyRoot "src-tauri/target/release/bundle/nsis/StickyNote_${stickyVersion}_x64-setup.exe"
if (!(Test-Path -LiteralPath "$stickyInstaller.sig")) { throw 'Missing updater signature.' }
if ((Get-AuthenticodeSignature -LiteralPath $stickyInstaller).Status -ne 'NotSigned') {
  throw 'The installer must be Authenticode-unsigned under the personal distribution policy.'
}
Push-Location $stickyRoot
try {
  & cargo run --quiet --manifest-path src-tauri/Cargo.toml --example verify_update --locked -- $stickyInstaller "$stickyInstaller.sig" $stickyVersion
  if ($LASTEXITCODE -ne 0) { throw 'Updater signature verification failed.' }
} finally { Pop-Location }
Write-Output 'Verified: Authenticode unsigned; updater signature and version valid.'
