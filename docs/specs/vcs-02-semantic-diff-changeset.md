---
id: "SPEC-034UEsXPitEuEsjBM4qjo6"
type: "spec"
status: "proposed"
title: "VCS-02 Semantic Diff Engine — canonical ChangeSet typed identity lineage Unknown tri-state (synvelis#93)"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "ISSUE-93"
    - "ISSUE-7"
    - "ISSUE-92"
  verified_by: []
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-02 Semantic Diff Engine — canonical ChangeSet・typed identity/lineage・Unknown tri-state

## 目的

Issue #93（親設計 Issue #7 VCS-02）。VCS-01（Issue #92）が提供する 2 つの `ImmutableSnapshot` を入力に、Project semantic の差分を **1 つの canonical `ChangeSet`** として決定的に出力する pure / non-RT の Semantic Diff Engine を `synvelis-project` に実装する。Git text diff・file tree diff・PCM/render 差分・runtime graph 差分とは別レベルの「作曲者が認識する project 意味差分」を担う。

本 slice は headless fixture で検証可能な範囲のみ。実 OS・実 Git remote/provider・GUI・実 plugin host probe は `NotRun`/`Unsupported` と明記する。

## 入力と前提

- 入力は VCS-01 の `ImmutableSnapshot`（`identity`, `projection: ReadOnlyProjectProjection`, `validation`, `assets`, `opaque_state`, `completeness`）2 件。
- 比較対象は `projection.project()` が返す schema-current `Project` 集約のみ。SnapshotResolver / ProjectController / ProjectStore / VCS port / live transport / runtime engine を一切 mutate しない。
- 旧 schema の Snapshot は VCS-01 の migration ladder で投影済み。投影由来の領域（後述）は authored 領域と区別して `PartialProjection` として Unknown 側へ倒す。

## 中核契約

### 型

- `ComparisonSpec`: before/after `SnapshotIdentity`、`policy_version`、`included_domains`（`ComparisonDomain` 集合）、`resource_budget`。
- `ComparisonKey`: before/after の content digest・source `schema_version`・`projection_version`・policy version・included domains を含む決定的キー。`ChangeSet` と Change Index/Cache の同一性の根拠。
- `ChangeSet`: `comparison`（spec/key）、`changes`（決定的順序）、`unresolved`（`ComparisonIssue` 列）、`overall: ComparisonEquality`、`completeness`、`summary`。
- `SemanticChange`: `id: ChangeId`、`object: SemanticObjectRef`、`kind: ChangeKind`、`before/after: Option<SemanticValueRef>`（entity canonical JSON の SHA-256 digest 参照）、`affected_range: Option<SemanticTimeRange>`、`evidence`（field-level before/after JSON）、`dependencies`。
- `SemanticObjectRef`: `project_id`、`kind: ObjectKind`、`stable_id`（typed tagged ID 文字列）、`lineage: Option<LineageRef>`（本 slice では常に `None`。lineage schema は #7 の通り将来拡張、旧 snapshot に偽 lineage を付与しない）。
- `ChangeKind`: `Added` / `Removed` / `Modified` / `Moved` / `Reordered` / `Reconnected` / `Replaced`。
- `ComparisonEquality`: `Equal` / `Different` / `Unknown(ComparisonIssue)`。
- `ComparisonIssue`: `UnsupportedSchema` / `MissingAsset` / `MissingOpaqueState` / `AmbiguousIdentity` / `PartialProjection` / `UnresolvedSource` / `InvalidReference`。

### 同一性ルール

- 同一 `project_id` 内では typed stable ID（tagged ID 文字列）を第一の同定キーとする。
- raw u128 が snapshot 間で **異なる ObjectKind** に衝突する場合、Undo watermark reseed / 独立 fork の ID 再発行の積極的証拠とみなし、該当 entity を `AmbiguousIdentity` として unresolved に記録し、自動 match も Added/Removed もしない。
- entity の **owner/lineage 参照 field**（例: `Lane.track_id`, `Note.clip`, `AutomationLane.target`, `ModulationSource.owner`, `ModulationAssignment.source/target`, `ParameterTarget.owner`, `SemanticPort.owner`, `Strip.owner`, `Take`/`CaptureSegment`/`CaptureStream`/`RecordingPass`/`CompRegion`/`CompTransition`/`Comp`/`TakeGroup` の親参照, `Bus.strip`）が同一 ID 間で変化した場合も `AmbiguousIdentity`（同一 entity と確認できない）。例外は仕様明示の `Clip.source_id → Replaced`、`Clip.placement.anchor / lane_id → Moved`、`RoutingConnection.source/destination → Reconnected`。
- `project_id` が異なる比較は自動 match せず `AmbiguousIdentity`（cross-project）として `Unknown`。explicit mapping は本 slice 対象外。
- 同一 kind・同一 raw ID の内容差異は stable-ID match のまま field diff を行う。kind/owner 矛盾のない raw 再発行（同一 kind での undo reseed 等）は 2 snapshot のみからは検出不能であり、creation lineage 記録は #7 通り schema 拡張待ちとして `Modified` 等の確定分類に留める（偽 Ambiguous にもしない）。

### 変更分類ポリシー（fixture 対応）

