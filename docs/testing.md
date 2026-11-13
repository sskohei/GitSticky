# 検証記録

現在の実行コマンドは [開発ガイド](development.md)、PRでの検証の目安は [貢献ガイド](../CONTRIBUTING.md) を参照してください。以下の日付付き記録は当時の結果であり、新しい環境や変更後の検証を代替しません。

## 無料の更新署名の設定（2026-11-13、公開準備の続き）

- 個人利用の方針に合わせ、Authenticode証明書は未設定のまま、Tauriの更新署名用の鍵を生成した。秘密鍵はリポジトリ外の `%USERPROFILE%/.tauri/gitsticky/updater.key` にパスワード付きで保存し、パスワードはWindows DPAPIで保護。ディレクトリのACLは現在のユーザーとSYSTEMのみ。秘密鍵やパスワードはログ・ソースに出していない。
- GitHub `sskohei/GitSticky` に `STICKYNOTE_UPDATER_PUBLIC_KEY` Variable、`TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` Secretsを登録。セットアップ再実行で鍵が変わらないことも確認した。
- `release:setup` / `release:build` と生成物の検証スクリプトを追加。CIも更新署名とAuthenticode未署名の両方を確認する。署名されたバージョンが更新情報と一致することをアプリで必須にした。
- 実際のインストーラーを用い、正しい署名の受け入れ、改ざんしたファイル・誤公開鍵・不一致バージョン・改ざんした署名内バージョンの拒否を確認。検証にはアプリの更新プラグインと同じ `minisign-verify` を使用し、インストーラーを起動していない。
- 配布方針に反するAuthenticode設定、バージョン検証の無効化、不正形式の公開鍵を拒否することを確認。不正公開鍵で既存の有効な設定が上書きされないことも確認した。PowerShell全スクリプトの構文チェック、ESLint、Prettier、YAML解析が成功。
- 公開鍵を埋め込んだ `.exe` と `.exe.sig` を生成した。`.exe` は5,056,723 bytes、SHA-256は `120EADFD643127D22B280472BDD1072D4329BEA8D7E06D9CDC72341853155BA4`、Authenticodeは `NotSigned`、更新署名と署名内の `0.1.0` は検証成功。成果物は `src-tauri/target/release/bundle/nsis/` にある。
- ビルド済みアプリ本体に設定した公開鍵が埋め込まれていることを確認。`cargo fmt -- --check` と `cargo clippy --lib --tests --examples --locked -- -D warnings` も成功した。

鍵・パスワードの外部バックアップは利用者が行う必要がある。DPAPIファイルをコピーするだけでは別PCで復号できないため、[バックアップ手順](updates.md) を参照。GitHub Actionsのリモート実行、Release公開、OSでのインストール・更新、実GitHubでの端末間同期はこの作業では行っていない。

## Windows公開準備（2026-11-13）

- 型チェック・ESLint・フロントエンド本番ビルドが成功。Vitestは全70件成功。テスト探索を `src/**/*.test.{ts,tsx}` に限定した後、更新機能の8件も成功した。
- Rustの `cargo test --manifest-path src-tauri/Cargo.toml --lib --locked` は全50件成功。`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` と `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests --locked -- -D warnings` も成功。
- `pnpm.cmd release:check` が成功。一時ディレクトリに設定をコピーし、不一致のタグ・package.jsonのバージョンがそれぞれ失敗になることを確認。
- `pnpm audit` は脆弱性の報告0件。変更ファイルのPrettier、ワークフローのYAML解析、`git diff --check` が成功。
- 日本語・英語のNSIS設定、ライセンス表示、公開前チェック、PR用Windows CI、公開手順、初回Release本文案を追加。既存のドラフトReleaseワークフローにもバージョンチェック・固定lockfileでのビルドを適用した。
- `pnpm.cmd tauri build --bundles nsis -- --locked` が成功。確認用の `src-tauri/target/release/bundle/nsis/StickyNote_0.1.0_x64-setup.exe` を生成した（5,049,834 bytes、SHA-256: `B61A1A059A0E023466F5AAA9B7805E297A52CFF4A29D36DBB7CE61409D0F63C8`）。Authenticodeは `NotSigned`、更新用の公開鍵も含まないローカル確認用ビルド。
- ビルド時には従来のlibsodiumのPDB欠落（LNK4099）と、遅延読込するMarkdownエディタのチャンクサイズの警告が出る。リンク・インストーラー生成は成功した。

実際の署名付きアプリ更新、テスト用GitHubでの端末間同期、インストール・アンインストールの実機操作は未実施。この作業では更新用の鍵を作成せず、GitHub ActionsのSecrets / Variableが未登録であることを確認した。リモートのワークフロー実行、タグのpush、Releaseの公開も行っていない。手順と残作業は [windows-release.md](windows-release.md) を参照。

