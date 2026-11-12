# 開発ガイド

Windows / Android向けのStickyNoteを開発するための手順です。参加の流れは [CONTRIBUTING.md](../CONTRIBUTING.md)、仕様は [ドキュメント一覧](README.md) を参照してください。

## Windowsの前提条件

| ツール                    | 使用する構成                                            |
| ------------------------- | ------------------------------------------------------- |
| Git                       | clone・ブランチ作成・変更提出に使用                     |
| Node.js                   | 22.12以上。既存の検証環境は24系                         |
| pnpm                      | `package.json` の指定に合わせて10.30.3                  |
| Rust                      | stable 1.94以上、Windows MSVC（`src-tauri/Cargo.toml`） |
| Visual Studio Build Tools | 「C++によるデスクトップ開発」とWindows SDK              |
| WebView2                  | Tauriの画面表示に使用                                   |

導入方法は [Tauri公式の前提条件](https://v2.tauri.app/start/prerequisites/) を参照してください。WindowsネイティブのPowerShellを使います。現在のTauri設定は `pnpm.cmd` を呼ぶため、この手順ではmacOS / Linux / WSLでのビルドを扱いません。

Node.js導入後、pnpmがない場合や版が異なる場合は以下で準備します。

```powershell
npm.cmd install --global pnpm@10.30.3
node --version
pnpm.cmd --version
rustc --version
cargo --version
rustup show active-toolchain
```

## cloneから初回起動まで

以下は直接pushできる開発者の例です。外部から参加する場合は先にGitHubでforkし、clone先を自分のforkのURLに置き換えてください。

```powershell
git clone https://github.com/sskohei/GitSticky.git
cd GitSticky
pnpm.cmd install --frozen-lockfile
pnpm.cmd tauri dev
```

以降も特記がなければ `package.json` のあるルートで実行します。初回はRust依存関係の取得・コンパイルに時間がかかります。

ウィンドウが開いたら、GitHubへ接続せずにメモを作成し、日本語やMarkdownを入力して閉じます。通常終了して再起動し、メモが残ればローカル開発の準備は完了です。ローカル機能と自動テストにはGitHub認証やAppの新規登録は不要です。

`pnpm.cmd dev` はViteのみを起動します。ブラウザ単体ではSQLiteやOS保護領域を利用できないため、アプリの確認にはTauriで起動してください。

`scripts/env.ps1` は任意の環境補助です。`.tools/cargo/bin/cargo.exe` がある場合だけローカルのRustをPATHへ追加し、Android SDKなども検出します。cloneしただけでは `.tools` にツールは入りません。通常のRustインストールがあれば不要です。

## コードを探す

| 変更内容                   | 主な入口                                                                                              |
| -------------------------- | ----------------------------------------------------------------------------------------------------- |
| 付箋一覧・検索・フィルター | `src/features/notes/Board.tsx`、`src/stores/app.ts`                                                   |
| Markdown編集・保存・タグ   | `src/features/editor/`、`src/lib/tags.ts`                                                             |
| 設定・ログイン             | `src/features/settings/Settings.tsx`、`src/stores/auth.ts`                                            |
| 同期の差分判定・実行       | `src/features/sync/engine.ts`、`useSync.ts`、`format.ts`                                              |
| DB・IPC窓口と型            | `src/db/index.ts`、`src/lib/native.ts`、`src/types/index.ts`                                          |
| SQLite・トランザクション   | `src-tauri/src/database.rs`、`commands.rs`、`src-tauri/migrations/`                                   |
| GitHub通信・commit更新     | `src-tauri/src/github.rs`、`repository.rs`                                                            |
| 認証・鍵・復旧             | `src-tauri/src/auth.rs`、`oauth.rs`、`secure_store.rs`、`crypto.rs`、`sync_security.rs`、`backups.rs` |
| Tauriコマンド・権限        | `src-tauri/src/lib.rs`、`src-tauri/build.rs`、`src-tauri/permissions/`、`src-tauri/capabilities/`     |
| Android固有処理            | `src-tauri/gen/android/app/src/main/`、`scripts/android-*.ps1`                                        |

ReactはUIと同期の実行制御、RustはDB・GitHub通信・秘密情報を担当します。詳しくは [アーキテクチャ](architecture.md) を参照してください。

## 検証コマンド

| 目的                     | コマンド                                                                                  |
| ------------------------ | ----------------------------------------------------------------------------------------- |
| TypeScript型チェック     | `pnpm.cmd typecheck`                                                                      |
| ESLint                   | `pnpm.cmd lint`                                                                           |
| フロントエンド全テスト   | `pnpm.cmd test`                                                                           |
| 特定のテストを実行       | `pnpm.cmd exec vitest run src/features/editor/Editor.test.tsx`                            |
| フロントエンド本番ビルド | `pnpm.cmd build`                                                                          |
| Rustユニットテスト       | `cargo test --manifest-path src-tauri/Cargo.toml --lib --locked`                          |
| Rust Clippy              | `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests --locked -- -D warnings` |
| 文書の整形確認例         | `pnpm.cmd exec prettier --check README.md CONTRIBUTING.md docs/development.md`            |

Vitestはjsdomで動作します。UIテストは対象コードの近く、共通設定は `src/test/setup.ts`、DBなどのfakeは `src/test/fakes.ts` にあります。Rustテストは `src-tauri/src/*tests.rs` と各モジュール内を参照してください。手動確認と過去の実施結果は [検証記録](testing.md) にあります。

## GitHub同期を開発する

現行認証はGitHub App Device Flowです。公開Client ID / slugは同梱済みで、自分のAppを使う場合のみ [GitHub App設定](github-auth.md) に従います。

検証専用の非公開リポジトリを用意し、READMEなどで初回commitとブランチを作成します。アプリから認可し、そのリポジトリをAppに許可してください。設定画面で同期用鍵を作成・控え・登録したうえで、リポジトリとブランチへ接続します。端末間の検証には同じ同期用鍵が必要です。

同期では暗号化した `notes/{UUID}.md` と `.gitsticky-security.json` を更新します。同期先ファイルの手編集は整合性検証で拒否されるため、通常の確認はアプリを通してください。[同期仕様](github-sync.md) と [暗号化・鍵交換・復旧](security.md) も参照してください。

開発版も同じアプリ識別子を使い、Windowsでは通常 `%APPDATA%/com.gitsticky.stickynote/` に保存します。日常利用のデータと分離する場合は別のWindowsユーザーを使ってください。実データ、トークン、同期鍵はfixtureやIssueへ含めないでください。

## Androidの追加セットアップ

Windowsの前提条件に加え、Android StudioとSDK ManagerからSDK Platform（現在の `compileSdk = 37`）、Build Tools、NDK、CMakeを導入し、Gradleと互換性のあるJDKを用意します。過去の環境の版は [検証記録](testing.md) を参照してください。`JAVA_HOME`、`ANDROID_HOME`、`NDK_HOME` を実際のインストール先に設定し、SDKライセンスを承認します。

```powershell
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

`src-tauri/gen/android` は追跡済みのカスタマイズと、追跡されないローカル生成ファイルを使います。新しいcloneでは以下でTauriの接続ファイルを生成し、`git diff -- src-tauri/gen/android` で既存の変更を確認してください。

```powershell
node node_modules/@tauri-apps/cli/tauri.js android init --ci
```

再生成後も `SecureAuthPlugin.kt`、Manifestのバックアップ無効化と除外XML参照、`adjustResize`、Gradleのシンボル除去設定を維持してください。ローカルパスや署名用ファイルをコミットしないでください。

ARM64デバッグAPKには、libsodiumをNDK/CMakeで構築する補助スクリプトを使えます。

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/android-build.ps1
```

実機・エミュレーターで開発する場合はAndroid専用のPowerShellを開き、同じプロセスでlibsodiumを準備します。次はARM64向けです。他のABIでは `android-sodium.ps1 -Abi` も合わせます。

```powershell
. ./scripts/android-sodium.ps1
node node_modules/@tauri-apps/cli/tauri.js android dev
```

実機から接続するときは、起動前に `$env:TAURI_DEV_HOST` をPCの到達可能なLANアドレスへ設定します。`SODIUM_LIB_DIR` はAndroid用なので、Windowsビルドには別のPowerShellを使います。

## 配布用ビルド

```powershell
pnpm.cmd tauri build --bundles nsis
```

Windowsインストーラは `src-tauri/target/release/bundle/nsis/`、Android APKは `src-tauri/gen/android/app/build/outputs/apk/` に生成されます。`pnpm.cmd build` はフロントエンドだけをビルドします。Android補助スクリプトはARM64デバッグAPKを生成します。配布用署名は別途設定してください。

## つまずいたとき

| 症状                                  | 確認すること                                                                 |
| ------------------------------------- | ---------------------------------------------------------------------------- |
| PowerShellでpnpmが拒否される          | `pnpm` の代わりに `pnpm.cmd` を使用                                          |
| cargoやリンカーが見つからない         | Rust MSVC、C++ Build Tools、Windows SDKを導入し、シェルを開き直す            |
| ポート1420が使用中                    | 既存のViteなどを確認し、不要なものを終了                                     |
| ブラウザでDB操作ができない            | `pnpm.cmd tauri dev` で起動                                                  |
| Vitestの並列実行が重い                | `pnpm.cmd exec vitest run --maxWorkers=1` を使用                             |
| GitHubのブランチ一覧が空              | 初回commitとAppへの許可を確認                                                |
| Androidの生成ファイルが見つからない   | `android init --ci` と再生成後の差分を確認                                   |
| Androidのlibsodiumビルドが失敗        | SDKのCMake・NDKと環境変数を確認                                              |
| Androidのシンボリックリンク作成が失敗 | `scripts/android-build.ps1` で、Rustコンパイル後の該当エラーだけコピーで補完 |

解決しない場合は [貢献ガイド](../CONTRIBUTING.md) に従ってIssueへ環境・コマンド・秘密情報を除いたエラーを記載してください。初期仕様と当時の判断は [開発履歴](development-history.md) に保存しています。
