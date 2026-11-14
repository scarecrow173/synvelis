# VCS Integration — Verification Record (synvelis#7)

Issue #7 の docs/ 設計契約 + evidence 台帳 slice の検証記録。
**本 slice は docs/ と evidence/ のみ**であり、実装 crate・
GUI・実 device/plugin/OS/Provider/RT/consumer 受入は変更・
実行していない。実行した検証とその結果のみを記録し、推測と
実測を区別する。

## 環境

- OS: Windows（Git Bash 経由）
- 作業 worktree:
  `C:\Users\akiyoshi\orca\workspaces\synvelis\sv7-vcs-integration`
- ブランチ: `docs/7-vcs-integration`（`origin/main` @
  `86715f9` から作成）
- Rust toolchain: mise 管理（fmt-check は cargo fmt 経由）
- Node: doc-maintenance/doc-status/impl-doc scripts 用
- canonical 検証入口: `mise.toml` の `mise run` タスク
- 補助証拠: docs 検証 script（`.agents/skills/*/scripts/`）

## 実行した検証

### V-1 `mise run fmt-check`（正規検証コマンド）

- コマンド: `mise run fmt-check`
- OS: Windows（Git Bash）
- toolchain: mise 管理 Rust
- exit code: 0
- 結果: 成功（`cargo fmt --all -- --check` 通過）
- 備考: docs/markdown のみの変更。Markdown の整合は
  doc-status lint で補完。

### V-2 doc-maintenance index plan/apply（index 再生成）

- コマンド:
  `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index plan|apply --dir <dir> [--expect <planHash>] --json`
- 対象 dir: `docs/specs`・`docs/designs`・`docs/plans`・
  `docs/tasks`・`docs/test-specs`・`docs/impl/ir`・`docs/impl/exp`
- OS: Windows（Git Bash）
- toolchain: Node（script 同梱）
- exit code: 0（全 dir）
- 結果: `index plan` で得た planHash を `--expect` に渡して
  `index apply` を実行し、`docs/specs/README.md`・
  `docs/designs/README.md`・`docs/plans/README.md`・
  `docs/tasks/README.md`・`docs/test-specs/README.md`・
  `docs/impl/ir/README.md` を再生成。`docs/impl/exp` は
  plannedFiles 空（`.md` exp notes は managed index の対象外、
  `.jsonl` のみ管理 — `sync_impl_index` が IN-SYNC を確認）。
  index の手編集なし。

### V-3 sync_impl_index（impl index 整合）

- コマンド:
  `node .agents/skills/impl-doc/scripts/sync_impl_index.js
  --kind ir` / `--kind exp`
- OS: Windows（Git Bash）
- exit code: 0
- 結果: `IN-SYNC docs/impl/ir`（183 sources）・
  `IN-SYNC docs/impl/exp`（1 source = `.jsonl` のみ管理）。
  preview のみで apply 不要。

### V-4 doc-status lint（docs 検証）

- コマンド:
  `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type {spec,design,plan,task,test-spec} --json`
- OS: Windows（Git Bash）
- exit code: 0（lint は finding を列挙して exit 0）
- 結果: **本 slice の新規文書に残る finding は 0 件**。
  - spec: 当初 `references: evidence/sv7/`（directory 参照）が
    `broken-relation-link` → 2 ファイルパスへ修正済み。
  - plan: `verified-by` が task を指していた
    `invalid-relation-target-type` + `plan-missing-test-spec-evidence`
    → test-spec パスへ修正済み。
  - task/design/test-spec: finding なし。
  - `evidence/sv7/vcs-integration-verification.md` 未作成に伴う
    `broken-relation-link`（plan/task/ir の references/related）は
    本ファイル作成で解消。
- 備考: repo 全体では blocking finding が多数残存するが、全て
  既存文書の pre-existing 状態（legacy `ISSUE-*`/`SPEC-*` token・
  malformed relations 等）であり本 slice の変更範囲外。

### V-5 impl-doc audit

