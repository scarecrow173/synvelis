---
id: "SPEC-561OpFactsUnverifiable02"
type: "spec"
status: "approved"
title: "Issue #561 spec — op_facts 観測不能 cascade の fail-closed"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/561"
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
    - "docs/discovery/issue-561-opfacts-unverifiable-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-561-opfacts-unverifiable-cascade.md"
    - "docs/test-specs/issue-561-opfacts-unverifiable-cascade.md"
  refines: []
---

# Issue #561 spec — op_facts 観測不能 cascade の fail-closed

## 目的

`op_facts` が domain apply の cascade で削除される
**観測不能 entity**（`ResourceKind` で名指し不能な
`ModulationSource`/`ModulationAssignment`/`SmfDocument` 等）
を `unsupported` に報告せず、prepare が不完全な効果集合を
「完全」として発行する under-report を解消する。
`RemoveSend` arm（synvelis#550）が確立した SEC-14 境界の
fail-closed を、同型の cascade を持つ残り 3 arm に拡張する。

## 対象 op と cascade

| op | wire capability | cascade 源（domain apply） | 観測不能な削除 |
| --- | --- | --- | --- |
| `SetSendGainTarget` | `synvelis.send.set-gain-target` | `release_send_level_target`（旧 seal 解放時のみ発火） | 旧 target に触れる `ModulationAssignment`（`a.target == old` または `a.depth_source == Some(old)`） |
| `RemoveTrack` | `synvelis.track.remove` | `remove_strip_owned` → `remove_control_owned(ControlOwner::Strip)`（track が strip を持つ場合）＋ `monitoring.tracks.remove`（無条件） | strip-owned `ModulationSource`・closure に触れる `ModulationAssignment`・`TrackMonitorDecl` |
| `RemoveMidiSource` | `synvelis.midi.source.remove` | `project.smf_documents.remove(source_id)`（無条件） | `SmfDocument` provenance entry |
| `Composite` | `synvelis.operation.batch` | member の逐次 apply | member *i* の facts が pinned base ではなく先行 member 適用後の状態を読む必要がある（stale-state under-report） |

## 要件

- `SetSendGainTarget` の `op_facts` は、apply の seal 解放が
  実際に発火する条件（旧 `send.gain_target` が `Some(old)`・
  新旧異なる・旧 target が `ControlOwner::Send(send_id)` +
  `ParameterRole::SendLevel`）をミラーし、そのとき旧 target
  に触れる `ModulationAssignment` が存在すれば
  `unsupported` を立てる。発火条件を満たさない rebind/clear
  （旧 target なし・同一 target・旧 target が SendLevel 以外）
  では立てない — 過剰拒否は既存受理の回帰。
- `RemoveTrack` の `op_facts` は、`project.monitoring.tracks
  .contains_key(track_id)` のとき `unsupported` を立てる
  （decl 削除は strip 有無に関わらず無条件で発火する）。
  さらに `strip_of_track` が strip を返し、その strip の
  control closure（`ControlOwner::Strip(strip_id)`）が
  観測不能 modulation entity を含む場合にも `unsupported`
  を立てる。strip を持たず decl も無い track・closure が
  観測可能 entity のみの場合は立てない。
- `RemoveMidiSource` の `op_facts` は
  `project.smf_documents.contains_key(source_id)` のとき
  `unsupported` を立てる。
- `Composite` の `op_facts` は member を pinned base では
  なく、先行 member を validate→apply で逐次適用した
  scratch の状態に対して評価する（`lower_bound_batch` の
  trial-apply と同型）。これにより batch 内で生成・解放
  される seal/closure 状態に対しても unverifiable 検査が
  正しく発火する。
- `RemoveSend` arm は semantics-preserving な helper 抽出で
  同一ロジックを共有してよいが、observable 挙動
  （unsupported 文言・条件・発火タイミング）は不変とする。
- `monitoring.tracks` decl は `TrackMonitorDecl` で
  `ResourceKind` を持たず `detail_of` も serialize しない
  観測不能 entity — `smf_documents` と同型の keyed companion
  であり unsupported トリガに含める。strip chain/ports 等の
  strictly-1:1 内部構造は引き続き受理内（dangling 参照は
  post-apply oracle が typed reject する）。
- 修正は `crates/synvelis-gateway/src/prepare.rs` の `op_facts`
  のみに閉じる。`crates/synvelis-project/src/**` の domain
  削除動作は既存仕様であり変更しない。

## 受入条件（Issue 転記）

- `set-gain-target`（rebind/clear）・`track.remove` の
  `op_facts` が、観測不能 kind（ModulationSource/
  ModulationAssignment 等、wire の unverifiable 集合と
  parity）を削除し得る場合に `unsupported`/`unverifiable`
  を返す
- mod entity を持つ send/strip に対する該当 op が prepare
  時点で typed 拒絶になることを pin する test
- `RemoveMidiSource` の `smf_documents` 削除について同クラス
  該当かを調査し、該当すれば同様に修正・非該当なら根拠を記録
- 関連する mise 検証が exit 0
- PR は `Refs #561` とし、close 判定は Coordinator に委ねる

## 非対象

- domain（`synvelis-project`）の削除動作・cascade 条件の変更
- `RemoveSend` arm の observable 挙動の変更
- variant 経路の per-op pin（sv#553 の `declared_facts` 伝播で
  op_facts の unsupported は variant edit に自動で届く）
- sv#559 占有パスへの一切の変更
- docs index（`docs/**/README.md`）の編集・再生成
