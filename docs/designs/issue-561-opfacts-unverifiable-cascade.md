---
id: "DESIGN-561OpFactsUnverifiable03"
type: "design"
status: "approved"
title: "Issue #561 design — op_facts 3 arm への unverifiable 検査追加"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/561"
    - "docs/specs/issue-561-opfacts-unverifiable-cascade.md"
    - "docs/discovery/issue-561-opfacts-unverifiable-cascade.md"
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
    - "docs/specs/issue-561-opfacts-unverifiable-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-561-opfacts-unverifiable-cascade.md"
    - "crates/synvelis-gateway/src/prepare.rs"
    - "crates/synvelis-project/src/ops.rs"
---

# Issue #561 design — op_facts 3 arm への unverifiable 検査追加

## 全体構造

修正は `crates/synvelis-gateway/src/prepare.rs` の
`op_facts` のみに閉じる。`RemoveSend` arm が確立した
「owner の control closure が観測不能 modulation entity を
含む場合 `unsupported.push(...)` → `provisional_gate` が
`PreparationRejected { reason: "unverifiable effects: …" }`
で全 caller を fail-closed」というパターンを、残り 3 arm
に拡張する。

```
op_facts(op, project)
  ├─ RemoveSend      → control_closure_unverifiable(Send)   [既存・helper 化]
  ├─ RemoveTrack     → monitoring.tracks.contains_key       [新規・無条件]
  │                  → strip_of_track → control_closure_unverifiable(Strip)  [新規]
  ├─ SetSendGainTarget → release 条件ミラー + assignment 触診          [新規]
  ├─ RemoveMidiSource → smf_documents.contains_key                  [新規]
  └─ Composite       → member を逐次 scratch に対して評価           [新規]
```

## Composite arm の逐次評価（レビュー指摘対応）

`ProjectOp::Composite`（`synvelis.operation.batch` が lowering
する形）の member は `Composite::apply` で**逐次**
validate→apply されるが、従来の `op_facts` は全 member を
pinned base に対して評価していた。これにより:

- member 1 が `SetSendGainTarget` で target を seal し、
  member 2 が別 target へ rebind/clear してその seal を
  解放する batch で、member 2 の facts は base の
  `gain_target`（seal 前）を読み unsupported を立てない
  — apply は中間状態の seal を解放し touching assignment
  を削除するのに、facts はそれを見ない under-report。
- 同型で `AddSend{gain_target}` + `RemoveSend`（bindings
  経由で minted send を参照）も closure 検査をすり抜ける
  — #550 の `RemoveSend` 検査にも同じ stale-state 穴が
  あった（本修正でまとめて解消）。

修正: `Composite` arm は `project.clone()` の scratch を
`CounterIdGenerator::new(project.id_watermark)` で seed し、
各 member の facts を評価した後に `lower_bound_batch`
（`prepare_engine.rs:1562-1626`）と同じ trial-apply 手順
（validate → clone → apply → 成功時のみ swap）で scratch
を進める。member が validate/apply に失敗した場合その
composite は commit 不能なので、scratch は最後の到達可能
状態に留まり後続 member の facts も保守的に評価される。
reads/impacted/preconditions も中間状態を反映するため
従来より正確になる。

## helper 抽出: `control_closure_unverifiable`

`RemoveSend` arm の inline ロジック（closure の
targets/sources 収集 → assignment 触診）を
`ControlOwner` を引数に取る小ヘルパへ抽出する。

- **判定式**（domain `remove_control_owned` の削除条件と
  一致）: owner が持つ `ParameterTarget`/`ModulationSource`
  を closure とし、`!sources.is_empty() || assignments.any(
  a.source ∈ sources || a.target ∈ targets ||
  a.depth_source ∈ targets)` なら unverifiable。
- **`RemoveSend` arm は semantics-preserving**: 同一の収集・
  判定・unsupported 文言
  （`"send-owned modulation entities are outside the
  observable model"`）を維持する。helper は bool を返し、
  文言は各 arm が owner に応じて push する
  （`RemoveTrack` は `"strip-owned modulation entities…"`）。
- **DRY の根拠**: 「owner の control closure に属する
  modulation entity の削除は観測不能」は domain の単一規則
  であり、`RemoveSend`/`RemoveTrack` の 2 caller が同じ
  authoritative 実装を共有するのが整合的。`RemoveMidiSource`
  の `smf_documents` 検査は control-closure とは別種の
  cascade なので helper に統合しない（表面的類似の誤抽象化
  を避ける）。

## 各 arm の仕様

### `RemoveTrack`

`project.routing.strip_of_track(*track_id)` が `Some(strip)`
を返すときのみ `control_closure_unverifiable(project,
ControlOwner::Strip(strip.id))` を評価する。strip 非存在
（cascade なし）・closure が観測可能 entity のみ（seeded
channel targets のみ等）では unsupported を立てない。

**受理内と判断した cascade（unsupported トリガに含めない）**:

- strip/chain/ports の削除: strip は enumerable
  （`ResourceKind::Strip`）、chain/ports は strip に
  strictly-1:1 で対応する内部構造であり、sv#550 の
  `RemoveSend` が connection/port 削除を受理内とした前例と
  同型。port を参照する connection/sends が残れば post-apply
  oracle が typed reject するため silent delete にはならない。