## 自動検証

Vitest / React Testing Libraryでローカル作成、検索、ソート、タイトル、色、ピン、500ms自動保存、閉じる前保存、削除マーカー、保存失敗時の入力維持、危険HTML/URLを検証する。

同期テストは初回統合、アップロード、ダウンロード、同一内容、競合コピー、リモート削除、削除と編集の競合、オフラインと復帰、同期中の追加入力、通信断、不正Front Matter、一覧失敗、不完全一覧、接続先変更、409再判定、多重実行防止、認証・権限・不存在・制限エラーを扱う。

RustテストはSQLiteの実トランザクションに対するCAS、競合反映の原子性、アップロード確認時の新しいrevisionの保護、HTTP 401/403/404/409/422/429の分類、レート制限の時刻、管理対象のパス制限を確認する。

## 手動確認手順

1. Windowsで起動し、新規メモに日本語・改行・Markdownを書いて閉じ、再起動して残ることを確認する。
2. 検索、ピン、色、テーマ、Ctrl+S、プレビュー、削除の確認操作を試す。
3. 登録したGitHub Appを検証用リポジトリにインストールし、Device Flowで接続してMarkdownファイルを確認する。
4. AndroidとWindowsの両方で同じリポジトリを接続する。片方をオフラインで編集し、他方で異なる本文にして同期する。両方の本文が元メモと競合コピーに残ることを確認する。
5. 同期中の追加入力、401・権限不足・通信断後のローカル編集、Androidのキーボード、戻る、復帰、縦横切替を確認する。

## 実行結果（2026-11-04）

- `pnpm.cmd typecheck`、`pnpm.cmd lint`：成功。
- `pnpm.cmd test`：4ファイル、25テスト成功。設定編集中に古い接続先への自動同期が走らないことも検証。
- 実SQLiteでのCAS、原子的な競合処理、同期確認時の追加入力保護、再オープンによる日本語Markdown永続化を確認。
- Strongholdでテスト用トークンを保存・再読込し、誤った鍵の拒否と暗号化ファイル中に平文がないことを確認。暗号化のwork factorは本番の既定値を維持し、scrypt / salsa20 / pbkdf2だけ開発ビルドでも最適化する。最適化後のデバッグ暗号化テストも成功（実行8.62秒）。
- `cargo test --release --manifest-path src-tauri/Cargo.toml --lib`：8テストすべて成功（実行6.74秒）。
- Windowsの最終修正を含むNSISビルド成功：`src-tauri/target/release/bundle/nsis/StickyNote_0.1.0_x64-setup.exe`（4.39 MiB）。
- Androidの最終ARM64デバッグAPK生成に成功：`src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk`（30,058,427 bytes / 28.67 MiB）。ネイティブシンボルはtargetディレクトリに残し、APKから除去した。アプリID `com.gitsticky.stickynote`、minSdk 24、targetSdk 37、arm64-v8aをaapt2で確認。デバッグ署名の検証にも成功。
- 実GitHubへの接続・書き込み・Windows/Android間同期はPAT未設定のため未検証。
- Windowsのロック解除後、リリース版を実画面で確認：新規作成、日本語タイトル・複数行Markdown入力、500ms自動保存、CodeMirror表示、GFMプレビュー、青への色変更、ピン留め、本文検索、Ctrl+S、通常終了と再起動後の保持を確認した。確認用メモ「動作確認メモ（日本語）」1件をローカルに残している。
- 実画面で検出したチェックリストの配置崩れをCSSで修正し、再生成したWindows版で表示を確認した。
- 通常終了できない不具合を修正。onCloseRequestedは保存完了までawaitし、保存失敗時だけ終了を防止する。Tauriが成功後に呼ぶdestroyに必要な権限をメインウィンドウに付与した。修正後にウィンドウが実際に消え、再起動してメモが残ることを確認した。
- 修正後もLint・Vitest 25件・型チェックを含むWindows NSISビルドとAndroid ARM64 APKビルドが成功。APK署名も再検証した。
- Androidの実機・エミュレーターでの操作確認、実GitHub同期は引き続き未検証。

環境：Windowsネイティブ、Node 24.11.1、pnpm 10.30.3、Rust 1.99.1 MSVC、Tauri 2.12.5。AndroidはNDK 28.2.13676358、SDK Platform 37、JDK 25、Gradle 9.6.1。

既知のビルド警告：libsodiumのMSVC配布物にPDBがなくLNK4099が出るがリンクは成功する。ViteはMarkdownエディタのチャンクサイズ警告を出す。Tauri生成GradleテンプレートにAGP 10で廃止予定のAPIに関する警告がある。いずれも今回のビルドを失敗させる警告ではない。