| 変化 | 分類 |
| --- | --- |
| `Project.name` / `ppqn` / `tempo_map` イベント列 | `Modified`（Project object） |
| `Track.name` | `Modified` |
| `Track.lane_ids` / `Lane.clip_ids` / `Comp.region_ids` / `transition_ids` の同一集合の順序差 | `Reordered`（membership 差は子 entity 側で表現、親 list では重複報告しない） |
| `Lane.track_id` / `Note.clip` 等 owner 参照差 | `AmbiguousIdentity` |
| `Clip.placement.anchor` / `lane_id` | `Moved` |
| `Clip.placement.extent` / `playback` / `muted` | `Modified` |
| `Clip.source_id` | `Replaced` |
| Clip 移動時の子 `Note` | note は clip-relative 座標のため差分なし（delete/add を出さない） |
| `Asset.digest` / `Source.asset_id` / `Source.content_digest` | `Replaced` |
| `Source.state` / `MidiSource.division` / `SmfDocument` 内容 / `ClockEpoch.description` | `Modified` |
| `MidiSource.notes` | 子 `MidiNote` entity として個別 diff（owner source が変われば `AmbiguousIdentity`） |
| `ProcessorChain.processors` | 同一 chain 内順序差 → `Reordered`(Chain)、chain 間移動 → `Moved`(Processor, field=`chain`) |
| `RoutingConnection.source`/`destination` | `Reconnected` |
| `RoutingConnection.kind`/`enabled`/`channel_map`、`SendConfig.tap`/`gain_target`/`connection` | `Modified` |
| `RoutingModel.master` | `Modified`（RoutingModel object） |
| `ParameterTarget`/`AutomationLane`/`ModulationSource`/`ModulationAssignment` の非 owner field、`AutomationLane.regions` | `Modified` |
| entity の存在差 | `Added` / `Removed` |
| UI layout 座標・id_watermark・tempo_map.revision・capture_generation・applied registry の keyed entry は semantic 対象外または Modified のみ | — |

### Unknown / tri-state

- `ComparisonEquality::Equal` は **changes 0 件・unresolved 0 件・両 snapshot `Complete`** の場合のみ。
- 確定 change がある場合は `Different`（unresolved が併存しても `Equal` にはしない。`completeness=Partial`）。
- changes 0・unresolved あり → `Unknown(代表 issue)`。
- snapshot `assets` manifest の `Missing`/`Unknown` entry → `MissingAsset` / `UnresolvedSource` issue（domain が include される場合）。
- snapshot `opaque_state` の `Known` 以外 → `MissingOpaqueState` issue。resolved-but-unprobed は `Unknown`、unresolved binding は `Missing`。
- schema 由来の投影領域:
  - `schema_version < 6`（routing 導入前）→ routing domain 全体を entity diff せず `PartialProjection`（投影 ID は watermark 由来で authored ID と無関係、send/FX/return を表現不能）。
  - `schema_version < 7` → `ParameterTarget.role`/`time_scope` field を suppressed field diff とし、差異があれば `PartialProjection`。
  - `schema_version < 8` → bootstrap `MASTER_FADER_RAW` target を suppressed entity とし、差異があれば `PartialProjection`。
  - `smf_documents` は v8→v9 で空 map 投影が faithful（機能不存在＝真の不在）なので通常 diff。
- digest が一致する AA でも snapshot-level issue（missing asset/opaque/schema suppression は発生しない）を収集し、partial snapshot は `Equal` に昇格させない。

### 決定性・resource

- entity 照合は typed stable ID、field diff は canonical JSON（serde 値）再帰比較、最終 sort は `(domain, object kind, stable_id, field, change kind, change id)` で決定的。
- `ChangeId` は `comparison_key digest + domain + object + kind + field paths` の SHA-256 先頭から生成し、snapshot digest・schema adapter・policy に束縛。
- `ChangeSetCache`/`ChangeIndex` は `comparison_key` digest をキーに `Arc<ChangeSet>` を保持し、上限付き（既定 64）で再計算せず同一結果を返す。derived cache であり Project/Media の正本ではない。
- `ComparisonBudget`（max entities / max changes / max unresolved）超過・`SnapshotCancel` による協調 cancel は `DiffError` で明示失敗。`ProjectController`/`ProjectStore`/VCS port/live transport には触れない。

## スコープ外（NotRun / Unsupported）

- 実 OS filesystem atomicity、実 Git remote/provider、GUI/native UI、実 plugin host probe: **NotRun/Unsupported**。
- PCM/audio content・rendered output・float tolerance/DSP 比較: 別契約（本 slice 対象外）。
- VCS-03 Git Read Provider、VCS-04 History/Visual Diff UI、VCS-05 Apply/Restore。
- Ruviform 一般 Graph/Media domain への Synvelis 専用型の混入（adapter は `synvelis-project` 内に閉じる）。
- `synvelis-app`/`workspace`/`library`/`gateway`/`player`/`recording`/`render`/`theme`/`poc`、`.github/`、`Cargo.toml`/`Cargo.lock`/`mise.toml`（dev-dep 追加なし）。
- `SemanticChange.dependencies` の因果鎖: snapshot は op provenance を持たないため本 slice では空（将来拡張）。
- cross-project explicit identity mapping: `Unknown` のみ。

## 受入対応

Issue #93 / #97 VCS-DIFF-01..08 の headless 検証可能行を `docs/test-specs/vcs-02-semantic-diff-fixtures.md` と `crates/synvelis-project/tests/vcs02_diff.rs` で検証する。
