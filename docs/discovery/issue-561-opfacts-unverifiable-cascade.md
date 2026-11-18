---
id: "DISC-561OpFactsUnverifiable01"
type: "discovery"
status: "resolved"
title: "Issue #561 discovery — op_facts under-report: 観測不能 cascade 削除の unsupported 欠落"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/561"
    - "https://github.com/scarecrow173/synvelis/issues/550"
    - "https://github.com/scarecrow173/synvelis/issues/552"
    - "https://github.com/scarecrow173/synvelis/issues/553"
    - "https://github.com/scarecrow173/synvelis/issues/557"
    - "https://github.com/scarecrow173/synvelis/issues/559"
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
    - "docs/impl/ir/issue-550-strip-routing-ops.md"
    - "docs/impl/ir/issue-552-sendgain-seal.md"
    - "docs/impl/ir/issue-553-intent-facts.md"
    - "docs/impl/ir/issue-557-adopt-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-561-opfacts-unverifiable-cascade.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
---

# Issue #561 discovery — op_facts under-report: 観測不能 cascade 削除の unsupported 欠落

## 背景

synvelis#550 で `RemoveSend` の `op_facts` arm に確立された
fail-closed パターン（owner の control closure が観測不能
modulation entity を含む場合 `unsupported` を立てる）は、
wire 全走査で「観測不能 kind の cascade を持つ ungated op」
が `RemoveSend` のみであることを前提にしていた。

synvelis#557 の監査（`docs/impl/ir/issue-557-adopt-facts.md`）
でこの前提を再検証したところ、実際には **3 件の op が同型の
under-report を持つ**ことが判明し、本 Issue `synvelis#561`
として起票された。本 discovery は main `41d85021` のコードで
その事実を再確認し、修正境界を確定する。

## 調査結果

### 観測モデルの境界（SEC-14）

`ResourceKind`（`crates/synvelis-gateway/src/request.rs`）が
名指しできる kind は Project/Track/Lane/Clip/Source/Asset/
Note/TakeGroup/Comp/ParameterTarget/AutomationLane/Strip/Send/
Processor/VariantSet/Branch。`ModulationSource`/
`ModulationAssignment`/`RoutingConnection`/`SemanticPort`/
`SmfDocument`/`TrackMonitorDecl` 等は kind を持たず、
`derive_effects` の before/after diff に現れない。
`resource_map` は `snapshot::ENUMERATED_KINDS` + project のみを
含むため、これらの entity が apply で削除されても effect に
出ない — `op_facts` が `unsupported` を立てない限り
prepare は「完全な効果集合」を偽って報告する。

### under-report が確認された 3 arm

`op_facts`（`crates/synvelis-gateway/src/prepare.rs`）の
match arm と domain apply（`crates/synvelis-project/src/ops.rs`）
の cascade を突き合わせた結果:

1. **`SetSendGainTarget`**（op_facts arm / apply `ops.rs:2405`）
   — 旧 `send.gain_target` が `Some(old)`・新旧異なる・旧
   target が `ControlOwner::Send`+`ParameterRole::SendLevel`
   のとき `release_send_level_target`（`ops.rs:2648`）が発火し、
   lanes（observable）・**assignments**（`a.target == old ||
   a.depth_source == Some(old)`、unobservable）・target 本体
   （observable）を削除する。op_facts arm は reads/
   preconditions のみで unsupported を立てない。

2. **`RemoveTrack`**（op_facts arm / apply `ops.rs:1629`）
   — `strip_of_track` が strip を返すと `remove_strip_owned`
   （`ops.rs:2598`）→ strip/chain/ports 削除 +
   `remove_control_owned(ControlOwner::Strip)`（`ops.rs:2666`）
   が strip-owned targets/lanes/**sources**/**assignments** を
   削除する。`monitoring.tracks` decl も落ちるが、TrackId key
   の strictly-1:1 companion で track delete の観測可能境界に
   包含される。op_facts arm は strip cascade を一切検査しない。

3. **`RemoveMidiSource`**（op_facts arm / apply `ops.rs:1727`）
   — `project.smf_documents.remove(source_id)` を無条件実行。
   `SmfDocument` は `ResourceKind` に variant がなく、
   `detail_of` の Source arm も `sources`/`midi_sources` のみ
   参照するため provenance 削除は完全に不可視。

### 再現実測（修正前 `41d85021`）

`tests/prepare.rs` に pin test を追加して実行
（`evidence/sv561/reproduction.md` に記録）:

- `send.set-gain-target` clear / rebind — `ModulationAssignment`
  を持つ fixture で `Completed`（`unsupported: []`、
  assignment delete は effects にも現れない）
- `track.remove` — strip-owned `Macro` source を持つ fixture で
  `Completed`（同上）
- `midi.source.remove` — `smf_documents` entry を持つ fixture で
  `Completed`（同上）

いずれも prepare が提案を発行し、観測不能 entity の削除が
沈黙する under-report を実測した。

## cascade の発火条件ミラー設計

修正は `RemoveSend` arm の確立パターン踏襲:

- owner の control closure（`ControlOwner`）が観測不能
  modulation entity を含むかを判定する小ヘルパ
  `control_closure_unverifiable` を抽出し、`RemoveSend` /
  `RemoveTrack` 両 arm から使う（同一ドメイン規則
  「owner の control closure に触れる modulation entity の
  削除は観測不能」の authoritative 実装を一箇所に）。
- `SetSendGainTarget` は closure ではなく単一 target の
  release なので、apply の発火条件（旧 target が
  Send-owned SendLevel・新旧異なる）をミラーしたうえで、
  その target に触れる assignment の存在を検査する。
- `RemoveMidiSource` は `smf_documents.contains_key` の
  単純検査（control-closure とは別種なので helper に統合
  しない）。

## companion Issue との境界

- `synvelis#559`（domain adopt 側 fail-closed）は別 Worker
  が `crates/synvelis-project/src/apply/entity.rs`・
  `variant/engine.rs` 等を占有中。本 slice は wire prepare
  層のみで lease 無交差。
- `synvelis#283`（CB-06 tracker）は OPEN のまま。ledger 追記は
  本 slice の lease 外。

## 結論

`op_facts` の 3 arm に unsupported 付与を追加し、domain
apply の観測不能 cascade 削除を fail-closed にする。
domain の削除動作自体は既存仕様であり変更しない。
