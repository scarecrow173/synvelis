---
id: "TSPEC-034WaP6Kew3pMEXmdHPiox"
type: "test-spec"
status: "approved"
title: "PC-C5 slice4 テスト仕様 — Recording status read model（UI-C5-04・headless）"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/pc-c5-04-recording-status-design.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c5-04-recording-status.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice4 テスト仕様 — Recording status read model（UI-C5-04・headless）

## Purpose

`recording_status`/`recording_status_with` の写像・provenance・合法 action
が spec の 5 状態契約を満たすことを、実 session ops 経由の fixture で
固定する。domain の既存テスト（`pc_c1.rs`）は lifecycle 自体を保証するが、
「session 証拠 → UI 表示状態」の写像は本 slice が新設する契約であり、
独立した fixture が必要。

## Feature

Recording Status Read Model — session 証拠を
Conflict/Prepare/Pending/Applied/Unknown に写像し、行ごとの説明・
typed provenance・合法 action を提示する。

## Rules

- Rule: open transaction が無い場合、`last_disposition == Committed` なら
  `Applied`、それ以外なら `Unknown`。
- Rule: open transaction があり `tx.state == Conflict` または
  `fixed_revision != live` なら `Conflict`。それ以外で caller 保持
  prepared の verdict が `Live` なら `Prepare`、残りは `Pending`。
- Rule: `Prepare` は永続 txn state ではなく、caller が保持する
  `PreparedRecordingCommit` の検証済み awaiting-apply 投影である。
  `prepared: None` では `Prepare` に到達しない。
- Rule: caller 保持 prepared は `preflight_record_finish`（または同等の
  read 検証）で検証され、stale/foreign/unknown は `PreparedVerdict` で
  型付き報告される。caller の主張を無検証で信用しない。
- Rule: 合法 action は Conflict=[Inspect,ExplicitRebase,Abort]、
  Prepare/Pending=[Inspect,Retry,Abort]、Applied/Unknown=[]。
- Rule: 各 explanation 行は `RecordingProvenance` の typed 値を持ち、
  `SealReason`/`RecordCommitError`/`RecordNotification`/
  `RecordDisposition`/`AppliedRecord`/`RecordHealthSummary`/revision 比較を
  verbatim に運ぶ。
- Rule: read model は read-side only — project・controller・session 状態を
  変更しない（view 構築前後で revision・recording 状態が不変）。
- Rule: record notification は bounded tail で提示され、溢れは
  `notices_dropped` で報告される。

## Examples

- Example: Pending — `record_start` 直後の open txn（未 seal・revision
  一致・prepared 無し）は `Pending`、actions=[Inspect,Retry,Abort]、
  transaction 行に id/state/seal/fixed targets が verbatim。
- Example: Conflict predictive — foreign project 変更で revision が進んだ
  open txn は commit 試行前でも `Conflict`、`RevisionDrift` provenance と
  `SealReason::ProjectChange` を持ち、actions に Retry を含まない。
  `approve_rebase` 後は `Pending` に戻る。
- Example: Conflict confirmed — `control_record_commit` が
  `RecordCommitError::Conflict` を返した後も `Conflict` のまま、
  `tx.state == Conflict` provenance を持つ。
- Example: Prepare — `control_record_commit` で得た prepared を
  `recording_status_with(session, Some(&prepared), ..)` に渡すと `Prepare`、
  `control_record_finish` が適用する。追加記録後の同 payload は
  `IntegrityConflict` verdict で `Pending` に留まる。
- Example: Applied — commit 成功後は open txn 無し +
  `RecordDisposition::Committed` で `Applied`、applied receipt provenance、
  actions=[]。
- Example: Unknown — abort 後は open txn 無し + `RecordDisposition::Aborted`
  で `Unknown`、actions=[]。idle session も `Unknown`。
- Example: Rejected — 構造 reject された txn は `Pending` 表示のまま
  `RecordingTxnState::Rejected` verbatim provenance を持つ。
- Example: notices/health — QueueFull/DataGap/Rejected notification が
  bounded tail に kind 別 provenance で出て、fixed target の
  `RecordHealthSummary` counter が health provenance で出る。

## Guarantees

- 全 5 状態が実 session ops のみで到達可能。
- provenance は型付き enum で、文字列タグへの退化がない。
- actions は状態と一対一で決定的。
- view 構築は session を変更しない。

## Non-goals

- app/ControlsUi 配線・`syn.control.status` op・実 GUI 表示（後続 slice）。
- `controls_ui.rs` への record-action/TransportProjection 追加。
- 実 OS GUI/high-DPI/IME/a11y（UI-C5-05・#114 gate、NotRun）。
- domain 側の lifecycle/prepare/rebase/abort semantics の再検証
  （`pc_c1.rs` が担保）。

## Risk

- predictive Conflict（`fixed != live` で commit 前に Conflict 表示）の
  解釈が UI 期待とずれるリスク → provenance で `tx.state` verbatim と
  drift を区別可能にし、spec/design に明記済み。
- prepared payload の staleness 判定漏れ → `preflight_record_finish` +
  `expected_revision == live` の二重検証で防ぐ。
