---
id: "SPEC-034WaMkHXz9f23GE3u0bie"
type: "spec"
status: "approved"
title: "PC-C5 slice4 (UI-C5-04): Recording status read model — Conflict/Prepare/Pending/Applied/Unknown + typed provenance（headless）"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c5-02-03-gesture-stale-inspector.md"
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c5-04-recording-status-design.md"
    - "docs/plans/pc-c5-04-recording-status.md"
    - "docs/tasks/pc-c5-04-recording-status.md"
    - "docs/test-specs/pc-c5-04-recording-status.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice4 (UI-C5-04): Recording status read model — Conflict/Prepare/Pending/Applied/Unknown + typed provenance（headless）

## What Should Be Built

synvelis#102 の UI-C5-04 を headless（fake 検証）で実証する slice。
`crates/synvelis-workspace` に recording transaction の status read model を追加する
（`relation_ui.rs` の pure read fn → DTO パターンに倣う）。

1. **5 表示状態への写像**: session 証拠（open transaction・fixed/live revision・
   caller 保持の prepared payload・last disposition・applied registry・
   record notification log）を `RecordingDisplayState` の
   `Conflict` / `Prepare` / `Pending` / `Applied` / `Unknown` に写像する。
2. **行ごとの説明**: 状態の根拠を `RecordingExplanation` 行として提示し、
   各行は typed provenance（`SealReason` / `RecordCommitError` variant /
   `RecordNotification` kind / `RecordDisposition` 等のドメイン値そのもの）を持つ。
   `StateProjection.provenance` の文字列タグではなく型付き証拠を運ぶ。
3. **状態ごとの合法 action**: `Inspect`（rebase preview）/ `Retry`（冪等 commit 経路）/
   `ExplicitRebase`（approve_rebase）/ `Abort` を状態ごとに合法なものだけ提示し、
   既存 session ops（`control_record_commit`/`control_record_finish`/
   `control_record_rebase_preview`/`control_record_approve_rebase`/
   `control_record_abort`）と整合させる。
4. **transaction/lane 詳細**: open transaction の fixed revision・seal・
   fixed target・lane ごとの記録点数・範囲・明示的 data gap・配送 interval 数を
   verbatim に投影する。

## Why Now

PC-C5-01〜03（PR #211/#240）は merge 済みだが、UI-C5-04 の Recording status は
app が `recording().is_some()` の bool しか見ておらず、Conflict/Prepare/Pending/
Applied/Unknown の説明・明示 Retry/Rebase/Abort・message provenance を UI が
構成するための read model が存在しない。domain machinery
（`RecordingTxnState`・`RecordCommitError` 7 variant・`SealReason` 10 variant・
`RebasePreview`/`RebaseLaneReport`・`RecordDisposition`/`AppliedRecord`・
`RecordNotification`/`RecordHealthSummary`）は #98/#131/#133/#134 で実装済みで、
本 slice はそれを消費するだけで domain 変更を要しない。

## Users and Value

- ユーザー: Automation recording transaction の commit 判断を行う DAW ユーザー、
  および status を描画する Inspector/transport UI（後続 slice）。
- 価値: 「recording があるか」の bool ではなく、なぜその状態か・何ができるか・
  証拠はどこから来たかを型付きで提示し、Conflict での無言 merge や
  UnknownTransaction の握り潰しを防ぐ。

## Scope

### In Scope

- `synvelis-workspace`: 新規 `recording_status.rs`（pure read fn → DTO）、
  `lib.rs` re-export、`tests/uic5_04.rs` fixture。
- `docs/`: spec/design/plan/task/test-spec/impl の文書チェーン。
- `synvelis-app`: 変更なし（`syn.control.status`/`syn.recording.*` op・
  Inspector/transport 表示配線は後続 slice）。

### Out of Scope

- UI-C5-05（実 OS GUI/Plugin window/high-DPI/IME/a11y）— NotRun、#114/#103 gate。
- `controls_ui.rs` への `TransportProjection`/record-action 追加 — 後続 slice。
- `synvelis-app` の app wiring — 後続 slice。
- `synvelis-project`/`synvelis-recording`/`synvelis-player` の変更 — consume-only。
- 永続 "Prepared" txn state の新設 — caller 保持 payload の検証読み取りのみ。

## Requirements

- 表示状態は session 証拠のみから決定的に写像する。推測・補完・文字列 parse をしない。
- `Conflict` は `tx.state == Conflict`（prepare が報告済み）または
  `fixed_revision != live`（prepare が行うのと同じ revision 検査の証拠 —
  次の commit が conflict することが既に決定している）で表示する。
