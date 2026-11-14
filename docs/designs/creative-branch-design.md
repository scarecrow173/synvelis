---
id: "DESIGN-034XecqdTFl5brsp93RKYN"
type: "design"
status: "draft"
title: "Creative Branch 統合設計 — VariantSet/Scope/Lineage・Compare/Adopt・Owner/DAG (synvelis#277 CB-00)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/277"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/creative-branch.md"
  implemented-by: []
  depends-on:
    - "docs/specs/creative-branch.md"
    - "docs/specs/project-model.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/recording-take-comping.md"
    - "docs/specs/vcs-integration.md"
    - "docs/specs/execution-plan.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/musical-time.md"
    - "docs/specs/media-render.md"
    - "docs/specs/command-gateway.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/cb-00-creative-branch.md"
    - "docs/specs/creative-branch.md"
    - "docs/designs/execution-plan-design.md"
    - "docs/designs/project-model-design.md"
    - "docs/designs/vcs-integration-design.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/recording-take-comping-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/creative-branch.md"
    - "docs/adr/cb-00-creative-branch.md"
    - "https://github.com/scarecrow173/synvelis/issues/277"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/278"
    - "https://github.com/scarecrow173/synvelis/issues/279"
    - "https://github.com/scarecrow173/synvelis/issues/280"
    - "https://github.com/scarecrow173/synvelis/issues/281"
    - "https://github.com/scarecrow173/synvelis/issues/282"
    - "https://github.com/scarecrow173/synvelis/issues/283"
    - "https://github.com/scarecrow173/synvelis/issues/284"
    - "https://github.com/scarecrow173/synvelis/issues/285"
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "https://github.com/scarecrow173/synvelis/issues/287"
    - "https://github.com/scarecrow173/synvelis/issues/288"
    - "https://github.com/scarecrow173/synvelis/issues/289"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/176"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/562"
  defers: []
  deferred-by: []
---

# Creative Branch 統合設計 — VariantSet/Scope/Lineage・Compare/Adopt・Owner/DAG (synvelis#277 CB-00)

## 1. Overview

本書は synvelis#277（CB-00）の design 正本であり、Issue 本文で
2026-11-11 に合意された Creative Branch 製品契約を、レビュー可能な
型・Owner・状態遷移・失敗型・Case ID・Issue 分解 DAG へ落とす。
製品契約の要件面は `docs/specs/creative-branch.md`、方式選択の
根拠は `docs/adr/cb-00-creative-branch.md` が正本であり、本書は
その統合設計を担う。**DESIGN-ONLY**: 本書はコード・実装を含まず、
実装・実 GUI・実 Plugin 受入は未実施のままである。文書の存在は
Gate PASS を意味しない。

Creative Branch は「共通 Origin と明示 Scope を持ち、独立編集・
比較・採用できる制作候補」であり、UX は **Fork → Edit →
Compare → Audition → Adopt（＋Retain/Discard）** に固定される。
Semantic Merge・自動 Blend・部分 Merge は初期対象外である。

## 2. Context and Boundaries

### 2.1 既存契約との接続

| 既存契約 | 正本 | Creative Branch との関係 |
| --- | --- | --- |
| 単一 Writer / CAS / 1 Undo | `docs/specs/project-model.md` PM-R-10〜R-12、`crates/synvelis-project/src/controller.rs`（`Command{expected_revision}`・validate→scratch apply→atomic swap・undo/redo は新 revision） | Fork/Edit/Adopt/Discard は全てこの Controller 経路の Versioned Project 操作。第二の Writer を置かない |
| Immutable Snapshot / 固定 locator | `docs/specs/vcs-01-immutable-snapshot-resolver.md`、`crates/synvelis-project/src/snapshot/types.rs` の `SnapshotIdentity`（project_id + `SnapshotOrigin` + `FixedSnapshotLocator` + `ContentDigest` + schema_version + projection_version） | `VariantSet.origin`/`Branch.created_from` はこの型を再利用。Revision 単独で同一性を断定しない契約と一致 |
| Semantic Diff / ChangeSet | `docs/specs/vcs-02-semantic-diff-changeset.md`、`crates/synvelis-project/src/diff/`（`compare_snapshots`・`ComparisonSpec`・`ComparisonDomain`・`SemanticChange`・`AmbiguityReason`・`ComparisonIssue`・tri-state equality） | Compare の Semantic Diff 層はこの engine の domain adapter を再利用。Branch 比較は SnapshotIdentity ペアの比較として表現する |
| TakeGroup/Comp | `docs/specs/recording-take-comping.md`・`mda-takegroup-comp-effective-projection-placement-contract.md`、`crates/synvelis-project/src/comp.rs`（`TakeGroup`・`Comp`・`CompRegion`・`CompCandidateRef`・`CompTransition`・`CompRegistry`） | Take/Comp 候補は raw を書かず参照のみ。Adopt は `active_comp_id` 等の宣言参照の置換 |
| MIDI Source / SMF | `docs/specs/musical-time.md`・`docs/specs/tim-02-smf-format01-import-export.md`、`project.rs` の `midi_sources`/`smf_documents`、`midi_comp.rs` の `MidiCompSelection`/`compile_midi_comp` | MIDI Clip 候補は Source native event/tick 由来を保持し、note/CC/velocity 編集点を独立化 |
| Routing semantic model | `docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md`、`crates/synvelis-project/src/routing.rs`（`RoutingModel`・`ChannelStrip`・`ProcessorChain`・`Processor`・`PluginRef`・`ProcessorBinding`・`SemanticPort`・`RoutingConnection`・`SendConfig`）・`routing_tx.rs`（`RoutingEditIntent`/`preflight`/`RoutingPlan`/`RoutingEditError`） | Processing Range の Start/End は `ProcessorId`/`SemanticPortId` の Stable Semantic 境界で識別。ordinal 永続化禁止 |
| Plan activation / last-good | `docs/specs/sf-03-map-semantic-runtime-contract.md`・`playback-graph.md`（Configured/Prepared/Published/Active/Observed・`last_good_generation`・generation-tagged dispatch） | Adopt の Project CAS 成功と RT Plan Publish/Activation を別段階・別 Receipt とする契約の既存正本 |
| Plugin State 品質 | `docs/specs/tim-04-plugin-state-snapshot-consistency.md`、`crates/synvelis-project/src/plugin_state.rs`（`StateCaptureQuality` Unsupported/BestEffort/Quiescent/Coordinated・`exact_eligible`・`PluginStateFailureKind`・`PluginStateReceipt`/`RestoreReceipt`） | Missing Plugin で Edit 可能/Exact 不可を分ける判定の既存語彙 |
| Plugin instance identity | `crates/synvelis-project/src/plugin_instance.rs`（`PluginInstanceId`・`PluginInstanceIdentities`・schema v13） | Instrument 候補の instance/state 独立化はこの identity を Live 側で維持 |
| Versioned migration | `crates/synvelis-project/src/migration.rs`（順序付き registry、v5→v6 routing 投影等）、`store.rs` `SCHEMA_VERSION = 13` | Legacy v5 系の読み出しは別実装せずこの registry を再利用。Branch 永続 schema は CB-01 で版管理 |
| VCS 責務分離 | `docs/specs/vcs-integration.md`・`docs/designs/vcs-integration-design.md` | Creative Branch は VCS branch/checkout ではない。VCS Off でも Fork/Compare/Adopt は動作 |

