# StickyNote

Windows / Android向けの、GitHub同期型付箋メモアプリです。メモは端末のSQLiteに保存し、ユーザー自身の非公開GitHubリポジトリに、暗号化した `notes/{UUID}.md` として同期します。Git CLIや追加のサーバーは不要です。

## 主な機能

- 日本語UI、レスポンシブな付箋グリッド、6色、ピン留め、本文・タイトル検索
- 複数タグの追加・解除、既存タグの候補表示、タグ名の検索、複数タグのAND絞り込み・タグなし表示
- CodeMirror 6のMarkdown編集とライブプレビューの同時表示、編集／分割／プレビュー切替、Undo / Redo、500ms自動保存、Ctrl+S
- オフラインでの作成・編集・削除、再起動後の永続化
- SHAによる双方向同期、削除マーカー、競合コピー、同期中の追加編集の保護
- 手動同期、設定で有効にした場合の起動・復帰・編集後15秒の自動同期（新規設定では無効）
- Light / Dark / Systemテーマ、GitHub Appでのログイン、認証の自動更新
- AES-256-GCMによるメモのローカル・同期時の暗号化、Windows DPAPI / Android Keystoreによる鍵と認証情報の保護

Tauri v2 / React / TypeScript (strict) / Vite / pnpm / Tailwind CSS / shadcn形式のRadix UI / Lucide / Zustand / SQLite (SQLx) / Rust / reqwestを使用します。

## Windowsでの開発

