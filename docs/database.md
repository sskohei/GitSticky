# ローカルデータ

`sqlite:stickynote.db` をTauri SQL Pluginで開く。スキーマは `src-tauri/migrations/001_initial.sql`。プラグインのマイグレーション管理テーブルに適用バージョンを記録する。

| テーブル | 主キー | 内容 |
| --- | --- | --- |
| notes | id | title、body_md、color、pinned、created_at、updated_at、deleted_at、local_revision、dirty |
| sync_states | note_id + connection_id | last_synced_sha、last_synced_revision、last_synced_at、sync_error |
| app_settings | id=1 | 設定のJSON。PAT・パスフレーズは含めない |

タイトルは空白だけを禁止し、最大80文字。UIで「無題のメモ」に正規化し、RustとDBでも検証する。colorは6色のCHECK制約。時刻はISO 8601文字列、IDはUUID。本文はUTF-8文字列として保持する。

ローカル編集はDBに存在するrevisionから必ず+1し、dirty=trueにする。削除はdeleted_atを立てて同様にrevisionを進める。未同期削除を即座に完全削除しない。

アップロード確認時は送信したrevisionのみを同期済みとして記録し、現在のrevisionが一致した場合だけdirtyを解除する。削除の確認も同じ条件でクリーンアップする。リモート反映はrevisionのCASと同一トランザクションで行い、競合コピーの作成と元メモ更新が部分的に保存されることを防ぐ。

connection_idは `[owner.toLowerCase(), repository.toLowerCase(), branch]` のJSON。接続先ごとに同期基準を分離する。接続先変更時には全メモをdirtyにし、以前のSHAは新しい接続先に使わない。

検索はSQLiteから取得済みのローカルスナップショットに対して実行し、タイトルと本文を大文字・小文字を区別せず照合する。ネットワークアクセスは不要。MVPではFTSを使わず、全メモをローカルメモリにも保持する。
