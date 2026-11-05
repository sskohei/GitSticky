# Dot-source this file: . ./scripts/env.ps1
$stickyRoot = Split-Path -Parent $PSScriptRoot
if (Test-Path "$stickyRoot/.tools/cargo/bin/cargo.exe") {
  $env:CARGO_HOME = "$stickyRoot/.tools/cargo"
  $env:RUSTUP_HOME = "$stickyRoot/.tools/rustup"
  $env:PATH = "$env:CARGO_HOME/bin;$env:PATH"
}
if (!$env:ANDROID_HOME -and (Test-Path "$env:LOCALAPPDATA/Android/Sdk")) { $env:ANDROID_HOME = "$env:LOCALAPPDATA/Android/Sdk" }
if (!$env:NDK_HOME -and $env:ANDROID_HOME) {
  $stickyNdk = Get-ChildItem "$env:ANDROID_HOME/ndk" -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1
  if ($stickyNdk) { $env:NDK_HOME = $stickyNdk.FullName }
}
if (!$env:JAVA_HOME) {
  $stickyJava = Get-Command java.exe -ErrorAction SilentlyContinue
  if ($stickyJava) { $env:JAVA_HOME = Split-Path -Parent (Split-Path -Parent $stickyJava.Source) }
}
