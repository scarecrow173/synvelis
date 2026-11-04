---
id: "SPEC-034RoTBwuGm6u9xW7IxYBu"
type: "spec"
status: "approved"
title: "PC-C1 Recording Transaction — prepare/apply/finalize 分離・Revision競合・失敗後保持"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/6"
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
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
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
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/synvelis/issues/134"
  defers: []
  deferred-by: []
---

# PC-C1 Recording Transaction — prepare/apply/finalize 分離・Revision競合・失敗後保持

## What Should Be Built

Issue #98(P0・Automation 録音受入の Blocking Condition)。現行
`ControlSession::build_commit_ops` は commit 時に `self.recording.take()`
して `ProjectOp::Composite` を構築するため、`ProjectController::apply`
が Stale/Validation 等で失敗した場合に録音点列が消費済みとなり得る
(データ消失リスク)。また `fixed_revision` は record_start 時に保存
されるが commit は `self.revision()` の現在値を採用し、開始時点の
プロジェクト状態との CAS になっていない。

本 spec では commit を **非破壊 `prepare` → `ProjectController` CAS
`apply` → 成功時のみ `finish`** の 3 段に分離し、失敗・競合時に点列
を保持したまま `Pending/Conflict/Rejected` で報告する。ユーザーの
操作面は `Inspect / Retry / Explicit Rebase / Abort` のみで、無条件の
自動 Rebase は禁止する。

## Requirements

### PC-C1-01: 非破壊 prepare + CAS apply + finalize

- `RecordingTransaction` は `record_start` 時に `RecordingTransactionId`
  (session 内で単調・一意)、`fixed_revision`、TargetIDs、LaneIDs
  (開始時の write-lane 写像の固定)、開始位置、transport epoch context
  を保持する。
- `prepare_record_commit(project, current_revision)` は
  - 点列の immutable snapshot + deterministic fingerprint を構成する。
  - Target/Lane/Scope/Range/Interpolation/Source の整合と Budget を
    確認し、`ProjectOp::Composite` を**無破壊で**構成する
    (transaction の点列は消費しない)。
  - `fixed_revision != current_revision` を既定
    `Conflict{fixed,current,affected_targets}` で報告する
    (無関係な Track edit を含む一切の revision 差分。最新 Revision
    への無断 rebinding/merge を行わない)。
- `ProjectController::apply(Command{expected_revision,
  op})` が成功したときだけ `finish_record_commit(id, fingerprint,
  applied_revision)` が transaction を Committed として消費する。
  apply 失敗時は `Pending/Conflict/Rejected` のまま点列を保持する。
- Commit 成功と Saved HEAD は別状態のまま(commit は save を伴わない)。

### PC-C1-02: 冪等・整合性

- 同一 `id` + 同一 fingerprint の二重 finish は `AlreadyApplied` を
  返し、二重書き込みを防止する。同一 `id` + 異 payload は
  `IntegrityConflict` で拒否する。
- 保証水準: in-memory・session scope の exactly-once(直近 N 件の
  有限 registry)。UI メモリだけの crash 耐性・永続 exactly-once は
  主張しない。Persisted Registry が必要になれば schema owner と
  別契約で連携する。
- ACK 喪失後の id を持たない session-level retry は、直近の
  disposition が Committed なら `AlreadyApplied` を報告する。

### PC-C1-03: Explicit Rebase のみ

- `rebase_preview(project, current_revision)` はユーザーが再適用を
  要求したときだけ呼ばれ、fixed target/lane の存在、記録範囲と既存
  region の重なり(Replace が上書きする区間)、fixed/current revision
  を表示する。
- `approve_rebase(current_revision)` が `fixed_revision` を現在値へ
  再承認し、transaction を `Pending` へ戻す。再承認後の prepare は
  新しい revision で再度構造検証される。
- 無条件の自動 Rebase は行わない。競合・失敗時の UI 面の選択肢は
  `Inspect / Retry / Explicit Rebase / Abort` に限定する。

### PC-C1-04: Recording Gate の閉鎖

- Session Stop / Loop wrap / Seek / 固定対象の Mode 変更 / 固定対象の
  Disarm / Project change(apply/undo/redo の成功)で、開いている
  transaction の write gate を seal する。seal は書き込みのみを止め、
  commit/abort は依然として可能(LoopWrap の既存契約どおり)。
- Pause は一時的な gate 閉鎖とする(既存契約: paused 中は書き込みが
  着地せず、resume で同一 playback が継続する)。Pause で transaction
  を恒久的に seal しない。
- `record_start` 後の write-lane 再 bind は、固定対象については拒否
  する(開始時写像が authoritative)。

### PC-C1-05: 座標・文脈の混同なし

- 記録点は scope 座標(clip/note-relative beats)のまま commit される
  (PC-B の既存契約維持)。Clip/Note scope、Tempo/PPQN、Transport
  Generation(epoch context)を混同しない。
- transaction の点数は有限 Budget で制限し(NonRT メモリ/サイズ
  制限)、超過する新規書き込みは拒否する(既録点は失わない)。
- RT callback から project command/alloc/block を行わない — 全経路は
  session(non-RT)層に留まる構造的不変条件。

### 維持契約

- LoopWrap Seal、Off/Read/Touch/Latch/Write、Arm+Record+Rolling、
  Scope 座標、Modulation 前の手動値、1 session = 1 undo は実装済み
  契約として維持する。

## Non-Requirements

- 実 Plugin・実 GUI・MultiWindow/GUI Focus の実経路検証
  (PC-C6 #103 の Domain Oracle、SV-GUI-05 #114)。
- 録音データの Durable Crash Recovery / persisted commit registry
  (#12 Recording Journal との別契約)。
- Region/Loop semantics 自体(#132/#133 は本契約の後続利用側)。
- 破壊的 Decimation、外部 CC、Audio-rate、MultiPass 等
  (#135〜139 の別 Feature Gate)。

## Acceptance Criteria

- REC-C1-01: committed success => 1 Project Undo、Raw/Manual 前
  Modulation 値、Save separate、Target も Lane も正しい。
- REC-C1-02: Stale ProjectRevision(無関係 Track edit 含む) =>
  default Conflict、点列保持、誤適用なし。Explicit Rebase 後にのみ
  Commit。
- REC-C1-03: Lane/Target deleted or changed で composite validate
  fails => Project/Revision/Undo 不変、点列・fixed targets 保持。
- REC-C1-04: identical retry/duplicate/ACK lost => 二重書込み防止、
  別点列同 ID は IntegrityConflict(保証水準を明示)。
- REC-C1-05: LoopWrap/Touch release/Latch/Write pause-resume/disarm/
  gesture_cancel/old receipt => gate 閉鎖・誤書込みなし。
- REC-C1-06: Clip/Note scope、Tempo/PPQN/Transport Generation 混同
  なし。Record Transaction の NonRT メモリ/サイズ制限、RT callback
  で Project command/alloc/block なし。
- `mise run fmt-check` / `check` / `lint` / `test` 全 green。
- doc chain 完走、docs/impl に impl record。
