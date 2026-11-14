---
id: "IMPL-034X6jca6fIX3WfnkPyxGI"
type: "impl"
status: "completed"
title: "Project Model — Issue #10 設計契約文書化 + headless 検証"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/10"
  changes:
    added:
      - type: "file"
        path: "docs/specs/project-model.md"
      - type: "file"
        path: "docs/designs/project-model-design.md"
      - type: "file"
        path: "docs/plans/project-model.md"
      - type: "file"
        path: "docs/tasks/project-model-task.md"
      - type: "file"
        path: "docs/test-specs/project-model.md"
      - type: "file"
        path: "docs/impl/ir/project-model.md"
      - type: "file"
        path: "docs/impl/exp/project-model.md"
      - type: "file"
        path: "evidence/sv10/project-model-case-ledger.md"
      - type: "file"
        path: "evidence/sv10/project-model-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/project-model-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/project-model.md"
    - "docs/designs/project-model-design.md"
    - "docs/plans/project-model.md"
    - "docs/tasks/project-model-task.md"
    - "docs/test-specs/project-model.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/project-model.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/project-model.md"
    rejected: []
---

# Project Model — Issue #10 設計契約文書化 + headless 検証

## Summary

Issue #10 の Project Model を、既存 `synvelis-project` 実装と承認済み上流
契約に整合する 7 文書チェーン（spec/design/plan/task/test-spec/ir/exp）と
`evidence/sv10/` 台帳 2 件として確定した。PM-AC-01〜13 を PM-T-01〜22 の
headless case・設計レビュー・NotRun/Deferred（PM-T-23〜30）に写像し、
`mise run test synvelis-project`（460 test pass・exit 0）で既存 fixture の
oracle を実測確認した。`crates/` の実装は変更していない。

## Context

Issue #10 は Recording（#12）・Timeline tools（#11）・Snapshot/Save/
Recovery/stable ID（#17）・Execution Graph（#18）・Automation（#6）・
Routing（#104）・Asset Library（#9）・VCS（#7）が共有する Project Model の
正本を要求する。`crates/synvelis-project` には既に Track/Lane/Clip/Source/
Take/Time/Control・CAS・snapshot/migration/VCS の実装と test が存在するが、
Issue #10 の「責務分界・非破壊編集・時間 domain・stable ID・単一 writer・
save/reload・失敗契約」を一つの canonical 文書鎖として固定する記述がなかっ
た。本 slice はその正本を文書化し、既存 fixture との対応を台帳化する。

上流契約は `project-asset-persistence-architecture.md`（Project/Asset/
Persistence baseline）・`configurable-ppqn-musical-time-architecture.md`
（TIM-01 PPQN/time）・`ed-a`/`ed-b`（編集モデル）・`mda`（TakeGroup/Comp）・
`vs-a`（stable ID/command/save-reload）・`vcs-*`（snapshot/diff/apply/
restore）・`tim-*`/`rcd-*`/`alib-*`/`ctl-*`/`sf-*`/`pc-*` の各設計・spec を
depends-on/related/derives-from に記録した。

## Implementation

### Changes

- `docs/specs/project-model.md`: PM-R-01〜16（要件）・PM-AC-01〜13（受入
  条件）を定義。Issue #10 の受入条件を正規化し、depends-on に VS-A/TIM-01/
  ED-A/ED-B/MD-A/CTL-01/VCS-01/02/05/RCD-02/ALIB-01/02/#86/SYN-IO-02/
  PC-C4-05/RC-D-04/TIM-02 を記録。
- `docs/designs/project-model-design.md`: entity 責務分界表・Track+
  Capabilities/Folder・非破壊編集 ops 表・TakeGroup/Comp・Branch/Scope/
  Lineage・Time domain 体系・PPQN/TempoMap 契約・Stable ID/AmbiguousIdentity・
  単一 writer/CAS/undo/revision 会計・永続化/migration・remote asset・
  失敗分類・Ruviform 境界・既存実装対応表を設計契約として固定。
- `docs/plans/project-model.md`: Goal/Scope/Upstream/File Map/Sequence/
  Dependencies/Task Breakdown/Verification/Open Questions。
- `docs/tasks/project-model-task.md`: Work/Classification/Upstream/
  Dependencies/Affected Files/Done When/Verification。
- `docs/test-specs/project-model.md`: PM-T-01〜22 の Rule・Examples（case→
  test 対応表）・NotRun/Deferred（PM-T-23〜30）・Guarantees・Non-goals・
  Risk。