**`monitoring.tracks` decl は unsupported トリガに含める**
（レビュー指摘で初版の受理内判断を修正）:

- `TrackMonitorDecl` は `ResourceKind` を持たず
  `detail_of`/`attributes` にも現れない。`smf_documents`
  entry と同じ「parent key の optional companion で独立
  content を持つ」クラスであり、同一修正内で SMF を
  unverifiable とした判断と整合させる。
- apply（`ops.rs:1641`）は strip 有無に関わらず無条件で
  `monitoring.tracks.remove(track_id)` を実行し、oracle の
  `validate_references` が decl の残存を拒否するため削除は
  必ず発火する — `smf_documents.remove` と同型。
- なお `SetTrackMonitor`/`ClearTrackMonitor` は wire
  lowering 非存在（`_` arm → unsupported）で、この cascade
  以外に decl を消す wire 経路はない。

### `SetSendGainTarget`

apply（`ops.rs:2405`）の発火条件を正確にミラーする:

```rust
let previous = project.routing.sends.get(send_id)
    .and_then(|send| send.gain_target);
if let Some(old) = previous
    && Some(old) != *gain_target
    && targets[old].owner == Send(send_id)
    && targets[old].role == SendLevel
    && assignments.any(|a| a.target == old || a.depth_source == Some(old))
{ unsupported.push(...) }
```

- 旧 target・lanes 自体の削除は `ParameterTarget`/
  `AutomationLane`（enumerable）なのでトリガに含めない
  （`RemoveSend` arm と同じ判断軸）。
- send-owned `ModulationSource` は release で削除されない
  （独立 entity として生存 — `release_send_level_target` の
  コメントどおり）ため検査対象外。
- 発火条件を満たさない場合（旧 target なし・同一 target
  再設定・旧 target が SendLevel seal でない）は
  `release_send_level_target` が走らず unsupported を立て
  ない — 過剰拒否による既存受理の回帰を防ぐ。

### `RemoveMidiSource`

`project.smf_documents.contains_key(source_id)` で
unsupported を立てる。`SmfDocument` は import op
（`ImportMidiSmf`）が insert し `RemoveMidiSource` が
無条件で削除する provenance で、note 射影
（`MidiSource.notes`）を超える tempo/meta/sysex/イベント列
を保持する。`detail_of` の Source arm は `sources`/
`midi_sources` のみ参照し `smf_documents` を serialize
しないため、削除は完全に不可視 — 同クラス該当確定。

## pin test 設計（`tests/prepare.rs`、ファイルローカル）

`mod rig` 共有 harness（`fixture_rig`/`prepare_req`/`op`/
`res_target`/`expected`/`human`/`complete`/`record_for`/
`effect`）のみ使用し、fixture は全てファイルローカルの
`ProjectController` で構築する（cb06 の
`strip_send_*_controller` と同型。`tests/rig/mod.rs` と
sv#559 占有の `cb06_strip_gateway.rs` は変更しない）。

| test | fixture | 期待 |
| --- | --- | --- |
| `sv561_set_gain_target_clear_unobservable_assignment_denied` | send + sealed SendLevel + sealed target を指す assignment | `PreparationRejected` "unverifiable" |
| `sv561_set_gain_target_rebind_unobservable_assignment_denied` | 同上 + adoptable 第2 target | 同上 |
| `sv561_set_gain_target_clear_without_assignment_accepted` | send + sealed SendLevel（assignment なし） | 受理・released target delete が effects に出る |
| `sv561_track_remove_unobservable_strip_cascade_denied` | lane 非保持 track + strip + strip-owned Macro source | `PreparationRejected` "unverifiable" |
| `sv561_track_remove_plain_strip_accepted` | 同上（modulation なし） | 受理・track/strip delete が effects に出る |
| `sv561_midi_source_remove_smf_document_denied` | `ImportMidiSmf` 由来の非参照 midi source | `PreparationRejected` "unverifiable" |

red→green: denied 系 4 件は修正前 `Completed`（under-report
実測 — `evidence/sv561/reproduction.md`）、修正後
`PreparationRejected`。positive control 2 件は修正前後とも
受理で過剰拒否がないことを pin する。

## optional domain pin（`synvelis-project/tests/sv561_unobservable_cascade.rs`）

domain の削除発火条件自体を pin し、gateway mirror が
`ops.rs` から drift しないことを担保する:

- `set_gain_target_clear_releases_seal_and_drops_assignment`
- `remove_track_drops_strip_owned_modulation`
- `remove_midi_source_drops_smf_document`
- `set_gain_target_same_target_does_not_release`（mirror の
  `Some(old) != *gain_target` guard 対応）

## リスクと対策

- **mirror の乖離**: op_facts は apply の発火条件を
  hand-mirror する。domain pin test で発火条件を固定し、
  gateway pin test で verdict を固定する二層で担保。
- **過剰拒否**: positive control 2 件 + 既存
  `cb06_strip_550_send_ops_wire_lowering`（assignment なし
  send の `set-gain-target` clear が variant edit で受理）
  で非回帰を確認。
- **`RemoveSend` 回帰**: helper 抽出は semantics-preserving。
  `cb06_strip_550_send_remove_unobservable_cascade_denied`・
  `cb06_strip_553_variant_edit_send_remove_unobservable_
  cascade_denied` が引き続き Pass することを検証済み。
