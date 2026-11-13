---
id: "EXP-d1ZLrVSwYuDgjZk3dhMBWn"
type: "experiment"
status: "completed"
title: "Project Model — 文書チェーン生成と契約照合"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/10"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/project-model.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-model-design.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/project-model-task.md"
    - "docs/test-specs/project-model.md"
  defers: []
  deferred-by: []
---

# Project Model — 文書チェーン生成と契約照合

## 問題設定

Issue #10 の Project Model 設計契約を canonical 文書鎖として確立する
必要があった。既存 `synvelis-project` には実装と test が存在するが、
Issue #10 が要求する責務分界・非破壊編集・時間 domain・stable ID・
単一 writer・save/reload・失敗契約を一つの正本として固定する記述が
なかった。本 slice は文書のみで crate を変更しないため、(a) 7 文書
チェーンの生成、(b) PM-AC→PM-T→既存 test の写像、(c) doc graph/index/
review の整合確認が試行対象となった。

## 試行と結果

### E1: 上流契約と実装の照合

`crates/synvelis-project/src/` の `project.rs`/`id.rs`/`ops.rs`/
`controller.rs`/`store.rs`/`migration.rs`/`time.rs`/`tempo.rs`/
`placement.rs`/`comp.rs`/`midi.rs`/`routing.rs`/`recording_data.rs`/
`range.rs`/`snapshot/`/`restore/`/`diff/`/`vcs/`/`apply/` を読み、
`tests/` 39 fixture file の test 名を棚卸しした。`store.rs` は token 閾値
超過のため `cheaper-bulk-read` 相当の分割 read で SnapshotEnvelope/
DirectoryBundleStore/HEAD CAS/`save_cas`/`snapshot_collision`/publish
honesty を確認した。設計書は既存実装を正本として参照し、概念/スキーマ
契約のみを固定する方針とした。

結果: `SCHEMA_VERSION=13`・`ProjectController`・`ProjectStore`（HEAD CAS
付き `DirectoryBundleStore`/`InMemoryStore`）・ordered migration
ladder（v5→v13 read-only projection）・VCS snapshot/diff/apply/restore・
`MidiSource`/`TakeGroup`/`Comp`/`CaptureSegment` を確認。PM-T oracle の
既存 fixture が `synvelis-project` test suite で実測 pass（460 test・
exit 0）であることを `mise run test synvelis-project` で確認した。

### E2: doc-driven-dev-graph router の blocked 確認

`node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus
project-model --json` を実行した。結果は `current: "probe"`・`edgeId:
null`・`status: "blocked"`。blockers は `artifact-graph` + 多数の既存
`broken-relation`（rcd/tim/vcs/automation/pc/sf 等の無関係文書の
deferred relation が docs パスとして解決不能）。**本 task の新規文書
起因の finding ではなく repo 全体の baseline 問題**であり、編集範囲外の
ため修復しない。workspace-model slice（sv2）と同じ対処で、doc-status
lint/audit で個別に整合を確認する方針とした。結果は
`evidence/sv10/project-model-verification.md` と case-ledger §4 に記録。

### E3: 文書チェーン生成と index/audit

`new_spec.js`/`new_design.js`/`new_plan.js`/`new_task.js`/
`new_test_spec.js`/`new_impl_record.js` で骨格を生成し、ID（`SPEC-`/
`DESIGN-`/`PLAN-`/`TASK-`/`TSPEC-`/`IMPL-` prefix + uuid62）を正規に
採番した。`new_experiment_log.js` は `.jsonl` を生成するため、repo 慣行
（workspace-model 等）の `.md` experiment record を採用し `EXP-` prefix
の uuid62 形式 ID を付与した。IR は生成名
`project-model-issue-10-headless.md` を要求ファイル名
`project-model.md` に rename した。

`doc_maintenance.js index apply` で触れた dir（specs/designs/plans/
tasks/test-specs/impl/ir）の index を再生成し（`docs/impl/exp` は
`.jsonl` のみ index 対象の既存規約のため `.md` experiment record は
index されず差分なし）、
`doc_status.js lint --type {spec,design,plan,task,test-spec,impl}` と
`audit_impl_record.js --json` で新規文書起因の blocking finding がない
ことを確認した（index 未生成の `index-missing-entry`/`orphan-*` は
index apply で解消）。

### E4: review-protocol（spec+design+chain 全体）

spec→design→plan→task→test-spec→ir→exp + evidence/sv10/ の全文書
チェーンを review-protocol でレビューした（分離 pass → validate →
judge、1 サイクル）。実装との照合で CONFIRMED 3 件を修正した:
(a) `ClipPlayback`/`PlaybackMapping`/`Clip` の field 名を実装に合わせて
訂正し、clip-level gain/fade field が現行 schema に存在しないことを
明記（gain は strip/send `ParameterTarget`、fade は `CompTransition`）、
(b) `Command<ProjectOp>` が `ProjectId` を持たず `expected_revision`+`op`
のみを持つ実装事実に合わせ spec/design を訂正、(c) `SplitClip` に明示
`DerivedFrom` field がない gap を design/ledger/test-spec に記録し
schema 拡張として deferred とした。P0/P1 blocker は残っていない
（verdict: ACCEPT WITH NON-BLOCKING NOTES 相当）。

## 結論

Issue #10 の Project Model 設計契約を 7 文書チェーン + `evidence/sv10/`
台帳として確定した。PM-AC-01〜13 は PM-T-01〜22 の headless case・
設計レビュー・NotRun/Deferred（PM-T-23〜30）に漏れなく写像され、
既存 `synvelis-project` fixture（460 test pass・exit 0）で oracle を
実測確認した。`doc-driven-dev-graph` router の blocked は pre-existing
baseline であり本 slice 起因ではない。実機・実 plugin・実 RT・実 storage・
Creative Branch・remote provider の受入は #112/#114/#176/#294/#84/#330/
#277/#278/ALIB 系の別 Gate に委譲する。