### 2.2 設計上の前提（Issue 合意の転記）

- 合意 UX は Fork → Edit → Compare → Adopt（＋Retain/Discard）。
  Semantic Merge・自動 Blend は初期対象外。
- 対象 Domain: Audio Clip 非破壊加工、Take 選択/Comp 候補、
  MIDI Clip/Source 編集候補、Instrument/Preset/State、FX の
  Processor 範囲、Track/Bus の宣言された構成領域、Arrangement の
  Track 集合×Musical Time×変更 Domain。
- Source/Raw Take は非破壊。Recording Raw/Session/Source/Take
  Capture/Journal/Recovery の所有は #12 系契約。
- 通常 DAW Gate A の完了を本機能の前提にしない。AI・外部 VCS・
  Git Repository は必須条件ではない。
- RCP/2026-11-11 合同契約: Branch は Project versioned 正本、
  Render Recipe/Artifact は非正本 immutable 派生。Fork 直後や
  Adopt 後の同一音響条件は cross-branch CacheHit 可だが、
  BranchRevision 一致だけで有効性を認めず共有 Upstream/Tempo/
  Plugin/Sidechain closure で検証する（接続先は RCP-05 #294）。

## 3. Canonical Model

Issue 概念型を、既存型への写像を含めて契約化する。**正式な
Rust 型名・crate 配置は CB-01（#278）で確定**し、本節の名前は
設計上の canonical 名である。

### 3.1 概念型

```rust
struct VariantSet {
    id: VariantSetId,
    scope: VariantScope,
    origin: SnapshotIdentity,        // ProjectId+固定locator+bytes digest+schema/projection
    branches: Vec<BranchId>,
    adopted: BranchId,
    revision: VariantSetRevision,
}

struct Branch {
    id: BranchId,
    parent: Option<BranchId>,
    created_from: SnapshotIdentity,
    content: BranchContentRef,       // ドメイン別・版付き。Asset は参照共有
    revision: BranchRevision,
}

enum VariantScope {
    AudioClip(ClipId),
    TakeUsage(TakeUsageTarget),
    MidiClip(ClipId),
    InstrumentSlot(ProcessorId),
    ProcessingRange(ProcessingRangeRef),
    StripFields { strip: StripId, fields: StripDomainMask },
    Arrangement { tracks: Vec<TrackId>, time: MusicalRange, fields: ArrangementDomainMask },
}

struct ComparisonSession {
    left: BranchRevisionRef,
    right: BranchRevisionRef,
    pinned_context: EvaluationContext,
    alignment: AlignmentPolicy,
    monitor: MonitoringPolicy,
}
```

### 3.2 既存型への写像（設計上の対応表）

| 概念 | 既存の正本/部品 | 写像規則 |
| --- | --- | --- |
| `SnapshotIdentity` | `synvelis_project::snapshot::SnapshotIdentity`（`snapshot/types.rs`） | そのまま再利用。`origin`/`created_from` の型。`SnapshotOrigin`（Live/Saved/Recovery/VcsStaged/VcsCommitted）と `FixedSnapshotLocator` が provenance を固定し、`ContentDigest`(SHA-256) が bytes を固定する。`capture_generation` は resolver 局所の採番であり永続 identity ではない |
| `ProjectRevision` / `BranchRevision` / `VariantSetRevision` | `controller.rs` の `ProjectRevision(u64)`（live 単調・undo/redo でも巻き戻らない） | Branch/VariantSet の revision は Project revision とは別系列。永続化時は schema 版付き domain として保持し、比較・preflight は `BranchRevisionRef`（BranchId + BranchRevision + content digest）で固定する |
| `BranchContentRef` | ドメイン別 versioned content 参照（CB-01 で schema 化） | Asset/Source/Raw Take は参照共有し所有しない。内容が opaque（Plugin State 等）なら digest+quality のまま保持し数値化しない |
| `VariantSetId`/`BranchId`/`BranchRevisionRef` | 新規 typed stable ID（`id.rs` の `define_id!` 系と同型の `<kind>-<32hex>` tagged 形式を CB-01 で追加） | Live entity ID 空間とは別 namespace。Git commit OID・ProjectRevision・runtime graph ID と混同しない |
| `VariantScope` 各 variant の参照先 | `ClipId`/`TrackId`/`StripId`/`ProcessorId`/`SemanticPortId`/`TakeId`/`CompId`/`SourceId`（`id.rs`） | scope は既存 typed stable ID を指す。branch-local 生成物には branch-scoped ID を使い、Live ID を無条件再利用しない（§6） |
| `EvaluationContext`/`AlignmentPolicy`/`MonitoringPolicy` | Ruviform Generic Probe/Render capability（ruviform#308/#309 整列・#562 比較 Context PoC）+ Synvelis 固定条件（tempo map・plugin snapshot・monitor gain） | pinned_context は Project content revision・Source/Plugin/Tempo/Time mapping・Engine/Graph/Clock を固定した digest 束。Ruviform 側に不足があれば Gap Issue を起票し再実装しない |
| 永続 truth | `Project` aggregate（`project.rs`）+ `ProjectController` + `ProjectStore`（`SCHEMA_VERSION = 13`、head CAS） | VariantSet/Branch は versioned persistent domain として Project schema に追加（CB-01）。History/VCS 固定 Snapshot は `SnapshotResolver` の read-only 経路のみ |

### 3.3 ID / Lineage 規則

- **Branch-local 生成物と Live の Typed Stable ID を分離**する。
  Fork 時に branch 内で新規生成される entity（例: branch 内で
  duplicate した Clip や Comp region）は branch-scoped ID を持ち、
  Live の `id.rs` ID 空間・`id_watermark` 系列とは別に採番する。
