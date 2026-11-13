# 開発への参加

不具合報告、ドキュメントの改善、テスト、機能の変更を歓迎します。環境構築は [開発ガイド](docs/development.md)、仕様を調べるときは [ドキュメント一覧](docs/README.md) から始めてください。

## Issueで相談する

既存の [Issue](https://github.com/sskohei/GitSticky/issues) と [PR](https://github.com/sskohei/GitSticky/pulls) を検索し、同じ内容があれば情報を追加してください。大きな機能追加や同期・保存形式の変更は、実装前に目的と変更案をIssueで相談すると認識を合わせやすくなります。誤字や小さな修正は直接PRにできます。

不具合報告には以下を記載してください。

- 期待した動作と実際の動作
- 最小限の再現手順、再現頻度
- Windows / Androidの版、アプリの版またはcommit
- 開発中の問題ならNode.js・pnpm・Rustなどの版と実行コマンド
- 秘密情報を除いたエラーや、必要に応じた画面画像

メモの実データ、DB、認証トークン、同期鍵、秘密鍵は公開しないでください。再現にはダミーのメモと検証専用リポジトリを使います。

## ブランチを作る

外部から参加する場合はGitHubでforkしてからcloneします。次のURLの `YOUR_ACCOUNT` を自分のアカウント名へ置き換えてください。

```powershell
git clone https://github.com/YOUR_ACCOUNT/GitSticky.git
cd GitSticky
git remote add upstream https://github.com/sskohei/GitSticky.git
git fetch upstream
git switch -c docs/improve-setup upstream/main
pnpm.cmd install --frozen-lockfile
```

直接pushできる場合は本リポジトリをcloneし、`git fetch origin` の後、`origin/main` から作業ブランチを作ります。ブランチ名は目的が分かる名前にします（例: `fix/editor-save`、`feat/note-filter`）。

## 変更と確認

- 1つのPRで1つの目的を扱い、関係する仕様説明も更新してください。
- TypeScriptはstrictを維持し、既存の型・UI・状態管理の構成に合わせます。Prettier / ESLintの設定はリポジトリに同梱しています。
- UIからGitHubへ直接通信せず、DB更新と秘密情報の扱いはRustへ集約します。
- 新しいIPCはRustのハンドラ・`build.rs`・permissions・capabilitiesを確認し、必要な範囲で許可します。
- DB移行、同期形式、競合・削除処理を変える場合は、既存データの互換性と失敗時の保持を確認します。[セキュリティ仕様](docs/security.md) も参照してください。
- 依存関係を変える場合は対応する `pnpm-lock.yaml` / `src-tauri/Cargo.lock` も更新します。`src-tauri/vendor/sqlx-sqlite` はSQLiteのセキュリティ修正用パッチなので、更新時は [既存の説明](docs/security.md) を確認してください。

変更に応じて以下を実行し、結果をPRへ記載します。コマンドの詳細は [開発ガイド](docs/development.md) にあります。

| 変更               | 確認の目安                                                       |
| ------------------ | ---------------------------------------------------------------- |
| 文書のみ           | 変更した文書のPrettier、リンク先と手順の整合性                   |
| TypeScript / React | 型チェック、Lint、Vitest、フロントエンドビルド                   |
| Rust               | Rustユニットテスト、Clippy、関連するフロントエンド検証           |
| UI                 | 関連テストとTauriでの操作確認、画面画像                          |
| 保存・同期・認証   | 正常時と失敗時のテスト、必要に応じて検証専用リポジトリで手動確認 |
| Android固有処理    | APKビルド、対象端末・エミュレーターでの操作確認                  |

不具合修正には再発を防ぐテストを、機能変更には期待する動作を確かめるテストを追加してください。文書や表示文言だけの変更では、目的に合う確認を選びます。実行できなかった項目は理由を明記し、自動テストの成功と実端末での確認を区別してください。

## PRを提出する

```powershell
git diff --check
git add CONTRIBUTING.md
git commit -m "docs: explain contribution workflow"
git push -u origin docs/improve-setup
```

上記のファイル名・ブランチ名・commitメッセージは例です。自分の変更に置き換え、追加前に `git diff` と `git status` で対象を確認してください。生成物、ローカルパス、署名ファイル、秘密情報を含めないでください。

GitHubで `sskohei/GitSticky` の `main` を提出先にPRを開きます。テンプレートに沿って問題・変更後の動作・検証結果と未確認事項を記載し、関連Issueがあればリンクしてください。UI変更は画面画像も添えてください。レビューで追加変更を求められたら、同じブランチへcommitしてpushします。

コードは [MIT License](LICENSE) で公開しています。
