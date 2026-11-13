# Windowsのアプリ内更新

Windowsインストーラー（`.exe`）はAuthenticode未署名で配布し、更新ファイルには無料で生成したTauriの鍵による署名（`.exe.sig`）を付けます。有料証明書や自己署名のWindows証明書は使用しません。Windowsの発行元不明・SmartScreenの警告は表示される場合があります。

アプリは起動時、1時間ごと、画面への復帰・ネットワーク復帰時に新しいバージョンを確認します。更新がある場合はメモ一覧の上部に通知します。「更新する」を押すと、保存完了を待ってダウンロード・署名検証・インストールを行い、アプリを再起動します。署名に含まれるバージョンも検証し、更新情報と異なる版は拒否します。編集中や同期中は更新ボタンを無効にします。失敗時は通知から再試行できます。

確認先は `https://github.com/sskohei/GitSticky/releases/latest/download/latest.json` です。リポジトリとReleaseアセットは認証なしで取得できる必要があります。GitHub同期用トークンは更新配信に使用しません。Androidはこの更新方式の対象外です。

## 初回の配信設定

1. 初回だけ、署名鍵の作成とGitHub Actionsへの登録を実行します。GitHub CLIのログインと、このリポジトリのActions設定の変更権限が必要です。

   ```powershell
   pnpm.cmd release:setup -RegisterGitHub
   ```

2. スクリプトは `%USERPROFILE%/.tauri/gitsticky/` に鍵を保存し、次を登録します。既存の鍵は再利用し、GitHubに異なる公開鍵がある場合は上書きせず停止します。
   - Variable `STICKYNOTE_UPDATER_PUBLIC_KEY`: `updater.key.pub` ファイルの内容。
   - Secret `TAURI_SIGNING_PRIVATE_KEY`: 暗号化済みの `updater.key` ファイルの内容。
   - Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: 自動生成した鍵のパスワード。

   保存先ディレクトリは現在のWindowsユーザーとSYSTEMだけがアクセスできるようにします。ローカルのパスワードは `password.dpapi` にWindows DPAPIで保護して保存し、鍵生成時のCLI出力はログに残しません。秘密鍵とパスワードは下記の手順でバックアップし、以降のリリースにも同じ鍵を使用します。

3. `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` とCargo.lockのバージョンを揃え、`pnpm.cmd release:check` で確認し、`v0.1.1` などの対応するタグをpushします。Actionsは署名付きインストーラー、署名、`latest.json` を含むドラフトReleaseを作成します。初回も同じ手順です。公開までのチェックリストは [Windows公開手順](windows-release.md) を参照してください。
4. Releaseの内容を確認し、公開します。公開後、以前にこの機能と同じ公開鍵を組み込んだアプリが通知を表示します。既存の更新機能を持たないアプリには、一度だけインストーラーでの手動更新が必要です。

通常の開発ビルドは公開鍵を含まないため更新確認を行いません。空の鍵で配信ビルドを作ることはできません。

## ローカルで更新署名付きビルド

```powershell
pnpm.cmd release:build
```

スクリプトは保存した鍵を読み、公開鍵を埋め込んだアプリと `.exe.sig` を生成します。ビルド後にAuthenticodeが未署名であること、更新署名が公開鍵で検証できること、署名のバージョンが一致することを確認します。生成する設定ファイルはgit管理対象外で、環境変数は処理終了時に元に戻します。RustのMSVCツールチェーンがPATHに必要です。

`--no-sign` は付けないでください。Tauriでは更新用署名もスキップされます。Windowsの発行元署名は `certificateThumbprint` / `signCommand` を未設定にすることで無効にしています。CIも同じ構成でビルドし、生成した署名を検証します。

## 鍵のバックアップ

`updater.key` と `updater.key.pub` を安全な保管先へコピーし、パスワードもパスワード管理アプリに保存してください。`password.dpapi` だけをコピーしても別のWindowsアカウントやPCでは復号できません。ローカルのパスワードをクリップボードへ取り出す場合は、自分のPowerShellで次を実行し、パスワード管理アプリへ貼り付けます。

```powershell
. ./scripts/updater-credentials.ps1
$stickyBackup = Get-StickyUpdaterCredentials -KeyDirectory "$env:USERPROFILE/.tauri/gitsticky"
$stickyBackup.Password | Set-Clipboard
$stickyBackup = $null
```

貼り付けが終わった後に、クリップボードを消します。

```powershell
Set-Clipboard -Value ''
```

別PCへ復元するときは鍵2ファイルを同じディレクトリへ戻し、保存したパスワードをそのPCでDPAPI保護します。

```powershell
Read-Host 'Updater key password' -AsSecureString |
  ConvertFrom-SecureString |
  Set-Content "$env:USERPROFILE/.tauri/gitsticky/password.dpapi"
```

## 配信前の実機確認

テスト用の公開リポジトリへエンドポイントを変更し、同じ鍵で署名した旧版・新版を用意してください。旧版をインストールして新版のReleaseを公開し、通知、クリック後の再起動、メモと認証情報の保持、ネットワーク切断後の再試行を確認します。不正な署名の配信物ではインストールされないことも確認します。自動テストでは更新APIをモックし、実際のインストーラーの起動は行いません。

仕様の参考: [Tauri Updater](https://v2.tauri.app/plugin/updater/) / [GitHub配信](https://v2.tauri.app/distribute/pipelines/github/)