## 認証変更の検証（2026-11-05）

- 型チェック・ESLint成功。Vitestは31件成功。設定画面のDevice Flow、待機、キャンセル、期限切れ、App未設定の案内、切断時のメモ維持を追加した。
- Rustは32件成功。起動時の復元、Device Flowの間隔とslow_down、期限切れ、キャンセル後の遅延応答、更新の直列化、同時401、更新失敗、保存失敗時の再試行、切断失敗、旧保管庫移行を確認した。Windows DPAPIの実暗号化・復号・改変拒否も確認した。
- Android ARM64デバッグAPKのビルドとv2署名検証が成功（53,114,484 bytes）。Keystore連携Kotlinとバックアップ除外リソースを含み、minSdk 24・targetSdk 37・arm64-v8aを確認した。
- Windows NSISビルド成功（4.46 MiB）。ロック解除後、新リリース版の実画面で設定ダイアログを確認。通常のPAT・パスフレーズ欄がなく、App未設定の案内と無効化された接続ボタンが表示される。接続状態の再確認もエラーなく動作した。既存の日本語メモの本文・青色・ピン留めを保持し、再度起動してもウィンドウが増えず既存画面が前面になることを確認した。
- GitHub Appは未登録のため、実際の承認・トークン更新・リポジトリ取得・端末間同期は未検証。Android Keystoreの実機動作も未検証。登録手順は[github-auth.md](github-auth.md)を参照。

### 接続ボタンが無効になる場合の案内

ブランチ取得中・空の一覧・取得エラー・未選択を区別して表示し、設定画面からブランチを再取得できるようにした。空のリポジトリでREADMEなどの最初のコミットが必要なことを案内する。空の一覧から再取得して接続可能になること、権限エラーを空の一覧として扱わないことを追加検証し、Vitest全33件・型チェック・Lintが成功した。

その後のユーザー確認ではGitHub Appを登録し、privateリポジトリの一覧表示・選択まで進んでいる。接続ボタンが無効だった対象は空のリポジトリとの回答があり、初回コミットを作成する手順を案内した。実際の書き込み・端末間同期は引き続き未検証。

### Markdownの同時編集・プレビュー

Inkdropを参考に、標準表示を編集とプレビューの分割に変更。デスクトップは左右、狭い画面は上下に配置し、編集のみ・プレビューのみも選択できる。保存タイマーが発火する前に見出し・太字・タスクリスト・表がプレビューに反映されること、表示切替時にエディタと未保存本文を保持し、その後も自動保存できることを追加検証した。Vitest全35件・型チェック・Lintが成功。

Windows NSISとAndroid ARM64デバッグAPKを再ビルドし、APK署名の検証にも成功。Windowsの実画面で既存の日本語Markdownの左右同時表示、見出し・タスクリスト・太字の描画、文字数、プレビュー単独表示と分割への復帰を確認した。本文は変更していない。狭い画面の上下配置とAndroid実機操作は目視未検証。

### エディタの表示領域拡大後の確認（2026-11-07）

Vitest全35件、Rustのreleaseユニットテスト全32件、型チェック、Lint、変更ファイルのPrettierチェックが成功。Windowsアプリを再ビルドし、既存の日本語メモで編集のみ・分割・プレビューのみの切り替えを実画面で確認した。

入力欄の背景が本文末尾で途切れる問題を確認し、CodeMirrorとその親要素に高さを指定して修正。再ビルド後に、入力欄が本文領域の下端まで広がることを確認した。メモ本文・色・ピンは変更していない。GitHub Appの公開設定と説明文の不一致も修正した。今回の確認ではAndroid APKの再ビルド・Android実機操作・狭い画面の上下配置は検証していない。

### メモのタグ（2026-11-07）

Vitest全48件、Rustのreleaseユニットテスト全34件、型チェック、Lintが成功。日本語タグの追加・候補からの追加・重複防止・解除・500ms自動保存、AND絞り込み、タグなし、検索との併用、使用されなくなった選択タグの解除を検証した。同期ではv1の読み込み、v2のタグ検証、タグだけの追加・解除の別端末への反映、タグだけの競合時のコピー保護を確認した。SQLiteの既存メモの移行、再接続後のタグ永続化、不正タグの書き込み時のロールバックも検証した。

Windows NSISとAndroid ARM64デバッグAPKを最終版で生成し、APKのv2署名検証にも成功。Androidは既存の補助スクリプトでシンボリックリンク権限不足を検出し、同じRustライブラリをコピーしてGradleでパッケージ化した。Windows実画面で既存メモがタグなしで開くこと、本文領域の高さを維持するタグボタンとパネルの表示を確認した。Escapeでエディタまで閉じる問題を修正し、パネルだけ閉じてタグボタンへフォーカスが戻ることを再ビルド後に確認。実データへのタグ追加・本文変更は行っていない。Android実機・狭い画面での操作と実GitHubリポジトリを使ったタグ付きメモの端末間同期は未検証。

