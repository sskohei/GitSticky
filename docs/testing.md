# 検証記録

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
