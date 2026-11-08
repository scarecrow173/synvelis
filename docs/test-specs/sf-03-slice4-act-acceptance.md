---
id: "TSPEC-034UgPehQz0fcmoolbwWKY"
type: "test-spec"
status: "approved"
title: "SF-03 slice4 受入テスト仕様 — NoRebuild Equivalence・世代連動 control・SF-ACT-05 headless"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
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
    - "docs/plans/sf-03-slice4-act-acceptance.md"
    - "docs/designs/sf-03-slice4-act-acceptance-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
    - "docs/designs/sf-03-slice4-act-acceptance-design.md"
  verified-by: []
  references:
    - "docs/test-specs/sf-03-slice1-lowering-activation.md"
    - "docs/test-specs/sf-03-slice2-fault-reconcile.md"
    - "docs/test-specs/sf-03-slice3-plugin-host-connect.md"
  defers: []
  deferred-by: []
---

# SF-03 slice4 受入テスト仕様 — NoRebuild Equivalence・世代連動 control・SF-ACT-05 headless

## Purpose

SF-03 slice4 の受入契約を固定する。spec
`sf-03-slice4-act-acceptance-norebuild.md` の R1–R4 と Issue #106
SF-ACT-05〜09 系のうち player/project 境界で headless 検証可能な
項目を、`crates/synvelis-player/tests/sf03_l4.rs` を中心とする
test として規定する。GUI/app/session 接続・実 OS/実デバイス・
ruviform RT/render 側の責務は本 spec の対象外（PR の未検証欄に
記録）。

## Feature

Semantic routing の prepare が `RoutingPrepare::{Built,
Equivalent}` を返し、routing fingerprint が同値の新 revision に
対して再 compile を省略して `ExecutionEquivalence` を別証拠として
返す。公開済み世代の chain processor control が世代スコープで
払い出され、旧世代の plugin instance へ誤配送されない。SF-ACT-05
の rapid publish・superseded・queue-full・reconcile drift の既存
契約が回帰なく固定される。

## Rules

- Rule 1（equivalence 成立条件）: `prepare` は最後に publish された
  plan の `RoutingBuildKey` と今回の key が `semantic_eq`（=
  `project_revision` 以外の全 field 一致）かつ
  `project_revision` が異なる場合に限り `Equivalent` を返す。
  同一 revision の再 prepare は常に `Built`。
- Rule 2（equivalence の副作用なし）: `Equivalent` は generation
  を予約せず、lowering・host connect・compile を実行せず、receipt
  を更新しない。live plan の `Active`/`Published` 記述はそのまま
  維持される（新 revision が Active になった偽証拠を出さない）。
- Rule 3（非 equivalence の build）: engine/source-format/
  capability generation・lowering policy version・build mode・
  device clock epoch・runtime instance のいずれかが異なる request、
  および stale candidate・誤 epoch・budget 超過は equivalence に
  ならず通常 build または fault になる。
- Rule 4（reconcile revision-drift 抑制）: live state から再構成
  した key が publish 済み key と `semantic_eq` なら
  `ProjectRevision` mismatch を報告しない。semantic が実際に
  drift した場合は従来どおり `StaleProjectSnapshot` を報告する。
- Rule 5（control 払い出し）: `take_processor_control` は公開済み
  世代の chain processor に対し `(NodeId, PluginNodeControl)` を
  one-shot で返す。未知世代/processor・非 plugin（`StaticProcessorHost`）
  接続・払い出し済みは `None`。
- Rule 6（wrong-plugin 0）: 払い出した control の channel は当該
  世代の plugin instance 専用であり、他世代・他 plugin instance へ
  配送される経路は存在しない。世代退役後の旧 control への push は
  新世代 plugin に届かない。
- Rule 7（teardown 順序維持）: `RetainedConnection` の drop は
  `lease`（host lifecycle teardown）→ `control`（channel end）の
  順で、既存の pending 置換・Active 観測・engine drop の規則を
  変更しない。
- Rule 8（SF-ACT-05 headless 固定）: rapid R1/R2/R3 の receipt
  系列・`Superseded` publish 拒否・`PublishQueueFull`（last-good
  維持）・stale counter → `EventStale` reconcile・
  `DeviceEpochChanged`・fresh retry を `sf03_l4.rs` で固定する。