- **Adopt 時の決定的 ID mapping/参照再結線**: Adopt の
  `ProjectOp::Composite` 構成時に、branch-local ID → Live ID の
  写像を fingerprint 付きで事前計算し、scope 内の全参照
  （Clip→Source、Comp→Take、Processor→Plugin instance、
  Automation→Control target 等）を再結線する。既存 Live ID を
  無条件再利用しない（衝突時は新規採番 + lineage 記録）。
- **Lineage**: `Branch.parent`/`created_from` が fork 系譜を保持。
  ProjectModel 側の `DerivedFrom` lineage（PM-R-06 の Split
  lineage 等）と整合させ、Adopt で Live へ反映された entity は
  採用元 Branch/Revision への provenance を保持する。
- **旧 Schema 由来が不明なら Ambiguous/Unknown で止める**:
  legacy snapshot からの Branch 再構成で identity が一意に
  決まらない場合は `AmbiguousIdentity` 相当の typed error で
  Reject し、自動 merge・推測結合をしない（vcs-integration の
  既存規則と同一）。
- **Legacy v5 migration を別実装しない**: 旧 schema の読み出しは
  `crate::migration` の順序付き registry（v5→v6 routing 投影等、
  #17/#86/#92 の versioned reader）を再利用する。Branch schema
  自体の版管理は CB-01 が `SCHEMA_VERSION` ladder に追加する。

## 4. State Machine — Fork/Edit/Compare/Adopt/Undo

### 4.1 状態の分離

以下は**別状態**であり、片方の成立を他方の成立とみなさない
（Issue「Edit/Fork/Compare/Adopt 状態と競合契約」）:

| 状態 | 意味 | Live 正式状態への影響 |
| --- | --- | --- |
| Editing | Branch content の編集中（採用候補・非採用候補とも） | 採用候補の通常編集は Live 対象と同一 Controller 操作で同期（§4.3）。非採用候補は永続候補のみ変更 |
| Comparing | `ComparisonSession` が左右 BranchRevision を固定評価中 | なし（read-only 評価） |
| Auditioning | 試聴選択（Monitor/Preview での一時的な聴取） | **なし**。正式 Project/Recording/Automation Write を変更しない |
| Adopted | Adopt CAS が成立し scope が Live へ反映済み | Live revision が 1 進み Undo 可能 |
| Runtime Active | 採用後の RT Plan が新 generation で駆動中 | Project CAS とは非原子的。失敗時は Last-good 維持（§7.3） |

### 4.2 操作と Versioned Project 対応

| 操作 | ProjectController 経路 | Undo/Save/Reload |
| --- | --- | --- |
| Fork | VariantSet/Branch 生成 op（scope fingerprint + origin SnapshotIdentity を固定） | 1 undo unit。Save で永続化、Reload で復元 |
| Edit（採用候補） | Live 対象と候補 content を**同一 op/Composite** で同期変更（双方向ではなく「編集意図を両者へ反映」） | 1 undo unit が Live と候補の両方を戻す |
| Edit（非採用候補） | 永続候補 domain のみを変更する op。Live entity を参照する外部変更は Reject（§8 Oracle） | 1 undo unit。Live revision は進むが Live content は不変 |
| Compare | `ComparisonSession` 生成（pinned context 固定）。Project を変更しない | Session は非永続 Derived。Save/Reload で破棄、再生成は Stale 判定から |
| Audition | Monitor/Preview 選択のみ。Project op を発行しない | 状態を永続化しない |
| Adopt | Preflight → `ProjectOp::Composite`（scope 置換 + ID mapping + lineage）を expected_revision CAS で apply | 1 undo unit。Undo で採用前 Live content を復元（branch 候補は残る） |
| Retain | 状態変更なし（明示的な保持宣言） | — |
| Discard | Branch/候補の破棄 op。Raw Source/外部 Asset/VCS 状態は破壊しない | 1 undo unit（破棄の undo は branch 復元） |

### 4.3 二重正本禁止（状態遷移表）

「採用候補の通常編集は Live 対象とその候補の内容を同一
Controller 操作で同期し二重正本を持たない」「非採用候補の
編集は永続候補のみ変更し正式 Live 対象を暗黙変更しない」
を、次の遷移表で固定する。`L` = Live entity、`C` = 採用候補
branch content、`N` = 非採用候補 branch content。

| # | 前提状態 | 操作 | 結果状態 | 不変条件 |
| --- | --- | --- | --- | --- |
| T-1 | VariantSet なし | Fork(scope) | VariantSet+Branch{parent:None, created_from:origin} 生成。L 不変 | origin は `SnapshotResolver` の read-only capture |
| T-2 | Branch C が採用候補として編集中 | C への通常編集 | L と C が同一 op で同期更新。`L == C@scope` が維持 | 別経路で L のみ/C のみを書き換える op は存在しない |
| T-3 | Branch N が非採用で編集中 | N への編集 | N のみ更新。L・C 不変 | N→L の暗黙参照 write は Oracle が Reject（CB-ORACLE-03） |
| T-4 | C・N が存在 | Compare(C,N) または Compare(L,C) | ComparisonSession 生成。全状態不変 | pinned_context 固定。結果は Stale 可能性を持つ Derived |
| T-5 | C を Audition | Monitor 選択 | audition 状態のみ。L/C/N の content 不変 | Recording/Automation Write・transport 確定を起こさない |
| T-06 | C 採用前 | Adopt(C) | Preflight 通過時のみ `Composite` CAS。L@scope ← C@scope、ID mapping 適用、1 undo unit | 失敗時 L・C・N 全て不変（validate→scratch→swap の既存契約） |
| T-07 | Adopt 成功 | Undo | L@scope が採用前へ復帰（新 revision）。C/N/VariantSet は残存 | Undo は content のみ復元、revision は巻き戻らない（既存契約） |
| T-08 | 任意 | Save → Reload | 永続 domain から VariantSet/Branch/C/N を復元。ComparisonSession/audition は再生成 | branch-local ID・lineage・scope fingerprint が reload 後も一致 |
| T-09 | C が編集中に Live 側で scope 外 edit | C への編集継続 | scope 外 edit は L にのみ影響。C@scope は独立に保持 | scope fingerprint が変わった場合は Preflight で検出（§7.2） |
| T-10 | Adopt(C) 後に runtime 失敗 | — | Project は Adopted のまま。Runtime は Last-good/Pending/Retry | Project を巻き戻さない（§7.3） |