- `docs/impl/ir/project-model.md`（本書）・`docs/impl/exp/project-model.md`。
- `evidence/sv10/project-model-case-ledger.md`: PM-AC→PM-T→test→Expected/
  Observed/Result + NotRun/Deferred。
- `evidence/sv10/project-model-verification.md`: 実行検証・baseline・
  NotRun lane・判定。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/README.md`:
  `doc_maintenance.js index apply` で再生成（`docs/impl/exp/README.md` は
  `.jsonl` のみを index 対象とする既存規約のため `.md` experiment record
  は index されず、再生成差分なし）。

### Decisions During Implementation

- **既存実装を正本として参照し再定義しない**: `synvelis-project` の module/
  test を contract の根拠とし、設計書は概念/スキーマ契約を固定する。物理
  serializer・schema version（現行 13）は既存実装の確立契約をそのまま参照。
- **新規 test を追加しない**: 本 slice は `crates/` 変更対象外。既存
  fixture が PM-T oracle を満たす範囲は実測 pass、満たさない範囲
  （Folder capability・Branch/Scope・remote cancel・実機系）は design-
  review/NotRun/Deferred で明示し gap を隠さない。
- **`doc-driven-dev-graph` router は blocked を baseline 記録**: pre-existing
  `artifact-graph`/`broken-relation`（無関係文書の deferred relation が
  docs パスとして解決不能）で `current: probe`・`status: blocked`。
  本 slice の新規文書起因ではなく範囲外のため修復せず、doc-status lint/
  audit で個別に整合を確認する方針とした（workspace-model slice と同じ
  対処）。
- **EXP doc は `.md` で作成**: `new_experiment_log.js` が生成する `.jsonl`
  ではなく repo 慣行（workspace-model 等）の `.md` experiment record を
  採用。`EXP-` prefix の uuid62 形式 ID を付与。
- **status 選択**: spec/design/test-spec=`approved`（review-protocol 承認）、
  plan/task=`in-progress`、ir=`completed`（workspace-model 慣行に合わせる）。

## Related Experiments

### Adopted

- `EXP-d1ZLrVSwYuDgjZk3dhMBWn`（`docs/impl/exp/project-model.md`）— 文書
  チェーン生成・fixture 照合・router blocked 確認・review-protocol の試行記録。

### Rejected

- なし。

## Validation

- `mise run fmt-check`: **exit 0 / Pass**（docs slice canonical）。
- `mise run test synvelis-project`: **exit 0 / Pass**。41 test binary・
  460 test 全件 pass（lib unittests・39 integration・2 doc-test）。
  PM-T oracle の既存 fixture を実測確認（`evidence/sv10/project-model-
  verification.md` V 系参照）。
- `node .agents/skills/doc-status/scripts/doc_status.js lint --type
  {spec,design,plan,task,test-spec,impl} --json`: 新規文書起因の blocking
  finding なし（index 未生成の `index-missing-entry`/`orphan-*` は index
  apply で解消）。
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index
  apply`: 触れた 7 dir の index を再生成。
- `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json`:
  project-model の finding なし。
- `node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus
  project-model --json`: **status: blocked**（pre-existing baseline、
  `evidence/sv10/` に記録）。
- review-protocol: spec+design+chain 全体で 1 サイクル、CONFIRMED P0/P1
  blocker なし（`docs/impl/exp/project-model.md` E4 参照）。

## Risks

- 既存 fixture が PM-T oracle を完全には網羅しない領域（Folder capability
  routing 非干渉・Branch/Scope・remote cancel・汎用 clip op 直接 test）は
  design-review/NotRun で明示。実装 gap は別 Issue に委譲。
- doc-driven-dev-graph router の blocked は repo baseline であり、無関係
  文書の broken-relation を本 slice で修復しない（範囲外）。
- index 生成が baseline 問題で失敗した場合は範囲内 dir のみ再生成し、
  失敗 dir を PR に記録する。

## Follow-ups

- 実 GUI Clip/Track 編集: synvelis#112/#114・#11（timeline tools）。
- 実 plugin state/instance: synvelis#176・tim-04・ruviform plugin host。
- 実 RT/execution graph: synvelis#18・#294・ruviform#219/#294。
- 実 storage durability/電源断: synvelis#84/#330・ruviform-io-file real FS。
- Creative Branch 実装: synvelis#277/#278。
- Remote provider fetch/cancel: ALIB 系（alib-04/06/07）・synvelis#9。
- `crates/synvelis-project` の実装変更が必要になった場合は別 Issue で
  Owner を立てる（本 slice は docs のみ）。
