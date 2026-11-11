# GitHub App認証の設定

標準フローは「GitHubで接続」→ブラウザで短いコードを入力→リポジトリを許可→アプリの一覧から同期先を選択、です。利用者がPATやパスフレーズを作成する必要はありません。

このリポジトリの `src-tauri/github-app.json` にはGitHub App `gitstickynote` の公開Client ID / slugを設定済みです。別のGitHub Appを使う場合は、以下の手順で登録して公開情報を変更してください。未設定のビルドではログインを無効化し、その状態を画面に表示します。ローカルメモは利用できます。

## 開発者が一度だけ行う登録

1. [GitHub App登録画面](https://github.com/settings/apps/new)を開きます。OAuth Appではなく **GitHub App** を作成します。
2. App名は利用可能な名前を指定します。Homepage URLはこのプロジェクトのURLなどを指定します。
3. **Enable Device Flow** を有効にします。
4. **Expire user authorization tokens** を有効のままにします。Web認証用のCallback URLはDevice Flowでは使用しません。「Request user authorization (OAuth) during installation」は使用しません。
5. Webhookの **Active** をオフにします。WebhookサーバーやWebhook secretは不要です。
6. Repository permissionsは **Contents: Read and write** と、自動付与される **Metadata: Read-only** だけにします。Account permissions / Organization permissions / Eventsは追加しません。
7. 他のGitHubアカウントにも配布する場合は、インストール対象を **Any account** にします。自分だけで試すなら自分のアカウントに限定しても構いません。
8. 登録後に表示される **Client ID** と、公開App URL `https://github.com/apps/APP-SLUG` の **APP-SLUG** を控えます。App IDとは異なります。
9. 登録後に「You must generate a private key in order to install your GitHub App」と表示されたら、同じ設定ページの **Private keys → Generate a private key** を押します。ダウンロードされる `.pem` は開発者が安全に保管します。その後、左側の **Install App** から自分のアカウントとメモ用リポジトリを選択できます。

Client SecretとApp秘密鍵は、このアプリのDevice Flowと更新には使用しません。アプリへ埋め込んだり、この設定ファイルへ記載したりしないでください。

GitHubの登録画面で求められる秘密鍵の生成と、StickyNoteに秘密鍵を渡すことは別です。生成した `.pem` はGitへコミットせず、チャットにも送信しないでください。

## ビルドに設定する公開情報

`src-tauri/github-app.json` に公開情報だけを設定します。

```json
{
  "client_id": "GitHubで表示されたClient ID",
  "slug": "公開App URLの末尾"
}
```

設定後、Windows / Androidを再ビルドします。両方に同じAppを設定できます。

```powershell
. ./scripts/env.ps1
pnpm.cmd tauri build --bundles nsis
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/android-build.ps1
```

CIなどでは `STICKYNOTE_GITHUB_CLIENT_ID` / `STICKYNOTE_GITHUB_APP_SLUG` のビルド時環境変数で上書きできます。これらは秘密情報ではありません。環境変数とファイルの変更はCargoの再ビルド対象です。

## 利用者の接続

1. アプリ設定で「GitHubで接続」を押します。
2. 開いたGitHubのページで、アプリに表示された短いコードを入力し、自分が開始した認証を許可します。GitHubへのログインと認可は利用者がブラウザで行います。
3. アプリが接続済みに変わったら「リポジトリの許可」からGitHub Appをインストールし、**Only select repositories** でメモ用リポジトリを選びます。
4. アプリへ戻り「一覧を更新」→リポジトリとブランチを選択→「このリポジトリに接続」を押します。
5. 設定を閉じると、自動同期が有効なら同期を開始します。別端末も同様に接続します。

空のリポジトリには、READMEの初期化などで対象ブランチを先に作成してください。ブランチ保護で直接書き込めない場合は、メモ用の書き込み可能なブランチを選んでください。

認証情報はWindowsではユーザー単位のDPAPIで暗号化した `github-auth.dpapi`、AndroidではKeystoreのAES-GCMで暗号化したアプリ専用SharedPreferencesに保存します。端末に認証情報を残せない場合は接続を成功扱いにしません。Androidでは認証情報・メモ・暗号鍵の自動バックアップと端末移行を無効にします。暗号化したメモの移行にはGitHub同期と控えた同期用鍵を使用します。

アクセストークンは期限の60秒前または401応答時に自動更新します。更新は直列実行し、同じ古いトークンへの並行401で複数回更新しません。refresh tokenの期限切れ・権限取り消し時は再認証が必要です。端末の認証が復号できない場合は「端末の認証をリセット」で認証のみを消して接続し直せます。メモは保持されます。

「この端末の接続を解除」は端末内の認証を削除する操作です。GitHub側のAppインストールや認可の取り消しはGitHub設定から行ってください。

## 以前のStrongholdからの移行

以前の `token.stronghold` が見つかる場合だけ「以前の保管庫から認証を移行」を表示します。一度だけ以前のパスフレーズを入力すると、既存PATをOS保護領域へ移行できます。元の保管庫・salt・メモは削除も更新もしません。移行したPATにはOAuthの自動更新は適用できないため、新しいGitHub App認証への移行を推奨します。

## 一次資料

- [GitHub App登録](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/registering-a-github-app)
- [Device Flow](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
- [Device Flowトークンの更新（client secret不要）](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens)
- [Windows DPAPI](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
- [Android Keystore](https://developer.android.com/privacy-and-security/keystore)