**Fixture 要件（CB-AC-03）**: 上表の各遷移について、headless
fixture で (a) L/C/N それぞれの content digest、(b) live
revision、(c) undo/redo stack 深さ、(d) save→reload 後の
branch-local ID 集合、を採取し二重正本が生じないことを検査
する。Fixture の実装は CB-01（#278）および各 Adapter Issue が
所有し、本書は遷移契約のみを固定する。

## 5. Domain Adapter Contract

Issue 本文の Scope/Adopt 表を、型・Owner・状態遷移・失敗型へ
落とした契約。`Fork 共有` = Fork 時に両候補が参照共有するもの、
`独立化` = Branch 内で独立編集可能になるもの、`Adopt 置換` =
Adopt CAS が Live へ反映する範囲。Owner 列は実装 Issue（§9）。

| Domain | Scope 型 | Fork 共有 | 独立化 | Adopt 置換範囲 | Owner Issue | 主な失敗型 |
| --- | --- | --- | --- | --- | --- | --- |
| Audio Clip | `VariantScope::AudioClip(ClipId)` | Immutable Source/Asset（`SourceId`/`AssetId` 参照共有、bytes 非複製） | Trim/Fade/Stretch/Pitch/非破壊処理レシピ（`ClipPlacement`/`ClipPlayback`/加工 recipe） | Clip の宣言された編集フィールドのみ。Source/Asset 実体は不変 | CB-02 #279 | Scope 外 Source 参照・Asset digest 不整合 → Reject。Source frame mapping 不明 → Degraded |
| Take/Comp | `VariantScope::TakeUsage(TakeUsageTarget)`（TakeGroup/Comp/Lane 配置を指す typed target） | Raw recording session/source/take（`RecordingRegistry`・`TakeId`・segment source range） | Take 使用/Comp 選択/加工候補（`TakeGroup.take_ids`・`active_comp_id`・`Comp` region/transition の候補定義） | Take 使用または Comp 定義の宣言参照。Raw capture は不変 | CB-02 #279 | 録音/Automation Write 中の対象変更 → Reject。Comp handle 範囲外 → Validate error。Raw 書換要求 → Reject |
| MIDI Clip/Source | `VariantScope::MidiClip(ClipId)` | Source native event/tick 由来（`MidiSource`・`SmfDocument` の source-native stream） | Note/CC/Velocity/編集点（clip-local 表現と source projection の対応を版付きで保持） | 対象 MIDI Clip の候補内容。native fidelity（SMF event 順序・meta・sysex）を保持 | CB-03 #280 | native event 復元不能 → Degraded。Tempo/Clock 差による frame ずれ → Stale/Reject。legacy identity 不明 → AmbiguousIdentity |
| Instrument | `VariantScope::InstrumentSlot(ProcessorId)` | 同一 MIDI 入力・通常は後段処理（上流 strip/chain 構造を共有） | Plugin/Instrument instance/State（`PluginInstanceIdentities`・opaque state blob + `StateCaptureQuality`） | Instrument Slot + 関連 Control/Automation 参照（`ParameterTargetId`/`AutomationLaneId`/`ModulationAssignmentId`） | CB-04 #281 | Plugin State 取得不能 → Edit 可・Exact 不可（Stale/Pending/Unavailable）。descriptor/version 不一致 → `NeedsRebind` 系 Degraded |
| FX Processing Range | `VariantScope::ProcessingRange(ProcessingRangeRef)`（chain 内 Start/End = Stable Semantic Processor/Port 境界） | 宣言範囲外の Upstream/Downstream（`RoutingConnection`・前後 `Processor`） | Processor chain/state/automation（範囲内 `Processor` の `PluginRef`/binding/opaque state/automation 参照） | 指定された連続 Processor 範囲のみ。範囲外 wiring は不変 | CB-05 #282 | 範囲外 Send/Sidechain/多出力 → Reject/Unsupported。ordinal のみの境界指定 → Reject（semantic ID 必須）。Plugin State 取得不能 → Exact 不可を表示 |
| Track/Bus | `VariantScope::StripFields { strip, fields: StripDomainMask }` | Scope 外の Project、Bus の既存 owner | 明示 Strip fields/Processing（fader/pan/insert/send/mute/solo 等の `StripDomainMask` で列挙した field のみ） | 列挙した Strip/Track 対象だけ。mask 外 field は不変 | CB-06 #283 | mask 外 field への波及 → Reject。Routing 外部依存（send 先 strip が scope 外）→ 明示検証または Reject |
| Arrangement | `VariantScope::Arrangement { tracks, time: MusicalRange, fields: ArrangementDomainMask }` | Scope 外の Project と Asset | 対象 Track 集合×MusicalRange×領域変更（clip 配置・分割・移動・fade 等の arrangement field） | 複数 Clip 等を 1 Composite CAS/Undo で採用。time range 部分 Merge はしない | CB-07 #284 | 複数 scope の部分失敗 → Composite 全体 Reject（all-or-nothing）。MusicalRange↔Media frame 変換不能 → Degraded |

補足:

- Take/Comp は Audio Clip 行と別 Domain として独立に Fork/Adopt
  可能だが、両者が同一 Lane/Clip を対象にする場合は scope
  重複の Reject 規則（§8.2 CB-ID-03）に従う。
- `StripDomainMask`/`ArrangementDomainMask` は「宣言された構成
  領域」を field mask で明示する仕組みであり、mask に無い
  field は Adopt で絶対に触れない。
- Discard は全 Domain 共通: branch-local 生成物・session 参照を
  破棄するが、Raw Source・VCS checkout/Index・外部 Asset・
  Live entity を破壊しない。

## 6. Processing Branch Boundary Contract

- **分岐可能範囲（初期）**: Track Strip FX Chain の入口から
  Processor 間を経て Chain 末尾まで。Start/End は
  `ProcessorId`/`SemanticPortId` 等の **Stable Semantic
  Processor/Port 境界**で識別し、`ProcessorChain` 内の ordinal
  のみで永続化しない。境界が chain 編集で前後しても semantic
  ID で追跡する。
- **将来拡張（表現のみ）**: 任意連続範囲・Instrument Audio
  Out・Bus/Return/Master への分岐は `VariantScope` の拡張で
  表現可能にしておくが、初期実装対象外。
- **代替経路の意味**: Branch は Master への追加経路ではなく
  **代替経路**である。採用されるまで Master/正式出力へ無断
  加算しない。Branch 内 Processor/Plugin State は独立、
  Upstream/Downstream は共有する。