- `Prepare` は open transaction が drift せず、caller が保持する
  `PreparedRecordingCommit` が `preflight_record_finish` を通り
  `expected_revision == live` の場合に限る。「prepare 実行中」という状態は
  存在しない（prepare は `control_record_commit` 内で同期実行）ため、
  Prepare = awaiting-apply（`control_record_finish` 待ち）と定義する。
- `Pending` は open transaction の残り全て（蓄積中・seal 済み・
  `RecordingTxnState::Rejected` — verbatim provenance で区別）。
- `Applied` は open transaction がなく `last_disposition == Committed`。
  `Unknown` は open transaction がなく Committed でもない
  （idle・Aborted・証拠なし）。
- caller 保持の prepared payload は session が観測できない証拠であり、
  read fn が `Option<&PreparedRecordingCommit>` として明示入力を受け、
  `preflight_record_finish` で検証する。caller の主張を無検証で信用しない
  （stale/foreign payload は `PreparedVerdict` で報告）。
- 各 explanation 行は `RecordingProvenance`（型付き enum）を持ち、
  `SealReason`・`RecordCommitError`・`RecordNotification`・
  `RecordDisposition`・`AppliedRecord`・`RecordHealthSummary`・revision 比較・
  session gate を verbatim に運ぶ。
- 合法 action は状態ごとに列挙する: Conflict → Inspect/ExplicitRebase/Abort、
  Prepare/Pending → Inspect/Retry/Abort、Applied/Unknown → なし。
  Conflict で Retry を出さないのは、revision が rebase されるまで
  commit が決定的に再 conflict するため。
- read-side only: project・controller・undo・session 状態を一切変更しない。
  非 RT のみ。
- record notification log は bounded tail（既定 8 件、`notice_tail` で調整可）を
  provenance 付きで提示し、log 溢れは `notices_dropped` で報告する。

## Acceptance Criteria

- [ ] open transaction が `Pending`（未 seal）で写像され、id・state・sealed・
      fixed/current revision・fixed targets・lane ごとの記録点/範囲/gap/配送数が
      verbatim に取れる。
- [ ] foreign project 変更後、commit 試行前でも `fixed != live` で `Conflict` が
      表示され、SealReason::ProjectChange と revision drift の provenance を持つ。
      commit 試行後も `Conflict` のまま。`approve_rebase` 後は `Pending` に戻る。
- [ ] caller 保持の live prepared payload で `Prepare` が表示され、
      `control_record_finish` が適用する。stale payload（追加記録・rebase・
      commit 済み・未知 id）は `PreparedVerdict` で型付きに報告される。
- [ ] commit 成功後は `Applied`（`RecordDisposition::Committed` + applied receipt）、
      abort 後は `Unknown`（`RecordDisposition::Aborted` provenance）になる。
- [ ] `RecordingTxnState::Rejected`（構造 reject）は `Pending` 表示のまま
      verbatim state provenance で区別され、Retry/Abort が合法 action。
- [ ] record notification（QueueFull/DataGap/Rejected/Partial）は bounded tail で
      kind 別 provenance 付きに提示され、health counter も対象 target ごとに出る。
- [ ] `tests/uic5_04.rs` が実 session ops 経由で各状態写像・provenance・
      合法 action を end-to-end 検証する（8〜12 fixture）。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
      `check-app-native` / `lint-app-native` / `test-msrv` / `build` が pass。

## Deferred Design Concerns

- app/`ControlsUi` への status 配線（`syn.control.status`・`syn.recording.*` op・
  Inspector/transport 表示）| read model は headless DTO まで。本 slice は
  `controls_ui.rs`/`synvelis-app` を lease 境界で触らない | 後続 slice
  （sv125-UI 系）| 実画面では status が見えない（headless で semantics は保証済み）
- 実 OS GUI・Plugin window・high-DPI/IME/a11y での Recording status 表示 |
  headless fixture では証明不能 | #114 / UI-C5-05・PC-C6 #103 の外部 gate | NotRun
- record health の全 target 列挙 | `ControlSession` は target 指定 lookup のみ
  公開し、health table の列挙 API を持たない | 必要になった時点で session API 追加を
  別 Issue で検討 | open txn の fixed targets 以外の health は本 view に出ない

## Sources and Relations

- synvelis#102（UI-C5-04 の受入条件の出典）
- synvelis#98 / PC-C1（recording transaction protocol — prepare/finish/rebase/abort）
- synvelis#131/#133/#134（seal・loop pass・record event/notification/health）
- `crates/synvelis-workspace/src/{control,session,relation_ui}.rs`
- `docs/specs/pc-c5-02-03-gesture-stale-inspector.md`（先行 slice の headless 流儀）