- コマンド:
  - `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json`
  - `node .agents/skills/impl-doc/scripts/audit_experiment_log.js --json`
- OS: Windows（Git Bash）
- exit code: 0
- 結果: `docs/impl/ir/vcs-integration.md` の finding は
  `vcs-integration-verification.md` 未作成に伴う warning 2 件のみ
  （本ファイル作成で解消）。error 50 件は全て既存ファイル
  （`issue-147-sccache.jsonl` 等）の pre-existing 状態。
  exp audit の error も `issue-147-sccache.jsonl` の既存
  `invalid-event-shape` であり本 slice 対象外。

### V-6 route_graph（doc graph route）

- コマンド:
  `node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js
  --focus docs/specs/vcs-integration.md --json`
- OS: Windows（Git Bash）
- exit code: 0
- 結果: `status: "blocked"`、`blockers: ["artifact-graph", ...]`。
  原因は repo 全体の pre-existing broken relations（legacy
  `ISSUE-*`/`SPEC-*` source token、`docs/designs/docs/designs/…`
  のような malformed related path 等）であり、本 slice 以前からの
  状態。focus なし実行時と同じ fail-closed 結果。
  `taskGraph` は本 slice の task
  （`TASK-034X9GCGPXdCl5ePe8gJ14`）を `active` として正しく解決。
  本 slice 起因の新規 broken relation は ir →
  `vcs-integration-verification.md` の未作成参照 1 件のみで、
  本ファイル作成で解消。

### V-7 review-protocol

- 実施: review-protocol skill によるレビュー（spec/design/
  plan/task/test-spec/impl/evidence の一貫性・evidence honesty・
  Owner 境界・relations 整合・Case ID 被覆の完全性）
- 結果: 実装した 2 pass レビュー（cooperative + adversarial
  相当）で検出した問題を修正:
  - case-ledger 集計表の Pass/Pass* 計数誤り（DIFF 10→11、
    GIT 10→11、RESTORE 8→7、COMPAT 0→1）を修正。
  - spec `references` の directory 参照を file 参照へ修正。
  - plan `verified-by` の target type 誤りを修正。
- blocker: なし（修正後の再確認で残存 blocker 0 件）。
- 備考: docs のみの slice のため実装 review ではなく契約
  review。subagent 不使用のため protocol の「clearly separated
  passes」として実施。

## Pass / Fail / NotRun / Unsupported 集計（本 slice）

| 区分 | 対象 | 結果 |
|---|---|---|
| Pass | docs 検証（fmt-check/index 再生成/sync_impl_index/lint/audit） | 全成功（新規文書の finding 0） |
| Pass | Case 被覆台帳の転記正確性（vcs-06 ledger との突合） | 全 Case ID + 拡張行を 1:1 転記、集計修正済み |
| NotRun | 実 GUI/IME/2Window/pixel/plugin dock（VCS-UI-05・UI-03 実 OS lane） | #114 後続 |
| NotRun | 実 remote/auth/GC・OS permission matrix・consumer precision（VCS-GIT-REAL residual・COMPAT-02/04・DIFF real view・APPLY/RESTORE-REAL） | #97 REAL lanes・#219/#294 |
| NotRun | 本 branch の docs 変更に対する CI run | merge 後の次回 dispatch 待ち（COMPAT-03 は既存 covering run 43428177865 の Pass を転記） |
| Unsupported | Git 以外の VCS provider・VCS Writer/3-way merge | ledger §out of scope を継承（Blocked/Unsupported として記録） |
| Blocked | doc graph route（`artifact-graph`） | pre-existing repo 状態。taskGraph は本 task を正しく解決 |

## 判定

本 slice の docs/ 設計契約 + evidence/sv7 台帳は **完了・
新規文書の検証は全て成功**。実装 crate は変更していない。
設計完了は実装完了・実機/GUI/Provider/RT/consumer 受入を
意味しない。後続 Owner（#92〜96 実装・#97 統合受入・#114
実 GUI・#219/#294 consumer・#324 plugin_instances diff）が残る。
