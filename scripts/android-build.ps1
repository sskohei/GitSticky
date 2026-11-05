# Build an ARM64 debug APK on Windows, including hosts without symlink privileges.
$ErrorActionPreference = 'Stop'
$stickyRoot = Split-Path -Parent $PSScriptRoot
Push-Location $stickyRoot
try {
  . "$PSScriptRoot/android-sodium.ps1"
  $stickyLog = Join-Path $stickyRoot '.tools/android-build.log'
  $stickyPreference = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  & node node_modules/@tauri-apps/cli/tauri.js android build --debug --target aarch64 --apk --ci 2>&1 | ForEach-Object { $_.ToString() } | Tee-Object -FilePath $stickyLog
  $stickyCode = $LASTEXITCODE
  $ErrorActionPreference = $stickyPreference
  if ($stickyCode -eq 0) { return }
  $stickyOutput = Get-Content -LiteralPath $stickyLog -Raw
  if ($stickyOutput -notmatch 'Creation symbolic link is not allowed|Failed to create a symbolic link') {
    throw 'Tauri Android build failed. See .tools/android-build.log.'
  }
  # Tauri compiled Rust successfully but could not link it into jniLibs.
  # Copy the same compiled library and package with Gradle; no OS policy changes.
  $stickyNative = Join-Path $stickyRoot 'src-tauri/target/aarch64-linux-android/debug/libsticky_note_lib.so'
  if (!(Test-Path -LiteralPath $stickyNative)) { throw 'Compiled Android library is missing.' }
  $stickyMetadata = & cargo metadata --format-version 1 --filter-platform aarch64-linux-android --manifest-path src-tauri/Cargo.toml | ConvertFrom-Json
  if ($LASTEXITCODE) { throw 'Could not resolve the Tauri Android library.' }
  $stickyTauri = @($stickyMetadata.packages | Where-Object { $_.name -eq 'tauri' })
  if ($stickyTauri.Count -ne 1) { throw 'Expected one Tauri version.' }
  $stickyTauriPath = (Split-Path -Parent $stickyTauri[0].manifest_path).Replace('\','/')
  $stickyAndroid = Join-Path $stickyRoot 'src-tauri/gen/android'
  $stickyJni = Join-Path $stickyAndroid 'app/src/main/jniLibs/arm64-v8a'
  New-Item -ItemType Directory -Force $stickyJni | Out-Null
  Copy-Item -LiteralPath $stickyNative -Destination "$stickyJni/libsticky_note_lib.so" -Force
  $stickyUtf8 = [Text.UTF8Encoding]::new($false)
  # These files are generated and gitignored. SQL/Stronghold have no Java modules.
  [IO.File]::WriteAllText("$stickyAndroid/tauri.settings.gradle", "include ':tauri-android'`nproject(':tauri-android').projectDir = new File('$stickyTauriPath/mobile/android')`n", $stickyUtf8)
  [IO.File]::WriteAllText("$stickyAndroid/app/tauri.build.gradle.kts", 'dependencies { add("implementation", project(":tauri-android")) }', $stickyUtf8)
  New-Item -ItemType Directory -Force "$stickyAndroid/app/src/main/assets" | Out-Null
  Copy-Item -LiteralPath src-tauri/tauri.conf.json -Destination "$stickyAndroid/app/src/main/assets/tauri.conf.json" -Force
  Push-Location $stickyAndroid
  try {
    & ./gradlew.bat :app:assembleArm64Debug -x :app:rustBuildArm64Debug -PabiList=arm64-v8a -PtargetList=aarch64 -ParchList=arm64 --console=plain
    if ($LASTEXITCODE) { throw 'Gradle APK packaging failed.' }
  } finally { Pop-Location }
} finally { Pop-Location }
