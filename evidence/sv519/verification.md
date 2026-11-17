# sv519 route_graph 修復 — 検証記録 (synvelis#519)

実施環境: Windows、`mise exec -- node`（node v26.12.1、mise.toml pin）。
route_graph 系は各 1 回あたり約 100 秒。全て実測値のみ記録し、未実行の項目は
NotRun とする。

## V-1 route_graph --json（修復前）

- コマンド: `mise exec -- node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --json`
- 結果: `status: blocked`
- blockers 236 件: `artifact-graph` 1、`broken-relation:*` 234、`focus-required` 1
- 証跡: `route_graph_before.json`

## V-2 route_graph --json（修復後・origin/main merge 後の最終状態）

- コマンド: 同上（`git merge origin/main` 取込後に実施）
- 結果: `status: blocked`（残 blocker は下記の意図的 terminal/例外のみ）
- blockers 3 件:
  1. `artifact-graph` — `graphIssues > 0` のときだけ立つ umbrella。
     唯一残る broken-relation（cb-09、下記 V-5 の例外）に連動して残存
  2. `broken-relation:Broken related relation from docs/impl/ir/cb-09-diff-explorer.md to evidence/sv286/` — lease 外例外（V-5）
  3. `focus-required` — `--focus` 未指定時の設計上の期待 hard stop（V-4 で消滅を実測）
- `broken-relation:*` は上記 cb-09 の 1 件のみ。修復対象 233 件は全て解消。
  merge で取り込んだ upstream 文書（issue-539 chain・cb-11 更新）による
  新規 broken-relation は 0 件。
- 証跡: `route_graph_final.json`

## V-3 route_graph --focus（focus-required 消滅の実測）

- コマンド: `mise exec -- node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus docs/specs/issue-518-job-system.md --json`
- 結果: `focus-required` は消滅。`status: blocked` のままだが残 blocker は
  `artifact-graph`・cb-09 の broken-relation・`task-graph-invalid`。
- `task-graph-invalid` の内訳: `plan-has-no-tasks` —
  `docs/plans/issue-518-job-system.md` に task 文書が紐付かない。
  task 文書 `docs/tasks/issue-518-job-system.md` は `implements: []` で
  `derives-from` のみを宣言しており、graphTask は `implements` のみで
  task を選択するため 0 node となる。**本 slice の変更とは無関係の
  upstream 既存状態**（当該 plan/task 文書はいずれも未変更・merge 対象外）。
  lease 外のため修復せず記録に留める。
- 証跡: `route_graph_focus518.json`

## V-4 blocker の扱い確定（AC#4）

- `focus-required`: `--focus` 未指定時の期待 terminal。`--focus` 指定で消滅を
  V-3 で実測。tool 変更不要。
- `artifact-graph`: broken-relation finding の umbrella。修復可能な finding は
  全件解消し、残存は cb-09 の例外 1 件のみで、それに連動して残るのは想定内。
- cb-09 の 1 finding: `synvelis#286` 占有中の lease 外ファイル。
  `synvelis#286` merge 後の残件として文書化（本ファイル V-5・PR 本文）。

## V-5 doc_status.js lint 前後対比（AC#5）

同一ツリー（私の diff のみ差分）での before/after。before は `git stash` で
変更を退避して実測、after は適用状態で実測。

| dir | before findings | after findings | before blocking | after blocking |
| --- | --- | --- | --- | --- |
| docs/specs | 223 | 139 | 156 | 106 |
| docs/designs | 275 | 248 | 185 | 159 |
| docs/adr | 35 | 31 | 22 | 18 |

- 全 dir で finding 数は**減少**（悪化なし）。prose `defers` 等が
  doc_status の `broken-relation-link` 系 finding の原因でもあったため、
  route_graph 修復と同根の改善として減少した。
- 証跡: `docstatus_{specs,designs,adr}_{before,after}.txt`

### merge 後の最終状態（参考）

`git merge origin/main`（issue-539 chain・cb-11 更新を取込）後の再実測:

| dir | documents | findings | blocking |
| --- | --- | --- | --- |
| docs/specs | 226 | 141 | 107 |
| docs/designs | 234 | 249 | 160 |
| docs/adr | 65 | 35 | 19 |

- 増分は全て upstream 新規 `issue-539-arrangement-adapter` 文書と
  `docs/designs/README.md` index warning 由来。**私の変更ファイルに
  起因する新規 finding は 0 件**（after→final の差分行を全件照合済み）。
- 証跡: `docstatus_{specs,designs,adr}_final.txt`

## V-6 fmt-check（AC#6）

- コマンド: `mise run fmt-check`（= `cargo fmt --all -- --check`）
- 結果: exit 0 / Passed（Rust 変更なし・merge 取込分も upstream で整形済み）

## V-7 prek（pre-commit 全フック）

- コマンド: `mise exec -- prek run --all-files`
- 結果: exit 0。全 hook Passed
  （trailing whitespace / end of files / check yaml / check toml /
  mixed line ending / utf-8 BOM / private key / actionlint）

## V-8 差分健全性

- `git diff --check`: whitespace error なし（prek の trailing whitespace
  hook も Passed で裏付け）
- 変更は 102 docs ファイルの frontmatter relation 値と `evidence/sv519/` のみ。
  `docs/**/README.md`（index）・`crates/`・`tools/`・設定ファイルの変更なし。
- `defers:` 空 key の残存なし（`syn-io-02-head-cas-e2e.md` は `defers: []` に
  修正済み — 空 key は `invalid-relation` finding となることを route_graph
  中間実行で実測し修正した）

## NotRun

- cargo check/lint/test/build・`*-full`・Feature Matrix・`*-msrv`・3OS 横断:
  `crates/**` 変更なしのため `POLICIES.md` §5 により必須検証に含めない。
- docs index 再生成（`index apply`）: Worker の禁止事項。Coordinator が
  統合時に `doc-maintenance` Skill で再生成する。
