---
id: "PLAN-034X1FLMu1TV4AQ642APXU"
type: "plan"
status: "approved"
title: "RCD-03 monitor/punch fake session fixture plan"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added:
      - "crates/synvelis-recording/tests/monitor_punch_session.rs"
      - "evidence/sv117/rcd-03-monitor-punch-session.md"
      - "docs/specs/rcd-03-monitor-punch.md"
      - "docs/designs/rcd-03-monitor-punch-design.md"
      - "docs/plans/rcd-03-monitor-punch-plan.md"
      - "docs/tasks/rcd-03-monitor-punch-task.md"
      - "docs/test-specs/rcd-03-monitor-punch.md"
      - "docs/impl/ir/rcd-03-monitor-punch.md"
      - "docs/impl/exp/rcd-03-monitor-punch.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - "docs/specs/README.md"
      - "docs/designs/README.md"
      - "docs/plans/README.md"
      - "docs/tasks/README.md"
      - "docs/test-specs/README.md"
      - "docs/impl/ir/README.md"
      - "docs/impl/exp/README.md"
  implements:
    - "docs/specs/rcd-03-monitor-punch.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-03-monitor-punch-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/rcd-03-monitor-engine.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-monitor-punch.md"
    - "docs/designs/rcd-03-monitor-punch-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-03-monitor-punch.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  defers: []
  deferred-by: []
---

# RCD-03 monitor/punch fake session fixture plan

## Goal

`crates/synvelis-recording` に `DeviceRig` + `MonitorProbe` 合成の
Fake Session fixture（`tests/monitor_punch_session.rs`）を追加し、
monitor/punch の route・fault・recovery・session 分離・commit
意味論を session/staging/commit 層まで pin する。

## Scope

1 ストリーム。`synvelis-recording` の test 追加 + docs + evidence
のみで完結し、`src/` 変更・他 crate 変更を伴わない。

## Upstream Documents

- `docs/specs/rcd-03-monitor-punch.md`
- `docs/designs/rcd-03-monitor-punch-design.md`（approved）
- `docs/specs/rcd-03-monitor-engine.md`（前提契約）
- `docs/specs/rcd-03-monitoring-punch-latency.md`（前提契約）

## Design Inputs

- `docs/designs/rcd-03-monitor-punch-design.md`（approved）

## File Map

- `crates/synvelis-recording/tests/monitor_punch_session.rs` -
  新規。Fake Session fixture helper（`SessionProbe`/
  `punch_plans`/`drive_with_probe`）と 4 test。
- `evidence/sv117/rcd-03-monitor-punch-session.md` - 新規。
  Expected/Observed・fake Pass・実機/GUI NotRun の証拠記録。
- `docs/` doc-chain 7 文書 + docs index（再生成）。

## Implementation Sequence

- [ ] T1: `monitor_punch_session.rs` を作成し、helper
      （`SessionProbe`・`punch_plans`・`drive_with_probe`・
      route 区間 merge/比較・fail-stop 観測・commit 構造
      抽出）を実装。
- [ ] T2: test 1（route/capture coherence + raw/commit 独立
      性）を実装し pass を確認。
- [ ] T3: test 2（device fault の monitor 非依存性:
      gap/disconnect × Off/Auto/On）を実装し pass を確認。
- [ ] T4: test 3（punch 中 device loss → fail-stop →
      verified prefix → commit recoverable 分離 → 新規
      session recovery commit）を実装し pass を確認。
- [ ] T5: test 4（2 session 並行の fault 分離）を実装し
      pass を確認。
- [ ] T6: `evidence/sv117/rcd-03-monitor-punch-session.md`
      を作成。
- [ ] T7: `mise run fmt-check` → `check synvelis-recording`
      → `lint synvelis-recording` → `test synvelis-recording`
      を実行。
- [ ] T8: review-protocol レビュー → 指摘対応 →
      `git fetch origin main && git merge origin/main` →
      docs index 再生成 → impl ir/exp 作成 → commit →
      push → PR。

## Dependencies and Risks

- `PreparedRecordingPlan` は非 `Clone` かつ `MonitorProbe` が
  `&plan` を借用するため、capture 側・monitor 側に同一 spec
  から 2 個構築する（design 記載済み）。
- fail-stop 後の probe feed は route 区間が capture prefix を
  越えるため、coherence 主張は clean 経路（test 1）に限定
  する（design 記載済み）。
- `build_payload` の `draw` に定数を使うため ID 比較はせず
  範囲・構造のみ比較する（既存 `device_recording.rs` と同じ
  運用）。
- recovery session は新規 `FakeInputDevice` で構築し、
  in-session resume を主張しない。

## Task Breakdown

- T1–T5 は実装タスク。T6–T8 は証拠・検証・提出タスク。
- `docs/tasks/rcd-03-monitor-punch-task.md` に詳細化。

## Verification

- [ ] `mise run test synvelis-recording` で
      `monitor_punch_session` の 4 test を含む全 test pass。
- [ ] `mise run fmt-check`/`check synvelis-recording`/
      `lint synvelis-recording` が exit 0。
- [ ] `review-protocol` で blocker なし。

## Open Questions / Gaps

- なし（fixture 構成・helper・検証対象は design で確定済み）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
      consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 worker、依存鎖が
  直列のため分割不要）。
- Independent work streams: none
- User approval required before dispatch: no（coordinator の
  review-protocol で代替）
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute
  task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- reviewer は `drive_with_probe` が `rig.drive` と同一の
  block 刻み・frame 数で probe を feed すること、2 plan 構成が
  gate schedule 同一性を保つこと、fail-stop 系 test が
  coherence を主張しないことを重点確認。
