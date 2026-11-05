# アーキテクチャ

Reactは表示・ローカル操作の要求・同期のオーケストレーションを担当し、RustはSQLiteトランザクション・GitHub通信・秘密情報の管理を担当する。UIからGitHubへ直接通信しない。

| 領域 | 責務 |
| --- | --- |
| components | Radix Dialog、shadcn形式のButton、共通UI |
| features/notes | 本文を表示しないグリッド、ローカル検索、ピン順の表示 |
| features/editor | CodeMirror、GFM、編集バッファ、保存キュー |
| features/settings | GitHub接続、リポジトリ選択、テーマ、自動同期 |
| features/sync | YAML変換、差分判定、GitHubインターフェース、自動同期 |
| stores | Zustandの永続データのスナップショットと検索語 |
| db | テストで差し替え可能なLocalDatabase、Tauriアダプタ |
| src-tauri/src/commands.rs | 型付きDBコマンドとトランザクション |
| src-tauri/src/github.rs | HTTPSの固定ホスト、API DTO、タイムアウト、エラー分類 |
| src-tauri/src/auth.rs / oauth.rs | Device Flow、期限管理、更新の直列化 |
| src-tauri/src/secure_store.rs | Windows DPAPI / Android Keystore、暗号化保存 |
| src-tauri/src/legacy.rs | 旧Stronghold保管庫の読み取り専用移行 |
| src-tauri/src/errors.rs | UIへ渡す機密情報を含まないエラー |

Tauri SQL Pluginが接続とマイグレーションを管理し、Rustは公開されたDbInstancesから同じsqlxプールを取得する。複数SQL文をJSから個別送信すると原子的な更新にならないため、競合解決・同期確認・設定変更はRustトランザクションにまとめた。

GitHub AppのDevice Flowを使用し、公開Client IDとslugだけをビルドに含める。Rustがトークンを取得し、Reactには確認コードと接続状態だけを返す。トークンをSQLite・ログ・localStorageへ保存しない。

Windowsではユーザー単位のDPAPI、AndroidではKeystoreのAES-GCMで暗号化する。更新は非同期Mutexで直列化し、同時の401でも同じ更新トークンを再使用しない。Windowsでは多重起動も防ぐ。更新後の保存失敗時は新しい資格情報をメモリに保持して保存を再試行する。切断は認証情報だけを削除し、メモを維持する。

旧Stronghold保管庫は一度のパスフレーズ入力で読み取り専用移行できる。復号はspawn_blockingで実行し、元の保管庫を変更しない。

CSPで外部スクリプト・外部通信を拒否する。RustはAPI用のapi.github.comと認証用のgithub.comの固定HTTPSエンドポイントだけに送信し、リダイレクトを追わない。フロントエンドには汎用HTTP、SQL実行、秘密情報の読取権限を与えない。

Windowsの終了要求は非同期ハンドラでエディタの保存完了を待ち、保存失敗時のみpreventDefaultする。成功後はTauri APIがウィンドウを破棄するため、mainウィンドウにcore:window:allow-destroyを付与する。[TauriのonCloseRequested / destroy仕様](https://v2.tauri.app/reference/javascript/api/namespacewindow/)に合わせ、終了要求を再帰的に発行しない。
