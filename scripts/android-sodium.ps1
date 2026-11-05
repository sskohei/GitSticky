param([ValidateSet('arm64-v8a','armeabi-v7a','x86','x86_64')][string]$Abi='arm64-v8a')
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/env.ps1"
if (!$env:NDK_HOME -or !$env:ANDROID_HOME) { throw 'Set ANDROID_HOME and NDK_HOME first.' }
$stickyRoot = Split-Path -Parent $PSScriptRoot
$stickyWork = Join-Path $stickyRoot '.tools/android-sodium'
New-Item -ItemType Directory -Force $stickyWork | Out-Null
$stickyArchive = Join-Path $stickyWork 'libsodium-1.0.22.tar.gz'
if (!(Test-Path $stickyArchive)) {
  Invoke-WebRequest 'https://download.libsodium.org/libsodium/releases/libsodium-1.0.22.tar.gz' -OutFile $stickyArchive
}
if ((Get-FileHash -LiteralPath $stickyArchive -Algorithm SHA256).Hash -ne 'ADBDD8F16149E81AC6078A03ACA6FC03B592B89EF7B5ED83841C086191BE3349') { throw 'libsodium source checksum mismatch.' }
$stickySource = Join-Path $stickyWork 'libsodium-1.0.22'
if (!(Test-Path $stickySource)) { & tar -xzf $stickyArchive -C $stickyWork; if ($LASTEXITCODE) { throw 'Source extraction failed.' } }
$stickyCmakeDir = Get-ChildItem "$env:ANDROID_HOME/cmake" -Directory | Sort-Object Name -Descending | Select-Object -First 1
if (!$stickyCmakeDir) { throw 'Install Android SDK CMake with SDK Manager.' }
$stickyCmake = Join-Path $stickyCmakeDir.FullName 'bin/cmake.exe'
$stickyNinja = Join-Path $stickyCmakeDir.FullName 'bin/ninja.exe'
$stickyBuild = Join-Path $stickyWork $Abi
& $stickyCmake -S "$PSScriptRoot/native-sodium" -B $stickyBuild -G Ninja "-DCMAKE_MAKE_PROGRAM=$stickyNinja" "-DCMAKE_TOOLCHAIN_FILE=$env:NDK_HOME/build/cmake/android.toolchain.cmake" "-DANDROID_ABI=$Abi" '-DANDROID_PLATFORM=android-24' '-DCMAKE_BUILD_TYPE=Release' "-DSODIUM_SOURCE=$stickySource"
if ($LASTEXITCODE) { throw 'Android libsodium configuration failed.' }
& $stickyCmake --build $stickyBuild --parallel 4
if ($LASTEXITCODE) { throw 'Android libsodium build failed.' }
$env:SODIUM_LIB_DIR = $stickyBuild
Write-Host "SODIUM_LIB_DIR=$stickyBuild (use this environment only for Android builds)"
