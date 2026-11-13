# Windows版の公開

配布先は [GitHub Releases](https://github.com/sskohei/GitSticky/releases)、配布形式はWindows x64向けNSISインストーラーです。Windows 10 / 11を対象とします。アプリ名と識別子は既存データを引き継ぐため `StickyNote` / `com.gitsticky.stickynote` を維持します。

個人利用向けに費用をかけず、Windowsの発行元署名（Authenticode）は付けません。アプリ内更新には自分で無料生成したTauriの鍵による署名を使います。有料証明書の購入や自己署名証明書のWindowsへの登録は不要です。

CIとReleaseのコンパイラは、ローカル検証と揃えてRust 1.99.1に固定します。新しいRustへの更新は、テストとWindowsビルドを確認してから明示的に行います。

## タグを付ける前

```powershell
pnpm.cmd install --frozen-lockfile
pnpm.cmd release:check
pnpm.cmd typecheck
pnpm.cmd lint
pnpm.cmd test --maxWorkers=1
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --lib --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests --examples --locked -- -D warnings
pnpm.cmd tauri build --bundles nsis -- --locked
```

Rustの環境がPATHにない場合は、インストール済みのMSVCツールチェーンを設定してください。`scripts/env.ps1` はこの作業ディレクトリの `.tools` がある場合に利用できます。

`release:check` はアプリ・package.json・Cargo.toml・Cargo.lockのバージョン一致、安定版の形式、タグとの一致、NSIS・ライセンス・HTTPS更新URLを確認します。バージョン変更時は `cargo check --manifest-path src-tauri/Cargo.toml` などでCargo.lockも更新してコミットします。

`Windows checks` ワークフローはPR、mainへのpush、手動実行に対応します。型・Lint・テスト・Rust書式・Clippy・NSISビルドを実行し、署名なしの確認用インストーラーをActionsのArtifactに7日間保存します。これは更新用の公開鍵を含まないため、アプリ内更新の実機検証には署名付きビルドを使用します。

## 署名とドラフトRelease

1. 初回だけ `pnpm.cmd release:setup -RegisterGitHub` で無料の更新用署名鍵を生成し、GitHub ActionsのVariableとSecretsを設定します。秘密鍵はリポジトリ外に暗号化保存します。鍵とパスワードをバックアップし、同じアプリの更新には同じ鍵を使い続けます。[保存先とバックアップ手順](updates.md)。
2. 通常のCIが成功したコミットで `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` とCargo.lockのバージョンを揃えます。初回は `0.1.0`、次回は `0.1.1` などにします。
3. 対応する `v0.1.0` などのタグをpushします。`Windows release` はチェックを実行し、署名付きの更新アセットと `latest.json` を含むドラフトを作成します。手動実行でも同じバージョンのドラフトが対象となるため、公開済みの版を再ビルドする用途では使いません。
4. ドラフトからインストーラーをダウンロードし、下の実機確認を行います。変更内容・既知の制約・テスト結果をRelease本文に記載します。初回の本文案は [v0.1.0リリースノート](release-notes-v0.1.0.md) にあります。
5. `.exe`、`.exe.sig`、`latest.json` が揃い、JSONの `windows-x86_64` に正しいURLと署名があることを確認してReleaseを公開します。公開済みアセットの差し替えを避け、修正は新しいバージョンで配信します。

更新用署名はTauriがダウンロードしたアセットとバージョンを検証するためのものです。Windowsの発行元表示に使うAuthenticodeコード署名とは別です。個人用の配布方針としてAuthenticode証明書は設定しないため、インストーラーに発行元不明・SmartScreenの警告が表示される場合があります。`release:check` とビルド後の検証でこの方針を確認します。ローカルで配信用の `.exe` と `.exe.sig` を作る場合は `pnpm.cmd release:build` を実行してください。

## 公開前の実機確認

ユーザーの実データを変更しないテスト用WindowsアカウントまたはVMを使います。

- 初回インストール: 日本語・英語の選択、ライセンス表示、起動、スタートメニュー、WebView2導入を確認。
- 保存: 日本語タイトル・Markdown・タグを入力し、閉じる直前の編集も通常終了後に保持されることを確認。
- GitHub: テスト用非公開リポジトリに接続し、暗号鍵登録、アップロード、別端末からの取得、競合時のコピー、切断後のローカル保持を確認。
- 更新: 同じ鍵を組み込んだ旧版から、通知、保存待ち、インストール、再起動、メモ・認証・暗号鍵の保持を確認。通信失敗・不正署名時に更新されず再試行できることも確認。詳しい手順は [updates.md](updates.md)。
- アンインストール・再インストール: ショートカットの削除とアプリデータの扱いを確認。テスト前に `%APPDATA%/com.gitsticky.stickynote/` の控えを取る。認証情報は端末間コピーに使用しない。

GitHub同期やOSインストーラーを実際に使う確認は、モックの自動テストでは代替できません。検証済み項目と未実施項目は [testing.md](testing.md) に記録します。

## 初回公開の残作業

2026-11-13に更新用の鍵を作成し、Actionsの更新署名用Variable / Secretsを登録しました。GitHubリポジトリは公開されており、Releaseはまだありません。鍵・パスワードの外部バックアップ、署名付きの旧版から新版への実機更新テスト、テスト用GitHubアカウントを使う端末間同期確認、Release本文の確認・公開が残っています。既存の署名鍵は、新しく作り直さず同じ鍵を使用してください。

参考: [Windowsインストーラー](https://v2.tauri.app/distribute/windows-installer/) / [Updater](https://v2.tauri.app/plugin/updater/) / [GitHub Actions配布](https://v2.tauri.app/distribute/pipelines/github/)