- **外部依存の境界**: 分岐範囲から外への Send/Sidechain/
  多出力は初期に明示検証し、未サポートなら Reject/
  Unsupported とする（無断配線しない）。RCP-00 #289 の render
  cutset 契約とも整合: 必要 Port 全てを検証し、初期未対応の
  外部 Send/Sidechain/Multi-out は Unsupported を保持する。
- **Runtime 識別子の分離**: Graph Spec/Node ID/Port ID/Plan
  Generation は Ruviform Runtime 識別子であり、Project semantic
  ID（`ProcessorId`/`SemanticPortId`/`RoutingConnectionId`）へ
  昇格しない。semantic→runtime 写像は generation-scoped で
  あり（SF-03 契約）、Branch の scope 指定は常に semantic ID
  で行う。

## 7. Compare / Adopt Semantics

### 7.1 Compare 3 層と固定 Evaluation Context

| 層 | 内容 | データ性質 | 既存部品 |
| --- | --- | --- | --- |
| Semantic Diff | scope 内 entity の意味的変更（field/参照/構造） | Configured（project truth 由来） | `diff::compare_snapshots` + domain adapter、`ChangeSet`/`SemanticChange`/`ComparisonIssue` |
| Signal Diff | 同条件で実測/Render した出力差（PCM/feature） | Measured（render/probe 実測） | Ruviform Media Render/Probe（#562 PoC で gap 判定）、Synvelis render pipeline（`media-render.md`） |
| Perceptual 評価 | 試聴・Blind/Level Match による人間判断 | Inferred（人間の知覚評価） | Audition/Monitor 選択、Difference Explorer UI（CB-09 #286） |

- **混同禁止**: 3 層は別データであり、表示・集計・Gate 判定で
  相互に代替しない。Audio の delta は時間・Gain・Phase・
  Plugin 非決定性を証明できる範囲のみ有効とし、証明不能は
  `Unknown`/`Approximate` とする。
- **固定 Evaluation Context**: `ComparisonSession.pinned_context`
  は Project content revision・Source/Plugin/Tempo/Time mapping・
  Engine/Graph/Clock・monitor 条件を digest で固定する。
  `alignment`（Aligned/Approximate/Unaligned/Unavailable）と
  `monitor`（MonitoringPolicy: gain/level-match/blind 条件）を
  明示する。比較対象以外の MIDI/Instrument/FX/Tempo/Asset/
  Plugin/時刻条件は固定し、他候補との直積評価は明示依頼時のみ。
- **証拠状態語彙**: Configured/Measured/Inferred/Unknown と
  Aligned/Approximate/Unaligned/Unavailable を区別する。
  Recording/Raw Source・Lossy probe cache と高精度
  offline/render を区別し、Graph 変更時に旧 Probe/Analysis を
  現在の証拠と表示しない。
- **時間整合・失効**: 入力領域は AudioClip→Source frame
  mapping、MIDI/Instrument/FX→Project musical/frame mapping、
  Take→recording placement、Arrangement→section/marker/time
  correspondence で写像する。異 Tempo に対して raw PCM null
  test を強制しない。pinned context の構成要素が変化した
  ComparisonSession/Render/Probe は **Stale** を明示し、
  現在の証拠として扱わない。

### 7.2 Adopt — 単一 CAS/Composite + 1 Undo

- **単一操作**: Adopt は候補 B の宣言 Scope を Live 正式状態へ
  反映する `ProjectController` の単一 CAS/Composite + 1 Undo
  である。`ProjectOp::Composite`（all-or-nothing・nested
  Composite 禁止の既存契約）に scope 置換 ops・ID mapping・
  lineage 記録を束ね、`Command{expected_revision}` の CAS で
  apply する。部分適用・scope 外波及は許さない。
- **Preflight**: apply 前に expected ProjectRevision・
  BranchRevision・対象 Scope fingerprint（scope 内 entity の
  content digest 集合）・安定参照/所有者/Asset/Plugin/
  Control/Automation 参照の整合を照合する。`routing_tx` の
  `preflight`→`RoutingPlan` パターンと同型の「検証→計画→
  commit」分離を domain adapter が提供する。
- **Stale/Conflict**: fingerprint・revision 不一致時は
  **自動 Rebase/Merge せず Reject** し、再 Compare を促す
  （typed error で `StaleRevision`/`ScopeConflict`/
  `AmbiguousIdentity`/`UnsupportedBoundary` 等を区別）。
- **非採用候補**: Adopt 前の非採用候補は自動削除しない。
  Retain で保持し Discard は明示操作のみ。Raw Source/VCS
  checkout/Index/外部 Asset を破壊しない。
- **Undo/Save/Reload**: Adopt は 1 undo unit（採用前 Live
  content へ復帰、branch 候補は残存）。Save は既存 head CAS
  経路、Reload は versioned migration を通じて VariantSet/
  Branch/lineage を復元する。

### 7.3 Runtime Publish/Activation — 別段階・別 Receipt

- Project CAS 成立と RT Plan Publish/Activation は**非原子的な
  別段階**であり、別々の Receipt を持つ（SF-03 の
  Configured/Prepared/Published/Active/Observed 区分と整合）。
- Commit 後の Runtime 失敗は **Last-good plan を維持**し、
  Pending/Blocked/Retry を表示する。Project を勝手に巻き戻さ
  ない。retry は常に fresh prepare（新 generation/SourceMap）
  から開始する既存契約に従う。
- Live の正式状態を変える Adopt と一時 Monitor 選択
  （Auditioning）を混同しない。Audition は Plan activation を
  経由しない一時聴取経路であり、generation を進めない。
- Live 候補実行は Plugin instance/state の独立性と PDC/
  transport 同期、RT bounded CPU/memory を前提とし、比較不能
  なら Reason 付き Unavailable とする。非採用候補を常時
  RT 駆動しない。

### 7.4 Evaluation/Resource 契約

- Live と Offline/Cache を対象能力・負荷で選択する。Cache は
  Source/Branch Revision に加え共有 Context・Plugin Snapshot
  品質・Tempo/time mapping・Render settings digest で再利用
  判定し、Graph 変更時に旧結果を現在の証拠と表示しない。
- Fork 直後・Adopt 後の同一音響条件は cross-branch CacheHit
  可（RCP 合同契約）だが、BranchRevision 一致だけで有効性を
  認めず共有 Upstream/Tempo/Plugin/Sidechain closure で検証
  する。Compare 専用 Render/Probe と共有用 Verified Artifact
  は別物（#291/#294 の境界）。
