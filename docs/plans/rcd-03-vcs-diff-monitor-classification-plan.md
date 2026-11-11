---
id: "PLAN-TYvHuxegoERJua2h0P3t1D"
type: "plan"
status: "approved"
title: "RCD-03 VCS semantic diff monitor 宣言分類 slice"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-vcs-diff-monitor-classification.md"
    - "docs/designs/rcd-03-vcs-diff-monitor-classification-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-03-vcs-diff-monitor-classification.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 VCS semantic diff monitor 宣言分類 Implementation Plan

## Goal

schema v12 の authored monitor 宣言（`TrackMonitorDecl`/
`PunchMarks`）を VCS-02 semantic diff の分類対象に追加し、
monitor 宣言の追加・削除・変更が既存分類規約どおり
`SemanticChange` として決定的に現れるようにする。既存 kind の
振る舞い・change-id 規則・apply surface の fail-closed を不変に
保つ。

## Scope

`crates/synvelis-project/src/diff/`（types/engine/adapters）と
`crates/synvelis-project/tests/vcs02_diff.rs`（新規 monitor
分類セクション）に閉じる。`synvelis-app`/`player`/`recording`/
`library`/`workspace`/`gateway`/`ui/`、`apply/` 実装は変更しない
（新 kind は apply surface 外として既存の fail-closed に委ねる）。

## Upstream Documents

- `docs/specs/rcd-03-vcs-diff-monitor-classification.md`
- `docs/specs/vcs-02-semantic-diff-changeset.md`（分類規約の正本）
- `docs/specs/rcd-03-monitor-persistence.md`（follow-up として
  本 slice を記録した永続化 spec）

## Design Inputs

- `docs/designs/rcd-03-vcs-diff-monitor-classification-design.md`
- `docs/designs/rcd-03-monitor-persistence-design.md`（monitor
  POD モデルの正本）

## File Map

- `crates/synvelis-project/src/diff/types.rs` —
  `ObjectKind::{TrackMonitorDecl, PunchMarks}` 追加（enum 末尾、
  `domain()` = `ComparisonDomain::Recording`）、`Recording`
  domain doc comment の拡張、`DIFF_POLICY_VERSION` 1 → 2。
- `crates/synvelis-project/src/diff/engine.rs` —
  `raw_kind_map` の keyed-synthetic 除外コメントに
  `monitoring.tracks` を追記（挙動不変 — 登録しない旨の明記）。
- `crates/synvelis-project/src/diff/adapters.rs` —
  `RecordingAdapter::diff` に `monitoring.tracks` の
  `diff_map(TrackMonitorDecl)` と `diff_punch_marks` singleton
  diff（declaration-presence + `affected_range`）。
- `crates/synvelis-project/tests/vcs02_diff.rs` — monitor 宣言
  分類の新規セクション（add/remove/modify・singleton presence・
  range・非回帰）。
- `docs/` — 本 chain 6 文書 + 各 README index 再生成。

## Implementation Sequence

- [x] spec/design/plan/task/test-spec の doc chain 作成
- [ ] `types.rs`（ObjectKind + domain + policy version）
- [ ] `adapters.rs`（RecordingAdapter monitor diff）+
  `engine.rs`（除外コメント）
- [ ] `tests/vcs02_diff.rs` monitor 分類セクション
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`build` で検証
- [ ] `git fetch origin main && git merge origin/main` 後に再検証
- [ ] review-protocol レビュー → blocker 解消 → impl-ir 記録
- [ ] Conventional Commits で論理分割 commit → push → PR（Refs #117）

## Dependencies and Risks

- 先行 slice（monitor persistence）で `Project::monitoring` POD・
  `SetTrackMonitor`/`ClearTrackMonitor`/`SetPunchMarks` op は
  確定済み — fixture は controller + op で構築する。
- `ObjectKind` variant 追加位置は enum 末尾 — `object.kind as u8`
  を material に使う change-id / issue dedup key の discriminant
  安定のため。
- `monitoring.tracks` を `raw_kind_map` に登録すると全 monitor
  track が `AmbiguousIdentity` を捏造する — keyed-synthetic
  除外の既存規約どおり登録しない（最大の実装リスクであり、
  fixture が cross-kind 衝突ケースをカバーする）。
- doc-driven-dev-graph の route は既存 artifact-graph 不整合で
  blocked（本 slice と無関係 — 先行 slice と同じく canonical
  配置に手動で文書を作成する）。

## Task Breakdown

- `docs/tasks/rcd-03-task-vcs-diff-monitor-classification.md` —
  単一 task（types/adapters 実装 + fixture + 検証）。

## Verification

- [ ] `mise run fmt-check` `check` `lint` `test` `test-rt`
  `test-msrv` `build` 全 pass。
- [ ] `cargo test -p synvelis-project --test vcs02_diff`（補助
  証拠、canonical = mise run test）。
- [ ] fixture が test-spec R1–R4 を網羅。

## Open Questions / Gaps

- monitor 宣言の apply/restore 対応は別 slice（apply surface
  追加は transplant/cascade 設計が必要）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker dispatch 内で実装）。
- Independent work streams: none。
- User approval required before dispatch: yes（worker contract に
  より review-protocol の blocker なしを承認として扱う）。

## Review Handoff

- reviewer は keyed-synthetic 除外（raw_kind_map）、punch の
  declaration-presence 意味論、`Recording` domain 選択、
  `as u8` discriminant 安定、policy version bump の必要性、
  既存 kind 非回帰を重点確認。
