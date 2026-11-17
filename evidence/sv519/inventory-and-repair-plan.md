# sv519 route_graph blocked 修復 — 棚卸しと修復計画 (synvelis#519)

`route_graph.js --json` が返した `broken-relation:*` finding 234 件の棚卸しと、
frontmatter relation 正規化の修復計画・結果を記録する。機械可読な修復マニフェストは
`repair-manifest.json`、適用スクリプトは `apply.mjs` を参照。

## 1. baseline 実測

- コマンド: `mise exec -- node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --json`
- 結果: `status: blocked`、blockers 236 件
  - `artifact-graph`（umbrella）1 件
  - `broken-relation:*` 234 件（103 ファイル）
  - `focus-required` 1 件（`--focus` 未指定時の設計上の期待 hard stop）
- 生ログ: `route_graph_before.json` / `route_graph_before.stderr.log`
- finding 一覧の解析結果: `findings_parsed.json`

## 2. 原因別棚卸し（234 件）

| 原因クラス | 件数 | 内訳 |
| --- | --- | --- |
| 自由記述 prose target（`defers` 中心） | 約 130 | 「VCS-02 Semantic Diff ChangeSet（synvelis#93）」のような説明文。graph の relation target として解決不能 |
| `docs/<dir>/` 二重 prefix 相対 path | 29 | `docs/specs/docs/specs/...` 等。rename/移動で実在しない path |
| 存在しない `docs/` path（rename・phantom 名） | 30 | `sv261-headless-session-host`・`autx-03-headless-cli`・`rcd-03-monitor-gate` 等。現行文書名への張替えが必要 |
| `evidence/` directory target | 24 | `evidence/sv511/` 等。`route_graph.js` は `isFile()` を要求するため directory は無効 |
| `ISSUE-*`/`SPEC-*` 等 token | 35 | `ISSUE-96`・`SPEC-vcs06-monitoring-matrix` 等。GitHub Issue URL / 現行文書 path へ変換 |
| 外部参照（ruviform 等） | 2 | `crates/... @ sha` 形式の散文。外部 Issue URL / source 参照へ変換 |

relation 別: `defers` 143、`related` 37、`source` 37、`references` 9、`depends-on` 8。

## 3. 修復方針（直す / 例外 / ツール側）

- **frontmatter 修復で全件処理可能と判断**。`.agents/` は gitignore 済み APM 配布物で
  synvelis に commit できないため、tool/rule 側の語彙拡張（`evidence/` directory や
  `ISSUE-*` token の許容）は本 slice では行わない。恒久修正が必要になった場合の
  修正先は上流 `scarecrow173/apm-packages`（`scripts/doc-driven-dev/src/skills/`）であり、
  今回の修復では不要と確定した（全 finding を frontmatter 側で解決）。
- `defers` の有効値は GitHub Issue/PR URL または既存文書 path（cb-09 の `source` に
  GitHub URL を使う前例と同じ許容形式）。prose は対応する Issue URL・現行文書 path に
  置換し、権威ある向け先が存在しない 23 件は対象文書本文（Deferred/検証節）に意味が
  残ることを確認した上で relation 項目自体を除去した。
- `evidence/svNNN/` directory 参照は同 directory 内の実ファイル
  （`verification.md`・`acceptance-ledger.md` 等）へ置換。
- rename/phantom path は git log・文書タイトル・参照元本文・既存 relation 連鎖で
  現行文書を同定して張替え（例: `sv261-headless-session-host` →
  `docs/impl/ir/autx-02-cli-adapter.md`、`autx-03-headless-cli` →
  `docs/impl/ir/autx-03-headless-cli.md`、`rcd-03-monitor-gate` →
  `docs/test-specs/rcd-03-monitor-punch-latency.md`）。
- `ISSUE-*` token は `gh` で Issue 実在・件名を確認の上 GitHub Issue URL 化、
  `SPEC-*` token は現行 `docs/specs/` 文書 path へ張替え。
- 変換先文書の `blocks`/`deferred-by` が空であることを事前確認し、
  reciprocal relation の新規 warning を誘発しないことを確認済み。

## 4. 適用結果

- `repair-manifest.json`: 233 entry（findings 234 - skip 1）、102 ファイル
- 適用: `apply.mjs` により 210 entry を値置換（計 266 value）、23 entry を除去
- 変更は frontmatter の relation 値のみ。status・文書本文・他フィールドは不変
- `docs/impl/ir/syn-io-02-head-cas-e2e.md` は全 `defers` 項目除去に伴い
  `defers: []` に正規化（空 key は `invalid-relation` となるため）

## 5. 例外（修復しない finding）

- `docs/impl/ir/cb-09-diff-explorer.md` の `related` → `evidence/sv286/` 1 件。
  同ファイルは `synvelis#286` が占有中で本 slice の lease 外。`evidence/sv286/` は
  directory であり `isFile()` を満たさないため finding は残る。`synvelis#286`
  merge 後に同様の file 参照へ正規化するか、占有者側で対処する残件。

## 6. tool 側変更の要否

- 不要と確定。全 finding を frontmatter 正規化で解消できた。
- 参考: `route_graph.js` の `localTarget` は `isFile()` を要求するため
  `evidence/` directory 参照は構造上無効。`evidence/sv286/` のような directory
  記法を許容したい場合は上流 `scarecrow173/apm-packages` 側の検討事項だが、
  本 slice では frontmatter を file 参照に揃える方針で完結させた。