- Missing Plugin: opaque State と Semantic Port/Automation 参照
  が保全できるなら Edit/Adopt 可。Exact audition が不可能
  なら Stale/Pending/Unavailable を表示する
  （`plugin_state.rs` の `StateCaptureQuality`/
  `PluginStateFailureKind` 語彙を再利用）。

## 8. Oracle / Failure Matrix / Case ID

### 8.1 Case ID 体系

Issue 受入条件の `CB-RCP-01` を先頭とする追跡可能な体系:

| Prefix | 範囲 | Owner Issue |
| --- | --- | --- |
| `CB-RCP-*` | Render Checkpoint 連携契約（RCP/2026-11-11 合同契約の Branch 側行） | #294（RCP-05）+ #288（CB-11） |
| `CB-SM-*` | Canonical Model/State machine（VariantSet・遷移表 T-1〜T-10・二重正本禁止） | #278（CB-01） |
| `CB-DOM-*` | Domain adapter 契約（§5 の 7+1 行ごとの scope/fork/adopt/失敗） | #279〜#284（CB-02〜07） |
| `CB-PROC-*` | Processing boundary（semantic 境界・外部依存 Reject/Unsupported） | #282（CB-05） |
| `CB-CMP-*` | Compare 3 層・pinned context・Stale/失効・表示分離 | #285（CB-08）・#286（CB-09） |
| `CB-ADP-*` | Adopt CAS/Preflight/Undo・Runtime 別 Receipt・Last-good 行列 | #278（CB-01）・#288（CB-11） |
| `CB-ID-*` | ID/Lineage/Scope Oracle（§8.2 の Reject 例） | #278（CB-01） |
| `CB-GATE-*` | Cross-Gate（実信号/Plugin/OS/RT/両 Consumer） | #288（CB-11） |

### 8.2 Reject/Degraded/Unsupported Oracle 例

Issue が要求する例を Case 化する。各例は「操作 → 期待結果
（Reject/Degraded/Unsupported と理由）→ 検査点」を持つ。
実装は対応 Owner Issue の fixture が担う。

| Case ID | 例 | 期待結果 |
| --- | --- | --- |
| CB-RCP-01 | Fork→Edit→Adopt→Undo/Reload を Missing Plugin 状態で実行 | Branch/Artifact 参照が音響依存で決まる。Edit/Adopt は opaque State 保全で可、Exact audition は Stale/Pending/Unavailable。Project CAS Receipt と RT Activation Receipt が別段階で記録される |
| CB-SM-01 | 採用候補 C の編集中に C のみを書き換える第 2 経路を試行 | 存在しない — 同期は単一 Controller op のみ。直接書換えは型/検査で Reject |
| CB-SM-02 | 非採用候補 N の編集が Live entity へ波及 | Reject（無断変更 Oracle）。N の content digest のみ変化 |
| CB-SM-03 | Adopt 成功後の Undo → Save → Reload | L@scope が採用前へ復帰、VariantSet/Branch/lineage が reload 後も一致、Undo は revision を巻き戻さない |
| CB-ID-01 | branch-local で duplicate した entity が Adopt で Live ID と衝突 | 決定的 ID mapping で新規採番 + lineage 記録。既存 ID の無条件再利用なし |
| CB-ID-02 | 旧 Schema snapshot から lineage が一意に決まらない Branch を構成 | `AmbiguousIdentity`/`Unknown` で Reject。推測結合・自動 merge なし |
| CB-ID-03 | 同一 Scope の 2 Branch が同一 field を重複所有して Adopt | 無断 Adopt は Reject。scope ownership 検証で検出 |
| CB-ID-04 | Scope 跨ぎの外部参照（例: ProcessingRange 候補が scope 外 strip の send を変更） | Reject または明示 Degraded。無断配線なし |
| CB-PROC-01 | ordinal のみで ProcessingRange 境界を指定 | Reject — Stable Semantic Processor/Port ID 必須 |
| CB-PROC-02 | 分岐範囲外への Send/Sidechain/多出力を含む候補 | Reject/Unsupported（初期）。明示検証記録を残す |
| CB-CMP-01 | pinned_context の Plugin snapshot/Tempo が Compare 中に変化 | ComparisonSession/Result に Stale を明示。旧結果を現在の証拠にしない |
| CB-CMP-02 | Aligned な PCM delta を主張するが Phase 非決定性を排除できない | `Approximate`/`Unknown` に格下げ。Aligned を偽装しない |
| CB-CMP-03 | Missing Plugin 下で Exact audition を要求 | Unavailable + Reason。Stale/Pending 表示と区別 |
| CB-ADP-01 | Preflight fingerprint 不一致（scope 内が Compare 後に変更） | Reject。自動 Rebase/Merge せず再 Compare を促す |
| CB-ADP-02 | Adopt CAS 成立後に RT Plan publish/activation が失敗 | Project は Adopted のまま Last-good 維持。Pending/Blocked/Retry Receipt。Project 巻き戻しなし |
| CB-ADP-03 | 録音/Automation Write 中に scope 対象を変更する Adopt/Edit | Reject（write gate 競合）。対象変更を保留せず明示失敗 |
| CB-ADP-04 | Plugin State 取得不能（`PluginStateFailureKind::Unavailable`/`Missing`）で Instrument Adopt | opaque State+参照が保全される範囲で Adopt 可。Exact 系は Unavailable |
| CB-ADP-05 | 旧 Schema projection 不明の entity を含む Adopt | Reject/Degraded — versioned reader で projection 不能を Unknown として扱い、empty/Equal にしない |
| CB-GATE-01 | 7 Domain × Project Snapshot/Undo/Save・RT Graph active・PCM/Audio Diff・実 OS GPUI・実 Plugin・Ruviform×AudioLens Consumer の独立記録 | #288 が各証拠を別々に評価。Start/Integration/Acceptance を混同しない |

### 8.3 失敗行列（代表）

| 状況 | 分類 | 挙動 |
| --- | --- | --- |
| 録音/Automation Write 中の scope 対象変更 | Reject | write gate と同様に対象変更を拒否。保留・後追い適用しない |
| Plugin State 取得不能 | Degraded | Edit/Adopt 可（opaque 保全）。Exact audition/Signal Diff は Unavailable+Reason |
| Tempo/Clock 差による time mapping 不一致 | Reject/Degraded | pinned_context の tempo map 差異を Stale/Reject で明示。raw PCM null test を強制しない |
| 旧 Schema projection 不明 | Reject | versioned reader の Unknown/PartialProjection を empty/Equal にしない |
| Scope 外 Routing への外部依存（Send/Sidechain/多出力） | Reject/Unsupported | 初期は明示検証→未対応は Unsupported。無断配線しない |
| BranchRevision/Scope fingerprint の Stale | Reject | 自動 Rebase/Merge なし。再 Compare を促す |
| ComparisonSession の pinned context 変化 | Stale 表示 | Derived/Session 結果を現在の証拠にしない |
| RT Plan publish/activation 失敗（CAS 後） | Pending/Retry | Last-good 維持。Project を巻き戻さない |
| Non-adopted 候補の常時 RT 要求 | Unsupported | 設計上禁止。必要時は明示的な Live audition 経路のみ |

