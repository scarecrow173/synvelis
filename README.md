# Project Template

言語・フレームワーク非依存の、mise + APM + prek + Git LFS による最小開発基盤です。
このリポジトリにはアプリケーションコード、言語別設定、CI、文書ディレクトリを含めません。

## 新規プロジェクトでの使い方

1. GitHub の **Use this template** から新規リポジトリを作成する。
2. `README.md` と `apm.yml` の名前・説明をプロジェクトに合わせて変更する。
3. mise をインストールし、リポジトリで次を実行する。

```sh
mise install
mise run setup
```

`mise.toml` では APM CLI を `github:microsoft/apm` で取得します。GitHub 配布バイナリの URL と SHA-256 は `mise.lock` 生成時に記録され、APM CLI 自身の pipx 依存グラフ sidecar は不要です。

`mise run setup` は APM 依存関係を復元し、`prek install` でコミットフックを、`git lfs install --local` でこのリポジトリの LFS フィルターと Git フックを有効化します（グローバルGit設定は変更しません）。
全ファイルのフックを手動実行する場合は `mise exec -- prek run --all-files` を使用してください。
Git LFS が必要な Git 操作では、mise を有効化したシェルを使うか、`mise exec -- git push` のように実行してください。初回clone時にLFSオブジェクトが未取得なら、セットアップ後に `mise exec -- git lfs pull` を実行してください。

4. 生成された `mise.lock` と `apm.lock.yaml` を確認してコミットする。
5. 使用言語・フレームワークが確定してから `mise.toml` にツール・検証タスクを追加する。

> 初回作成時はツールによる解決がまだ行われていないため、lockfile は含めていません。
> `mise.lock` の生成・コミット後も通常の `mise install` を使用します。`mise.toml` と整合する lockfile があれば、そこに固定されたツールバージョンが使われます。

mise lockfile 形式 v2 以降は、npm・PyPI系ツールの依存グラフが `.mise/locks/` に保存される場合があります。`mise lock --sidecars` で確認し、`mise.lock` が参照するファイルを一緒にコミットしてください。`.mise/` の他のローカル状態はGit管理対象外です。

新しいworktreeでは、`.worktreeinclude` に列挙した既存のGit無視対象ファイル（ローカル環境設定とAPM展開物）を引き継げます。`mise.lock` と、コミット済みの `.mise/locks/` はGitから各ワークツリーに展開されるためコピー対象には含めません。

## 管理対象

| ファイル | 役割 |
| --- | --- |
| `.gitattributes` | OS間の改行・テキスト属性、Git LFS管理対象 |
| `.gitignore` | 秘密情報、ローカルファイル、APM生成物の除外 |
| `.worktreeinclude` | 新規worktreeへ引き継ぐローカル設定・APM生成物の一覧 |
| `AGENTS.md` | エージェント共通作業規約 |
| `apm.yml` | 共通のAPMパッケージとMCP設定 |
| `.pre-commit-config.yaml` | prekによる共通のGitフック |
| `mise.toml` | APM・prek・Git LFSなどの共通ツールと `setup` タスク |
| `apm.lock.yaml` / `mise.lock` | 初期化後に生成・追跡するlockfile |

言語に依存するソース配置、Lint、テスト、ビルドは各プロジェクトが決定します。