Node.js 22.12以上（検証環境は24）、pnpm 10、Rust stable 1.94以上のMSVCツールチェーン、Visual Studio C++ Build Tools、WebView2を用意してください。[Tauri公式の前提条件](https://v2.tauri.app/start/prerequisites/)も参照してください。

```powershell
pnpm.cmd install --frozen-lockfile
pnpm.cmd tauri dev
```

PowerShellの実行ポリシーに制約がある環境では `.cmd` を使用します。今回の作業用Rust環境が `.tools` にある場合は、次のようにプロセス内で環境を読み込めます（OSの実行ポリシーは変更しません）。通常のRust環境では不要です。

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -Command ". ./scripts/env.ps1; pnpm.cmd tauri dev"
```

`pnpm.cmd dev` はViteだけを起動します。開発サーバーは既定で127.0.0.1に限定します。Android実機から接続する場合だけ、`TAURI_DEV_HOST`に開発PCのLANアドレスを設定してください。ブラウザ単体ではSQLiteや保管庫を利用できないため、アプリを使うにはTauriで起動してください。

## Androidでの開発

Android Studio、Android SDK（生成プロジェクトのcompileSdkに対応するPlatform）、SDK Build Tools、NDK、互換性のあるJDKをWindows上にインストールしてください。`JAVA_HOME`、`ANDROID_HOME`、`NDK_HOME`を設定し、SDKのライセンスを承認します。WSLのパスは使用しません。

```powershell
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
node node_modules/@tauri-apps/cli/tauri.js android dev
node node_modules/@tauri-apps/cli/tauri.js android build --debug --target aarch64 --apk --ci
```

WindowsからのARM64 APKビルドでは、Stronghold依存のlibsodiumをNDKで先に構築する補助スクリプトを使用できます。公式1.0.22のソースをSHA-256で確認し、OS設定を変更せずビルドします。Tauriがシンボリックリンク権限で失敗した場合は、完成した同じ共有ライブラリをコピーしてGradleでパッケージ化します。他のビルドエラーはそのまま失敗として報告します。Android SDKのCMakeも必要です。

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/android-build.ps1
```

この補助スクリプトの成果物はARM64のデバッグAPKです。標準CLIを使う場合は `. ./scripts/android-sodium.ps1` を同じPowerShellプロセスで先に実行してください。`SODIUM_LIB_DIR` はAndroid用なので、Windowsビルドとは別のシェルを使用します。[libsodiumの公式ビルド方針](https://doc.libsodium.org/installation)を参照し、移植性を優先したC実装を使用しています。

`src-tauri/gen/android` のGradleプロジェクトを同梱しています。再生成する場合は `node node_modules/@tauri-apps/cli/tauri.js android init --ci` を実行し、`windowSoftInputMode="adjustResize"`、`SecureAuthPlugin.kt`、バックアップ除外XMLとManifest参照、シンボル除去設定を維持してください。Windowsのpnpmシム経由でモバイルCLIのパス引用が失敗する場合は、上記の `node` 直接呼び出しを使用します。

## GitHub接続

1. アプリの設定で「GitHubで接続」を押します。
2. ブラウザのGitHubで、アプリに表示された短いコードを入力して認可します。
3. 「リポジトリの許可」から対象リポジトリをGitHub Appに許可します。
4. アプリで一覧を更新し、非公開リポジトリとブランチを選びます。
5. 最初の端末では同期用の暗号鍵を作成して安全な場所へ控え、登録します。他の端末では同じ鍵を登録します。
6. 「このリポジトリに接続」を押し、必要に応じて自動同期を有効にします。

PATの作成・貼り付けや、起動ごとのパスフレーズ入力は不要です。トークンの取得・更新・OS保護領域への保存をRustが担当し、Reactに認証トークンを返しません。新しい端末では同じGitHub App・リポジトリ・ブランチへ接続し、同じ同期用の暗号鍵を登録してください。

同梱の `src-tauri/github-app.json` にはGitHub Appの公開Client ID / slugを設定済みです。別のGitHub Appを使う場合は、[登録・ビルド設定の手順](docs/github-auth.md)に従って設定を変更してください。未設定ビルドの接続ボタンは無効です。Client SecretやApp秘密鍵は不要です。

接続時にはリポジトリとブランチを確認し、書き込み時にも権限エラーを判定します。エラー時もローカル変更は維持します。旧Stronghold保管庫は残し、必要な場合だけ一度のパスフレーズ入力で移行できます。

接続先変更時にはローカルメモを新しいリポジトリと統合します。前の接続先のSHAは再利用しません。既存リポジトリの `README.md` や対応UUID以外のファイルは管理対象外です。

メモの暗号化・既存データの移行・鍵の保管・過去のGit履歴に残る平文については、[セキュリティと移行の説明](docs/security.md)を確認してください。

## テストとビルド

```powershell
pnpm.cmd typecheck
pnpm.cmd lint
pnpm.cmd test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm.cmd build
pnpm.cmd tauri build --bundles nsis
```

`cargo test --release --manifest-path src-tauri/Cargo.toml --lib` でも全Rustユニットテストを実行できます。開発ビルドでも主要な暗号プリミティブを最適化し、Strongholdの暗号化強度を維持したまま待ち時間を抑えています。

Windowsの成果物は `src-tauri/target/release/bundle/nsis/`、Android APKは `src-tauri/gen/android/app/build/outputs/apk/` に生成されます。リリース署名と配布用証明書は利用者側で設定してください。

## データと制約

- SQLiteとWindowsの `github-auth.dpapi` はTauriのアプリ設定ディレクトリに保存します。Windowsでは通常 `%APPDATA%/com.gitsticky.stickynote/` です。旧 `token.stronghold` / `vault.salt` は移行用に保持します。認証情報は他の端末へコピーせず、端末ごとにGitHubで接続してください。
- アプリの強制終了・OS停止で、まだ500msの保存を終えていない入力が残る保証はありません。通常の画面／Windowsウィンドウを閉じる操作では保存完了を待ちます。
- タグは1個30文字、1メモ20個までです。前後の空白を除去し、ASCII英字の大文字・小文字は同じタグとして扱います。タグ付きメモの同期には、すべての端末でタグ対応版を使用してください。旧版はタグ付きメモの読み書きを中止してデータを保護します。
- Androidの完全終了中の常駐同期、画像添付、タグの階層化・色分け・一括変更、フォルダ、自由配置、リアルタイム共同編集は対象外です。
- Markdownの生HTMLは描画せず、プレビューから外部画像を読み込みません。
- 不正Front Matter、5MB超のメモ、不完全なリモート一覧は安全のため同期を拒否します。全リモートメモを取得するMVPのため、大規模リポジトリには向きません。
- 競合は3-way mergeせず、ローカル版を「（競合コピー）」として保存し、元メモをGitHub版に更新します。コピーは次回同期対象です。
- 実リポジトリへの接続と端末間同期の検証には、登録済みGitHub Appと利用者によるブラウザ認可が必要です。実施状況は [検証記録](docs/testing.md) を参照してください。

設計: [architecture](docs/architecture.md) / [database](docs/database.md) / [github-sync](docs/github-sync.md) / [ui-spec](docs/ui-spec.md) / [development](docs/development.md)