## 9. Owner / Issue 分解 DAG

### 9.1 Owner 分離

| Owner | 責務 |
| --- | --- |
| **Synvelis** | Branch 意味論、Typed Scope、ProjectController mutation、Lineage/Adopt、MIDI/Clip/Take/Arrangement、Compare orchestration、UX、Gateway Action、GUI scenario |
| **Ruviform** | DAW 語彙を持たない Generic Time/Typed Graph/Plan activation/Plugin Host/Media Render/Probe/Analysis/Driver-neutral GUI。追加 Generic capability は必要な Gap を再現した場合のみ新 Issue で定義し、既存 ruviform#308/#309/#294 を再実装しない |
| **AudioLens** | f64/長尺/Native 精度 Consumer。Synvelis RT f32/GUI とは独立 Gate で確認。非回帰を CB 各検証 Issue に明記 |

### 9.2 Issue 分解と並行開始規則

`docs/designs/execution-plan-design.md` の SV-CB DAG
（`SV-CB-00(#277) →start SV-CB-01(#278) →start {SV-CB-02〜07
(#279〜284) adapters, SV-CB-08(#285), SV-CB-09(#286 mock),
SV-CB-10(#287), SV-CB-11(#288 fixture)}` → `acc CB-GATE(#288)`、
および `RF#562/#196/#198 + SV-SF-03(#106)/SV-TIM-04(#128)/
SV-CB-00(#277)/SV-MEDIA →int SV-RCP-01〜06`）に整合する。

| Issue | 内容 | Start 条件 | Integration | Acceptance |
| --- | --- | --- | --- | --- |
| #277 CB-00 | 本設計正本（spec/design/ADR） | 済（本 PR） | — | 設計レビュー（本書 §8 の反例レビュー） |
| #278 CB-01 | VariantSet/Scope/Lineage・ProjectController Fork/Edit/Adopt/Undo・永続 Schema | CB-00 正本の確定 | — | Project Snapshot/Undo/Save の headless oracle（CB-SM/CB-ID/CB-ADP 系） |
| #279 CB-02 | Audio Clip/Take/Comp 候補 Adapter | CB-01 の最低限 Typed Contract | media/render 連携 | CB-DOM AudioClip/TakeUsage 行 + CB-RCP-01 の音響依存分 |
| #280 CB-03 | MIDI Clip/Source 候補 Adapter | 同上 | MIDI/SMF/native fidelity | CB-DOM MidiClip 行 |
| #281 CB-04 | Instrument 候補 Adapter | 同上 | Plugin State/instance（#128 系） | CB-DOM InstrumentSlot 行 + Missing Plugin 分岐 |
| #282 CB-05 | FX Processing Range 候補 Adapter | 同上 | Routing semantic/plan（#104〜#106 系） | CB-DOM ProcessingRange + CB-PROC 系 |
| #283 CB-06 | Track/Bus 候補 Adapter | 同上 | Strip/Routing owner（#121/#122 系） | CB-DOM StripFields 行 |
| #284 CB-07 | Arrangement 候補 Adapter | 同上 | timeline/editing（#11 系） | CB-DOM Arrangement 行 + Composite CAS |
| #285 CB-08 | Comparison Orchestrator — Semantic/Signal/Perceptual・Context 固定・Cache | CB-01 Typed Contract + mock/Fake から並行可 | diff engine・render/probe・Ruviform #562 gap | CB-CMP 系 headless oracle |
| #286 CB-09 | GPUI Difference Explorer・Domain Diff View・Audition/Adopt UX | mock/Fake driver から並行可 | GUI scenario（#112/#114 系） | GUI-L2 実機は #114/#176 系 Gate |
| #287 CB-10 | Action/Workflow/CLI/AI Gateway 統合・権限・Provenance | mock/Fake から並行可 | command-gateway（#45 系）・ai-native-baseline（#44 系） | AI OFF/VCS OFF 条件の非回帰明記 |
| #288 CB-11 | Creative Branch 7 領域×PCM/RT/Project/GPUI/Plugin Cross-Gate | 全 CB-01〜10 の Integration 完了 | RCP-05 #294・Ruviform #562・#114/#176 | CB-GATE 系。実信号/Plugin/OS/RT/両 Consumer を別々に記録 |

規則:

- **CB-01（#278）型/Schema を先行正本**とし、ドメイン Adapter
  は CB-01 の最低限 Typed Contract で並行開始可能。各ドメイン
  の既存実装 Issue 全 Close を開始 Blocker にしない。
- Compare semantics/分析契約・Difference Explorer UI/Action/
  CLI fixture はモック/Fake から並行可能。実信号/Plugin/OS
  受入のみ関連 Owner の Integration Gate で合流する。
- **Start / Integration / Acceptance を区別**する。最終検証
  では Project Snapshot/Undo/Save・RT Graph active・
  PCM/Audio Diff・実 OS GPUI・実 Plugin・Ruviform×AudioLens
  Consumer を別々に記録する。
- 既存 Issue との Owner 重複・DAG 循環がないことを §9.3 で
  監査する。

### 9.3 既存 Issue との Owner/DAG 監査結果

Issue #277 受入条件 CB-AC-07 の監査。詳細な写しは
`evidence/sv277/owner-dag-audit.md`。

