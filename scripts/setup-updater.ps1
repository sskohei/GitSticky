param(
  [string]$KeyDirectory = (Join-Path ([Environment]::GetFolderPath('UserProfile')) '.tauri/gitsticky'),
  [switch]$RegisterGitHub,
  [ValidatePattern('^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$')]
  [string]$Repository = 'sskohei/GitSticky'
)
$ErrorActionPreference = 'Stop'
$stickyRoot = Split-Path -Parent $PSScriptRoot
. "$PSScriptRoot/updater-credentials.ps1"
$stickyKey = Join-Path $KeyDirectory 'updater.key'
$stickyPasswordFile = Join-Path $KeyDirectory 'password.dpapi'

# Never silently replace the key that installed applications already trust.
if ((Test-Path -LiteralPath "$stickyKey.pub") -and !(Test-Path -LiteralPath $stickyKey)) {
  throw 'Only the public key exists. Restore the private key instead of generating a replacement.'
}
if ($RegisterGitHub) {
  $stickyVariables = & gh variable list --repo $Repository --json name,value | ConvertFrom-Json
  if ($LASTEXITCODE -ne 0) { throw 'Could not read GitHub Actions variables.' }
  $stickySecrets = & gh secret list --repo $Repository --json name | ConvertFrom-Json
  if ($LASTEXITCODE -ne 0) { throw 'Could not read GitHub Actions secrets.' }
  if (!(Test-Path -LiteralPath $stickyKey) -and (@($stickySecrets.name) -contains 'TAURI_SIGNING_PRIVATE_KEY')) {
    throw 'A signing secret already exists on GitHub. Restore the existing key before continuing.'
  }
}

if (!(Test-Path -LiteralPath $stickyKey)) {
  if (!(Test-Path -LiteralPath $KeyDirectory)) {
    $stickyDirectory = New-Item -ItemType Directory -Path $KeyDirectory
    $stickyAcl = [Security.AccessControl.DirectorySecurity]::new()
    $stickyAcl.SetAccessRuleProtection($true, $false)
    foreach ($stickyIdentity in @([Security.Principal.WindowsIdentity]::GetCurrent().User, [Security.Principal.SecurityIdentifier]::new('S-1-5-18'))) {
      $stickyRule = [Security.AccessControl.FileSystemAccessRule]::new($stickyIdentity, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
      $stickyAcl.AddAccessRule($stickyRule)
    }
    Set-Acl -LiteralPath $stickyDirectory.FullName -AclObject $stickyAcl
  }
  if (!(Test-Path -LiteralPath $stickyPasswordFile)) {
    $stickyRandom = New-Object byte[] 32
    $stickyRng = [Security.Cryptography.RandomNumberGenerator]::Create()
    try { $stickyRng.GetBytes($stickyRandom) } finally { $stickyRng.Dispose() }
    $stickyPassword = [Convert]::ToBase64String($stickyRandom) | ConvertTo-SecureString -AsPlainText -Force
    [IO.File]::WriteAllText($stickyPasswordFile, ($stickyPassword | ConvertFrom-SecureString))
  }
  $stickySecurePassword = Get-Content -LiteralPath $stickyPasswordFile -Raw | ConvertTo-SecureString
  $stickyPreviousPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  try {
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Net.NetworkCredential]::new('', $stickySecurePassword).Password
    # The Tauri CLI prints key material. Capture and discard all of its output.
    $stickyKeygenOutput = & node "$PSScriptRoot/updater-keygen.mjs" $stickyKey 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'Updater key generation failed. No key material was logged.' }
  } finally {
    $stickyKeygenOutput = $null
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $stickyPreviousPassword
  }
}
$stickyCredentials = Get-StickyUpdaterCredentials -KeyDirectory $KeyDirectory
try {
  if ($RegisterGitHub) {
    $stickyExisting = @($stickyVariables | Where-Object name -eq 'STICKYNOTE_UPDATER_PUBLIC_KEY')
    if ($stickyExisting.Count -gt 0 -and $stickyExisting[0].value.Trim() -ne $stickyCredentials.PublicKey) {
      throw 'The GitHub public key differs. Refusing to rotate the updater key.'
    }
    foreach ($stickySetting in @(
      @{ Kind = 'secret'; Name = 'TAURI_SIGNING_PRIVATE_KEY'; Value = $stickyCredentials.PrivateKey },
      @{ Kind = 'secret'; Name = 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD'; Value = $stickyCredentials.Password },
      @{ Kind = 'variable'; Name = 'STICKYNOTE_UPDATER_PUBLIC_KEY'; Value = $stickyCredentials.PublicKey }
    )) {
      # Write stdin directly: a PowerShell pipeline adds a newline to the password.
      $stickyProcess = [Diagnostics.Process]::new()
      try {
        $stickyProcess.StartInfo.FileName = (Get-Command gh -CommandType Application).Source
        $stickyProcess.StartInfo.Arguments = "$($stickySetting.Kind) set $($stickySetting.Name) --repo $Repository"
        $stickyProcess.StartInfo.UseShellExecute = $false
        $stickyProcess.StartInfo.CreateNoWindow = $true
        $stickyProcess.StartInfo.RedirectStandardInput = $true
        $stickyProcess.StartInfo.RedirectStandardOutput = $true
        $stickyProcess.StartInfo.RedirectStandardError = $true
        [void]$stickyProcess.Start()
        $stickyProcess.StandardInput.Write($stickySetting.Value)
        $stickyProcess.StandardInput.Close()
        $stickyProcess.StandardOutput.ReadToEnd() | Out-Null
        $stickyProcess.StandardError.ReadToEnd() | Out-Null
        $stickyProcess.WaitForExit()
        if ($stickyProcess.ExitCode -ne 0) { throw "Could not register GitHub setting: $($stickySetting.Name)" }
      } finally { $stickyProcess.Dispose() }
      Write-Output "Registered $($stickySetting.Name) on $Repository."
    }
  }
  Write-Output "Updater key ready: $stickyKey (existing keys are reused)."
  Write-Output 'Back up updater.key, updater.key.pub and the password in a password manager. password.dpapi can only be decrypted by this Windows account.'
} finally {
  $stickyCredentials = $null
  $stickySetting = $null
}
