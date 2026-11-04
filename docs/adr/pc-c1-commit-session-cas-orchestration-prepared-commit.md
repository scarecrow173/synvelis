---
id: "ADR-034RoTL4YIadCWevIM5Mfg"
type: "adr"
status: "accepted"
title: "PC-C1: commit 境界を session CAS orchestration + 非破壊 prepared commit に置く決定"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/98"
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
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md"
    - "docs/adr/pc-b-dispatch-boundary-note-entity-prepared-eval.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c1-recording-transaction-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/12"
  defers: []
  deferred-by: []
---

# PC-C1: commit 境界を session CAS orchestration + 非破壊 prepared commit に置く決定

## Status

accepted

## Context

`ControlSession::build_commit_ops` は `self.recording.take()` して
`ProjectOp::Composite` を構築するため、`ProjectController::apply`
の失敗時に点列が消費済みとなり得る(P0 のデータ消失リスク)。
また commit の `expected_revision` は `fixed_revision` ではなく
現在値を採用しており、record_start 以降の project 変更を検出しない。
修正は「誰が CAS の authority を持ち、誰が点列を保持するか」の
責務境界の問題である。

## Considered Options

* Option 1(採用): transaction のライフサイクル(prepare/finish/
  rebase/disposition registry)を `ControlSession` に置き、CAS
  orchestration(prepare→apply→finish)を `ProjectSession` に置く。
  controller の `Command{expected_revision}` CAS が競合の最終
  authority であり、prepare の Conflict は早期の正直な報告。
* Option 2: `ProjectController` 内に録音 transaction 概念を持たせる
  — project aggregate は persisted model + op のみを所有し、session
  状態(gesture/arm/transport)を知らない。責務混入で却下。
* Option 3: commit 前に project を clone して diff し、変更が lane に
  無関係なら自動 Rebase — 「無関係 Track edit でも既定 Conflict」
  の受入と「無条件自動 Rebase 禁止」に反する。
* Option 4: 失敗時に点列を UI 側バッファへ避難する — authoritative
  owner が二重化し、retry/abort の整合が崩れる。点列は transaction
  が finish まで保持し続けるべき。

## Decision

1. **prepare は無破壊、`finish` のみが消費する。**
   `prepare_record_commit(&mut self, project, current_revision)` は
   snapshot+fingerprint+構造検証+`ProjectOp::Composite` 構成のみを
   行い、失敗時に state を `Conflict`/`Rejected` として記録する
   (点列は常に保持)。`finish_record_commit(id, fingerprint,
   applied_revision)` のみが transaction を Committed として
   消費する。`&mut` なのは state bookkeeping のためであり、点列・
   payload の immutable 契約は変わらない。
2. **`expected_revision = fixed_revision`、CAS は controller。**
   prepare が `fixed != current` を `Conflict{fixed,current,
   affected_targets}` で報告し、`Command{expected_revision:
   fixed_revision}` の CAS が apply 時の最終 authority と
   なる。無関係な edit でも既定 Conflict — ユーザーが
   `rebase_preview` を inspect して `approve_rebase` で再承認
   したときだけ再 prepare できる。
3. **冪等は session scope の有限 registry。**
   `(id → fingerprint, applied_revision)` を直近 64 件保持し、
   同 id/同 fingerprint は `AlreadyApplied`、同 id/異 fingerprint
   は `IntegrityConflict`。保証水準は in-memory exactly-once で
   あり、crash 耐性・永続 registry は主張しない(#12 の別契約)。
4. **Recording Gate は seal で閉じる。**
   Stop/LoopWrap/Seek/固定対象の Mode 変更・Disarm(arm 状態変更)/
   Project change(`ProjectSession` の apply/undo/redo 成功時の
   `note_project_changed`)で `SealReason` を記録する。Pause は
   gate の一時閉鎖のみ(write_gates が Playing を要求)で seal
   しない — pause-resume で同一 playback が継続する維持契約。
5. **エラー面は `RecordCommitError` で型付けする。**
   `Conflict`/`Rejected`/`AlreadyApplied`/`IntegrityConflict`/
   `UnknownTransaction`/`NoTransaction`/`Invalid` を区別し、
   UI の `Inspect/Retry/Explicit Rebase/Abort` への mapping を
   型で表す。project `Error` への潰し込みはしない。

## Consequences

- `record_start` が `RecordingTransactionId` を返し、transaction は
  `id`/`fixed_lanes`/`epoch`/`state` を保持する(session schema
  のみで persisted model には触れない — workspace session は非永続)。
- `build_commit_ops`/`control_record_commit` の signature が変わり、
  `controls_ui::record_commit` の error 型が `RecordCommitError`
  になる。呼び出し側は `control_with`/session 経路に揃う。
- recording 中の任意の project apply(無関係な Track edit 含む)は
  gate を seal し commit を Conflict にする — 既存テストで
  mid-session apply に依存する箇所は再構成が必要。
- Rebase は「write gate を再開」しない — `approve_rebase` は commit
  の revision 再承認のみで、seal は維持される。