| 参照 Issue | Owner 領域 | Creative Branch との関係 | 重複/DAG 判定 |
| --- | --- | --- | --- |
| #10 Project Model | Track/Lane/Clip/Source/ID/非破壊 | PM-R-08〜R-09 が VariantScope/BranchContentRef/stable ID を既契約化。CB はその上に構築 | 重複なし。depends-on |
| #12 Recording/Take/Comp | Raw/Take/Comp/Recovery | Raw Source 非破壊・参照非所有を継承。Take/Comp 候補は #279 が担当 | 重複なし。#12 が raw を所有、CB は候補参照のみ |
| #17 Project/Persistence | Snapshot/Save/Recovery/migration | `SnapshotIdentity`・versioned reader・head CAS を再利用 | 重複なし。depends-on |
| #7 VCS | read-only Snapshot/Semantic Diff/Apply | Semantic Diff 層の engine 再利用。Creative Branch ≠ VCS branch（vcs-integration で分離済み） | 重複なし。VCS Off でも CB は動作 |
| #104/#105/#106 SF-01〜03 | Routing semantic/commands/lowering/activation | Processing Range の semantic 境界・plan activation/last-good 契約を再利用 | 重複なし。depends-on |
| #121/#122 CTL-01/02 | Control owner/Plugin binding/Routing atomic commit | Instrument/Track-Bus scope の Control/Automation 参照を typed ID で指す | 重複なし。depends-on |
| #128 TIM-04 | Plugin State 品質 Receipt | Missing Plugin の Edit 可/Exact 不可判定語彙を再利用 | 重複なし。depends-on |
| #114/#175/#176 GUI | GPUI/Plugin Dock/製品 E2E | Difference Explorer は GPUI scenario として #286、実機受入は #114/#176 | 重複なし。実機は別 Gate |
| #23 Execution Plan | 横断 DAG/Gate | SV-CB node/edge は execution-plan-design に既存。本書はその正本化 | 重複なし。depends-on |
| ruviform#308/#309 | Probe 再 Binding/時間整列 | Compare の Aligned/Approximate 判定が利用する Generic 整列契約 | 重複なし。再実装禁止・Gap は新 Issue |
| ruviform#294 | consumer-verify 拡張 | RCP/Consumer 横断証拠の集約先 | 重複なし。references |
| ruviform#562 | Generic Alternate Evaluation Context PoC | Signal Diff/Perceptual の Generic capability gap 判定先 | 重複なし。Gap のみ upstream Issue |

DAG 循環なし: CB-00 → CB-01 → {CB-02〜07, CB-08, CB-09(mock),
CB-10, CB-11(fixture)} → CB-11 acceptance →（RCP-05 #294 へ
integration edge）。execution-plan-design の既存 edge と一致し、
新規の循環 edge を追加しない。

## 10. Non-Functional Constraints

- **RT**: 非採用候補を常時 RT 駆動しない。Live 候補実行は
  Plugin instance/state 独立性・PDC/transport 同期・RT
  bounded CPU/memory を前提とし、比較不能なら Reason 付き
  Unavailable。VCS/Probe/Render 処理は RT スレッドに流さない。
- **永続化**: VariantSet/Branch は versioned persistent domain。
  `SCHEMA_VERSION` ladder に CB-01 が追加し、旧 snapshot は
  `crate::migration` registry 経由の read-only projection。
  source bytes・HEAD・assets を書き換えない。
- **決定性**: Fork/Adopt/ID mapping/fixture は決定的であり、
  同一入力に同一 fingerprint/digest を返す。Live revision は
  単調増加し巻き戻らない。
- **非破壊**: Source/Raw Take/Asset/外部 VCS 状態を破壊しない。
  Discard/Adopt/Compare は raw を書き込まない。
- **UI 技術**: 旧 WebView/SolidJS を正式 UI 契約にしない。
  Ruviform Driver-neutral UI + 標準 GPUI + Synvelis Native App。

## 11. Risks and Trade-offs

| Risk | 緩和 |
| --- | --- |
| branch-local ID と Live ID の衝突・誤再 bind | §3.3 の分離 + Adopt 時決定的 mapping + `AmbiguousIdentity` Reject（CB-ID-* oracle） |
| 採用候補編集の二重正本化 | §4.3 遷移表 + 単一 Controller op 同期を型/検査で強制（CB-SM-*） |
| Semantic/Signal/Perceptual の混同による誤 Gate | §7.1 の 3 層分離 + 証拠状態語彙 + Stale 明示（CB-CMP-*） |
| Adopt 成功後の RT 失敗で Project/Runtime 不整合 | §7.3 の別段階・別 Receipt・Last-good/Pending/Retry（CB-ADP-*） |
| Processing 分岐の外部依存を無断配線 | §6 の semantic 境界 + Send/Sidechain/多出力 Reject/Unsupported（CB-PROC-*） |
| Ruviform Generic capability 不足 | Gap を再現した場合のみ upstream Issue（#562 系）。Synvelis 側で恒久的重複実装しない |
| docs-only で実装契約が空文化 | CB-01 で正式型/schema を確定。設計合意を実測 PASS にしない |

## 12. Deferred Design Concerns

- VariantSet/Branch/VariantScope/ComparisonSession/EvaluationContext
  の正式 Rust 型・serde schema・`SCHEMA_VERSION` 版 — CB-01（#278）。
- 各 domain adapter の op 集合・typed error・fixture 実装 —
  CB-02〜CB-07（#279〜#284）。
- Compare orchestrator の cache key/budget/probe-render pipeline —
  CB-08（#285）。Ruviform 側 gap は #562 PoC で判定。
- Difference Explorer UI・Gateway Action/CLI — CB-09/CB-10
  （#286/#287）。
- Semantic Merge/Cherry-pick/部分 Merge/Blend/Mix — 初期対象外、
  Adopt 契約安定後に別 Issue で再評価。
- 実信号/実 Plugin/実 OS/PCM/RT 受入 — CB-11（#288）+
  #114/#176 系独立 Gate。

## 13. Verification Notes

- [ ] 本書の型・状態遷移・失敗型が Issue #277 本文の合意契約と
  矛盾しないこと（contract-trace: `evidence/sv277/contract-trace.md`）。
- [ ] 既存コード根拠（`controller.rs` CAS、`snapshot/types.rs`
  SnapshotIdentity、`diff/` engine、`comp.rs`/`midi_comp.rs`、
  `plugin_state.rs`/`plugin_instance.rs`、`routing*.rs`、
  `migration.rs`/`store.rs` SCHEMA_VERSION=13）との整合を
  目視照合済みであること。
- [ ] §8 の Case ID/Oracle/失敗行列が Issue 受入条件
  CB-AC-01〜08 を網羅すること。
- [ ] §9.3 の Owner/DAG 監査で循環・重複がないこと
  （`evidence/sv277/owner-dag-audit.md`）。
- [ ] `mise run fmt-check`・doc-status/doc-maintenance の lint/
  index 監査で新規文書由来の finding がないこと
  （`evidence/sv277/verification.md`）。
- [ ] **NotRun**: 実装・実 GUI・実 Plugin・PCM/RT・OS 受入は
  本スライス対象外。CB-11（#288）以降の独立 Gate で評価。