- Rule 9（SF-ACT-07/08 fail-closed 維持）: Missing port/source/
  unsupported mapping の deterministic failure は両 mode
  fail-closed を維持し、fake silence 成功を出さない。preview 許容
  policy の分岐は本 slice では実装しない。
- Rule 10（回帰なし）: `sf03_l1.rs`/`sf03_l2.rs`/`sf03_l3.rs` が
  新署名で green であり、slice 1〜3 の receipt・fault・teardown
  契約が維持される。

## Examples

- Example 1（Project 名変更 equivalence）: Given semantic 同値の
  project が publish・Active 済み、When `SetProjectName` で新
  revision を commit して prepare、Then `Equivalent` が返り
  generation・host connect 回数・receipt が不変で、
  `ExecutionEquivalence` が configured revision・live plan の
  built-from revision・semantic 一致 key・plan generation を示す。
- Example 2（undo/redo）: Given graph 変更を undo して semantic が
  publish 済み plan と同値に戻る、When prepare、Then
  `Equivalent`。When semantic が実際に異なる revision、Then
  `Built`。
- Example 3（同一 revision 再 prepare）: Given publish 済み plan と
  同一 revision で再度 prepare、When 呼出、Then `Built`（明示的
  rebuild 経路）。
- Example 4（device config 差）: Given engine/source-format
  generation が異なる request、When prepare、Then equivalence に
  ならず `Built`。
- Example 5（reconcile revision-only drift）: Given publish 済み
  plan が revision R1、committed revision が semantic 同値の R2、
  When `reconcile(R2, ...)`、Then `ProjectRevision` mismatch が
  報告されない。When semantic drift、Then `StaleProjectSnapshot`。
- Example 6（control one-shot）: Given publish 済み世代の chain
  processor、When `take_processor_control(gen, pid)` を 2 回呼ぶ、
  Then 1 回目は `Some`、2 回目は `None`。When 未知世代/processor、
  Then `None`。
- Example 7（wrong-plugin 0）: Given 実 `PluginHost` で 2 世代を
  publish、When 旧世代の control で param event を dispatch、
  Then 新世代の plugin instance には届かず旧世代 instance のみが
  受信する。
- Example 8（rapid publish）: Given R1/R2/R3 を連続 prepare→publish、
  When 各 publish、Then receipt が `Published` を記録し RT 未観測の
  pending 世代が次 publish で置き換わり、`last_good_generation` が
  RT 観測まで後退しない。
- Example 9（superseded/queue-full）: Given publish 済み世代以下の
  generation を持つ prepared plan、When publish、Then `Superseded`
  で拒否。Given queue-full 状態で publish 連打、When 上限超過、
  Then `PublishQueueFull` で last-good 維持。

## Guarantees

- `sf03_l4.rs` が Rule 1–10 を網羅し、`mise run test` で green。
- equivalence 時に `planned_generation`・host connect 回数・receipt
  が不変であることを instrumented host/engine state で実測。
- `take_processor_control` が `StaticProcessorHost` 接続で `None`、
  実 `PluginHost` 接続で `Some` を返すことを実測。
- `ParamDispatch` への control 配線は caller 責務であり、engine は
  払い出しのみを行う（bind/dispatch/observe 契約は不変）。

## Non-goals

- GUI/app/session への equivalence/control 配線。
- SF-ACT-05 の実機部分（RT latency/underrun/device epoch restart/
  実 OS・実デバイス）。
- SF-ACT-06 の automation/control plan → `TimedParamEvent` lowering
  の engine 配線、Fast Control active-generation gate。
- SF-ACT-07 の preview 許容 policy 分岐。
- SF-ACT-08 の ruviform RT/render/AudioLens 側実装。
- `capability_revision`/`version` の port 単位照合強化。

## Risk

- equivalence 判定を `checked_key` 前に行うと stale candidate・誤
  epoch・budget 超過を見逃す → Rule 3 で検査順序を固定。
- 同一 revision 再 prepare を equivalence にすると明示的 rebuild
  経路が消える → Rule 1 の revision 差必須条件で防止。
- control を engine が保持し続けると世代退役後も channel が残る →
  one-shot 払い出し + instance 専用 channel で wrong-plugin 0 を
  構造的に保証（Rule 5/6）。
- `RetainedConnection` の drop 順序が変わると teardown→control の
  順序契約が崩れる → field 宣言順で固定し Rule 7 で実測。
