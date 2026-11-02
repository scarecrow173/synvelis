---
id: "PLAN-fEinABRn8LJsKRmVkn0kNJ"
type: "plan"
status: "approved"
title: "MD-A 実装計画 — TakeGroup/Comp・Effective Comp 投影・正規化 Placement/Playback"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/adr/mda-normalized-placement-comp-schema.md"
    - "docs/designs/mda-comp-effective-projection-placement-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/mda-model-poc-acceptance.md"
  references: []
  defers: []
  deferred-by: []
---

# MD-A 実装計画 — TakeGroup/Comp・Effective Comp 投影・Placement/Playback

## Goal

spec の R-1..R-10 を実装し、test-spec の MD-A-01..06 + 既存スイートを
全 green にする。doc chain (spec/ADR/design/plan/tasks/test-spec/impl
record) を完走する。

## Scope

単一実装ストリーム。変更面: `synvelis-project` (正本・ops・投影)、
`synvelis-recording` (commit clip 生成の適合)、`synvelis-player`
(lowering 解決の適合)、各 crate テスト、doc chain。

## Upstream Documents

- spec: `docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md`
- ADR: `docs/adr/mda-normalized-placement-comp-schema.md`
- design: `docs/designs/mda-comp-effective-projection-placement-design.md`

## Design Inputs

- 旧 `TimeAnchor` (区間 enum) → `ProjectRange` rename、新
  `TimeAnchor` = 点 anchor。`Clip` は `placement`+`playback` を保持。
- Comp 系は `CompRegistry` + `CompOp` (project-domain)。
- Effective Comp は純粋導出 `project_comp`。Editor target は型契約のみ。
- `SCHEMA_VERSION = 2`。

## Tasks

| # | Task | 内容 |
| --- | --- | --- |
| 1 | mda-task1 | 正規化 placement 型・Clip 移行・consumer 適合 |
| 2 | mda-task2 | Comp スキーマ (ID/registry/POD) + CompOp + 検証 |
| 3 | mda-task3 | Effective Comp 投影 + editor target 型 |
| 4 | mda-task4 | MD-A-* 受入テスト + 全検証 + impl record |

## File Map

- `crates/synvelis-project/src/id.rs`: +4 Stable ID
- `crates/synvelis-project/src/time.rs`: `MusicalDuration`/`MediaDuration`
- `crates/synvelis-project/src/range.rs`: `ProjectRange` (= 旧 TimeAnchor 移管先に応じて再配置)
- `crates/synvelis-project/src/project.rs`: `Clip` 移行・`CompRegistry`
  登録・`validate_references` 拡張
- `crates/synvelis-project/src/placement.rs` (新): TimeAnchor/ClipExtent/
  ClipPlacement/ClipPlayback/PlaybackMapping/resolve
- `crates/synvelis-project/src/comp.rs` (新): Comp POD/CompOp/検証/投影/
  editor target
- `crates/synvelis-project/src/ops.rs`: AddClip/MoveClip/SetClipExtent/
  SetClipPlayback
- `crates/synvelis-project/src/store.rs`: `SCHEMA_VERSION = 2`
- `crates/synvelis-project/src/lib.rs`: export 更新
- `crates/synvelis-recording/src/commit.rs`: clip 生成/検査の適合
- `crates/synvelis-player/src/lowering.rs`: resolve 消費 + UnsupportedMapping
- tests: `synvelis-project/tests/md_a.rs` (新)、`controller_store.rs`、
  `vs_d.rs`、`l1_e2e.rs`、`l2_e2e.rs`、`x_e2e.rs` 適合

## Risk Register

| Risk | 対応 |
| --- | --- |
| Clip スキーマ変更が consumer を破壊 | 機械的適合 + 既存 suite が検証 |
| handle 検証の rate 不在 | TakeGroup.source_rate の明示近似 (design 記録) |
| take project placement 未実装 | Take candidate の source_offset 明示契約 |
| 旧 snapshot 互換 | SCHEMA_VERSION bump で明示拒否 (PoC・migration 対象なし) |

## Verification Matrix

| 受入 | 検証 |
| --- | --- |
| MD-A-01 | `md_a.rs` takegroup→comp→projection、save/reload/undo 同一 |
| MD-A-02 | 非重複・隣接・overlap・handle 各拒否 + atomic |
| MD-A-03 | Musical+Natural/Absolute+Natural/Musical+Stretch の tempo 跨ぎ再導出 |
| MD-A-04 | 境界一意帰属 + overlap 限定重複 |
| MD-A-05 | undo/redo で ID/状態復元 |
| MD-A-06 | Raw version 0 束縛拒否 |
| 既存 | workspace test 全 green、fmt-check/check/lint |

## Rollback

ブランチ `scarecrow173/sv42-mda-comp-model` 上の commit 単位で revert
可能。`SCHEMA_VERSION` bump により新規保存のみ v2。
