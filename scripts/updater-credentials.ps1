function Get-StickyUpdaterCredentials {
  param([Parameter(Mandatory = $true)][string]$KeyDirectory)
  $stickyKey = Join-Path $KeyDirectory 'updater.key'
  $stickyPasswordFile = Join-Path $KeyDirectory 'password.dpapi'
  foreach ($stickyFile in @($stickyKey, "$stickyKey.pub", $stickyPasswordFile)) {
    if (!(Test-Path -LiteralPath $stickyFile -PathType Leaf)) {
      throw "Missing updater credential file: $stickyFile. Run scripts/setup-updater.ps1 first."
    }
  }
  $stickySecurePassword = Get-Content -LiteralPath $stickyPasswordFile -Raw | ConvertTo-SecureString
  return @{
    PrivateKey = [IO.File]::ReadAllText($stickyKey).Trim()
    PublicKey = [IO.File]::ReadAllText("$stickyKey.pub").Trim()
    Password = [Net.NetworkCredential]::new('', $stickySecurePassword).Password
  }
}
