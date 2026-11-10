---
id: "TSPEC-034UEueLhP34NeCWclgoux"
type: "test-spec"
status: "approved"
title: "VCS-02 semantic diff fixtures"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "SPEC-034UEsXPitEuEsjBM4qjo6"
    - "DESIGN-034UEtYMoQUWsPmOBrl819"
  verified_by: []
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-02 semantic diff fixtures

Spec `SPEC-034UEsXPitEuEsjBM4qjo6` / Design `DESIGN-034UEtYMoQUWsPmOBrl819` を検証する headless fixture test 仕様。実装は `crates/synvelis-project/tests/vcs02_diff.rs`。

Fixture 構築は VCS-01 と同型: `ProjectController` + fake `ProjectStore`/`SnapshotEnvelope` で `ImmutableSnapshot` を capture（または envelope 直列化→resolver）。snapshot は `serde_json` envelope → `SnapshotResolver` で `ImmutableSnapshot` 化し、diff 入力の正本経路を通す。

## Feature: semantic 分類（VCS-DIFF-01..05）

- **Track rename** → `Modified`(Track, field `name`)。notes/lane に副次 change なし。
- **Clip move vs trim** → anchor 変更は `Moved`、extent/playback 変更は `Modified`。同一 clip の move+trim は `Moved` と `Modified` の 2 change。
- **Clip duplication** → 新 clip id `Added`(Clip) + 親 lane `clip_ids` の membership 差は親 list で重複報告しない。
- **FX/processor reorder** → `ProcessorChain.processors` 順序差 → `Reordered`(Chain)。processor の chain 間移動 → `Moved`(Processor)。
- **Routing edge rewire** → 同一 `RoutingConnectionId` の source/destination 差 → `Reconnected`。新規/削除 edge → `Added`/`Removed`。`SendConfig` tap/gain 変更 → `Modified`。
- **Note inheritance** → clip `Moved` のみで子 `Note` に delete/add/modify が出ない（clip-relative 座標）。
- **Automation edits** → `AutomationLane.regions`/point 差 → `Modified`(AutomationLane)。
- **Asset replacement** → `Clip.source_id` 差 → `Replaced`。`Source.content_digest`/`Asset.digest` 差 → `Replaced`。

## Feature: Unknown tri-state（VCS-DIFF-06..08）

- **AA/AB/BA** → AA(完全既知)は `Equal`・changes 空。AB/BA は `Added`↔`Removed` 反転、`Modified`/`Moved` 等は対称。結果 sort/id は決定的。
- **Unknown 領域** → missing asset → `MissingAsset`/`UnresolvedSource` issue で `Equal` に昇格しない。opaque `Known` 以外 → `MissingOpaqueState`。`schema_version<6` → routing `PartialProjection`。owner field 差 → `AmbiguousIdentity`。cross `project_id` → `AmbiguousIdentity`+`Unknown`。
- **Fork/ID collision** → 同一 raw が before=Track / after=Clip 等の異 kind → `AmbiguousIdentity`（Added/Removed/Modified を出さない）。
- **Determinism/cache/budget** → 同一入力 2 回で `ChangeSet` 完全一致。`ChangeSetCache` は同一 key で再計算せず同一結果。budget/cancel は `DiffError`。

## 非対象（NotRun/Unsupported）

実 OS・実 Git remote/provider・GUI・実 plugin host probe は `NotRun`/`Unsupported`。PCM/render/float tolerance は対象外。