## セキュリティ対応の検証（2026-11-11）

- フロントエンドの型チェック・ESLint・本番ビルドが成功。Vitestは59件成功。
- Rustユニットテストは40件成功。暗号文の改ざん・誤鍵・別メモ/ブランチへの入れ替え、同期鍵の復元・保存失敗、移行後の平文拒否、ローカルDBの平文からの移行・再オープン・鍵の欠落・誤鍵・保存失敗後の再試行を確認。移行したDBファイルにタイトル・本文・タグの平文がないこと、JSONエスケープで大きくなる本文も保持できることを検証。
- 公開/公開範囲不明のリポジトリの拒否、UIでの公開先の除外、同期鍵なしで接続できないこと、鍵の登録失敗時の入力維持と成功後の消去を確認。
- 同期では既存の平文を1回の同期で暗号化する経路、変更のないメモの移行で余計な競合コピーを作らないこと、復号失敗時にアップロード・削除・ローカル更新を始めないことを検証。
- `cargo clippy --lib --locked -- -D warnings` が成功。
- Android ARM64向けRustの `cargo check --target aarch64-linux-android --lib --locked` が成功。Kotlinの `:app:compileArm64DebugKotlin` とリリース用の `:app:processArm64ReleaseMainManifest` も成功。クリーンなworktreeに必要なgitignoredのTauri生成ソース・Gradle接続ファイルを用意して検証した。マージ済みリリースManifestの `allowBackup=false` と `usesCleartextTraffic=false` を確認。
- `pnpm audit` は開発用を含め警告0件。Cargo.lockの636レジストリパッケージをOSVと照合し、従来のSQLite・ZIP・RSAの警告が解消したことを確認。残る8パッケージの通知と適用範囲は [security.md](security.md) に記載。
- 同梱したSQLx SQLiteドライバーのソースとライセンスは、crates.ioが公開するSHA-256と照合した0.9.0のアーカイブとバイト単位で一致。変更はlibsqlite3-sysのバージョン制約のみ。上流のCargo.lockは同梱せず、アプリのCargo.lockを使用する。

今回の検証では実ユーザーのDB・GitHubのメモを変更していない。実GitHubでの暗号鍵登録・端末間同期・Android実機のKeystore動作は未検証。APK/NSISの配布・インストールは実施していない。残存するGit履歴や過去のバックアップの平文はこの変更では消去しない。

### 追加の同期保護と鍵交換（2026-11-11）

- Vitestは62件成功（`pnpm.cmd exec vitest run --maxWorkers=1`）。鍵を外部へ控えるまで交換を開始できないこと、中断した交換を保存済みの鍵で再開すること、復旧用コピーの一覧・復元を追加検証。
- Rustは50件成功（`cargo test --manifest-path src-tauri/Cargo.toml --lib --locked`）。ローカルHTTPのGitHub API模擬サーバーを通して、本番と同じGit tree・commit・ref更新処理を検証。無断の暗号文差し替え・削除、管理ファイルを含む確認済み世代への巻き戻し、正規の削除同期、新旧鍵の切り替え、別端末の新鍵登録を確認。
- 鍵の保存に失敗するとアップロードを始めないこと、ref更新前の失敗ではブランチが変わらないこと、ref更新後の応答喪失・再起動後も交換を再開できること、同時更新を上書きせず管理対象外ファイルを保持することを検証。
- 別の鍵を再登録しても平文移行の完了状態と更新番号が維持されること、交換中は通常同期・鍵の再登録を停止することを検証。復旧用コピーの暗号化・別UUIDでの復元と、適用失敗時の同一トランザクションでのロールバックを確認。
- ビルドで生成したACLをTauriの実際のresolver・RuntimeAuthorityへ渡し、mainのローカルIPCを許可し、別ウィンドウ/WebView・外部URLを拒否することを検証。ウィンドウ新規作成のcore権限も許可していないことを確認。
- 型チェック・ESLint・本番フロントエンドビルド・変更対象のPrettierチェックが成功。`cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests --locked -- -D warnings` が成功。Windowsテストのリンクでは従来のlibsodiumのPDB欠落警告が出るが、50件の実行は成功。

検証に使用したのはテスト用認証・鍵・メモのみ。実GitHubでの新しいGit Database API経路、Android実機、インストーラーの生成・配布は未実施。初回接続以前や未観測の世代の巻き戻し、過去のGit履歴の消去は保証しない。保護範囲と手順は [security.md](security.md) を参照。
