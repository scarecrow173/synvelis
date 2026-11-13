---
id: "DESIGN-034X6iC9O7gEAUDL5Sxqqv"
type: "design"
status: "approved"
title: "Project Model Design — entity schema, single-writer/CAS/undo, time domains, persistence"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/10"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/project-model.md"
  implemented-by: []
  depends-on:
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/ed-a-editing-model-design.md"
    - "docs/designs/ed-b-media-domain-editing-design.md"
    - "docs/designs/mda-comp-effective-projection-placement-design.md"
    - "docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md"
    - "docs/designs/vcs-01-immutable-snapshot-resolver-design.md"
    - "docs/designs/vcs-02-semantic-diff-engine-design.md"
    - "docs/designs/vcs-05-apply-restore-headless-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
  refines:
    - "docs/designs/project-asset-persistence-architecture.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/18"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/65"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/176"
    - "https://github.com/scarecrow173/synvelis/issues/277"
    - "https://github.com/scarecrow173/synvelis/issues/278"
    - "https://github.com/scarecrow173/synvelis/issues/279"
    - "https://github.com/scarecrow173/synvelis/issues/280"
    - "https://github.com/scarecrow173/synvelis/issues/284"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/330"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/219"
    - "https://github.com/scarecrow173/ruviform/issues/294"
  defers: []
  deferred-by: []
---

# Project Model Design — entity schema, single-writer/CAS/undo, time domains, persistence

> Status: approved — review-protocol 承認済み。本書は
> `docs/specs/project-model.md`（SPEC-034X6inBx3ortD0y20pxqk）の設計化であり、
> `crates/synvelis-project` の**実装済み事実**と本設計契約を区別して記述する。
> headless fixture の対応は `docs/test-specs/project-model.md` と
> `evidence/sv10/` を参照。

## Context

`docs/designs/project-asset-persistence-architecture.md` は Project/Asset/
Persistence の初期ベースライン、`configurable-ppqn-musical-time-architecture.md`
は TIM-01 の PPQN/time 契約、`ed-a`/`ed-b` は編集モデル、`mda` は
TakeGroup/Comp/effective projection、`vs-a` PoC は stable ID/command/
save-reload の基盤をそれぞれ確立した。`crates/synvelis-project` はこれらを
実装済みで、schema version 13・`ProjectController`・`ProjectStore`（HEAD CAS
付き `DirectoryBundleStore`/`InMemoryStore`）・ordered migration ladder・
VCS snapshot/diff/apply/restore を持つ。

本設計はそれらを**再定義せず**、Issue #10 が要求する Project Model 全体の
責務分界・不変条件・失敗契約を一つの正本として統合する。物理 serializer や
schema version は既存実装が確立した契約をそのまま参照し、本書では概念/
スキーマ契約を固定する。

## Scope

### In Scope

- Track/Lane/Clip/Source/Take/Timeline/Control の責務分界と entity 契約。
- Track+Capabilities による Audio/MIDI/Instrument/Bus/Return/Folder 表現。
- 非破壊編集 ops と invariant、TakeGroup/Comp、Branch/Scope/Lineage。
- Time domain 体系と明示的変換規則、PPQN 変更契約。
- Stable ID・`AmbiguousIdentity`・identity 空間の分離。
- 単一 writer・CAS・atomic apply・undo/revision 会計。
- Snapshot/HEAD/Recovery/VCS/migration の永続化契約。
- Remote asset・失敗分類・Ruviform 境界。
- 既存実装（module/test）との対応表。

### Out of Scope

- `crates/synvelis-project` の実装変更・新規 schema version。
- GUI/RT/Plugin 側の第二正本の設計（禁止事項としてのみ記述）。
- 実機・実 plugin・実 storage の検証（別 Gate）。
- Ruviform API の変更。

## Entity 責務分界

| Entity | 責務 | 可変性 | 主な stable ID |
| --- | --- | --- | --- |
| Project | 集約 root。tracks/lanes/clips/sources/assets/takes/comps/control/routing/monitoring/plugin_instances/smf_documents を保持 | command 経由でのみ変更 | `ProjectId` |
| Track | timeline 上の段。capabilities・display order・monitor 宣言を持つ。routing 実体ではない | rename/move で ID 維持 | `TrackId` |
| Lane | track 内の編集面。clip の配置先、comp の owner、automation の owner | 追加/削除は明示 op | `LaneId` |
| Clip | lane 上の配置。source への非破壊参照 + placement/playback mapping | 全編集は clip field の変更のみ | `ClipId` |
| Source (media) | asset が指す media 実体。`SourceFrame` 単位の native grid | 内容は不変。state（verified 等）のみ前進 | `SourceId` |
| MidiSource | project 所有の note 列。`MidiDivision` native grid で `MidiNote` を保持 | note 編集 op で変更（source 単位） | `SourceId`（media と同一空間） |
| Asset | media bytes の provenance/digest/locator を持つ library entity | digest/locator は import 時確定 | `AssetId` |
| Take | recording session の結果。CaptureSegment 群を参照 | `Raw → Verified` の前進のみ | `TakeId` |
| CaptureSegment | session-time ↔ source-time の対応区間 | 登録後不変 | `CaptureSegmentId` |
| RecordingSession/Pass/Stream/ClockEpoch | recording registry | lifecycle 前進のみ | 各専用 ID |
| TakeGroup/Comp/CompRegion/CompTransition | comping registry。candidate を**参照**し所有しない | comp 編集 op | `TakeGroupId`/`CompId`/`CompRegionId`/`CompTransitionId` |
| Note（clip-local）/ MidiNote（source payload） | clip-local は `ControlOwner::Note` 評価 domain、source payload は `(SourceId, NoteId)` で addressing | op 経由 | `NoteId`（両者同一空間・addressing は pair） |
| Connection/ParameterTarget/Automation/Monitoring/PluginInstance | routing/control/monitor/instance 各 registry | op 経由 | 各専用 ID |

### 責務の要点

- **Source/Raw Take/native MIDI は immutable**。`Source`・`MidiSource` の
  payload・`Take`/`CaptureSegment` の記録は編集で書き換えない。編集は常に
  Clip/Comp/Control 側の参照・導出を変更する。
- **Clip は配置と写像のみを持つ**。`Clip` = `{id, lane_id, source_id,
  placement, playback, muted}`。`ClipPlacement` = `{anchor: TimeAnchor,
  extent: ClipExtent}`（保存された権威）で、`ClipPlayback` =
  `{source_region: SourceRange, mapping: PlaybackMapping}`（source 読み
  window と project→source 写像）を分離する。`TimeAnchor`/`ClipExtent` は
  `Musical | Absolute` の混在 domain、`PlaybackMapping` は
  `Natural | Loop | SimpleStretch(StretchRatio)`。source bytes には触れない。
- **TakeGroup/Comp は candidate を参照するだけ**。`CompCandidateRef::Take`/
  `ExistingSource`/`Silence` は raw material を所有・書換えしない
  （`comp.rs` の `CompCandidateRef` が正本）。
- **Timeline は表示・hit-test・gesture の面**であり project truth ではない。
  表示順・zoom・選択は project state を変更しない。

## Track+Capabilities と Template

Audio/MIDI/Instrument/Bus/Return/Folder は別 schema ではなく **Track の
capability 宣言**で表現する:

- Track は kind を固定 enum で持たず、routing port・plugin strip・lane 構成・
  monitor 宣言などの capability で振る舞いを決める。Folder は「子 track を
  表示上まとめる」capability に過ぎず、**routing edge を追加しない**。
  Folder の見かけだけで signal flow が変わってはならない（PM-AC-01）。
- Bus/Return は strip/port 構成（`routing.rs`/`sf_02` 系）で表現し、
  Instrument は processor/plugin strip capability で表現する。
- Routing の実体は `ConnectionId` を持つ接続 entity であり、Track の
  display hierarchy とは独立して検証される（`routing_sf01` の cycle/orphan
  拒否 test が正本）。

## 非破壊編集契約

Clip に対する編集 op はすべて「clip field の変更」であり、source/asset/
take の bytes・content を変更しない:

| 操作 | 変更対象 | source への影響 | ID |
| --- | --- | --- | --- |
| Move | `ClipPlacement`（anchor/extent） | なし | 維持 |
| Trim | `ClipPlayback.source_region` と/or `ClipExtent` | なし（参照窓の変更） | 維持 |
| Slip | `ClipPlayback.source_region` の offset（extent 不変） | なし | 維持 |
| Stretch | `PlaybackMapping::SimpleStretch(StretchRatio)` | なし | 維持 |
| Loop | `PlaybackMapping::Loop`（`source_region` window 内 wrap） | なし | 維持 |
| Split | `ProjectOp::SplitClip` で 2 Clip に分割（`at: MusicalTick` + `clip_b_source_offset`）。**現行 schema に明示 `DerivedFrom` field はない** — lineage は同一 `source_id`・隣接 range の暗黙関係のみ。明示 lineage field は schema 拡張として deferred | なし | 新 ClipId。Undo で元 ClipId 復元。明示 DerivedFrom は gap（下記） |
| Duplicate (Independent) | 新 ClipId・同一 `source_id` 参照 | なし | 新 ClipId |
| Duplicate (Linked/explicit) | 共有 Source 編集が両 Clip に反映 | source payload の編集は source 単位 op | 維持 |
| Fade | `CompTransition.fade_in`/`fade_out`（`FadeShape`）— comp 境界の fade であり clip field ではない | なし | 維持 |
| Mute | `Clip.muted`（`SetClipMuted`） | なし | 維持 |
| Rename | Track/Project 等の display name field（Clip は現行 schema に name field を持たない） | なし | 維持 |

- **Clip gain/fade field は現行 schema に存在しない**。gain は strip/send
  の `ParameterTarget` domain（`send.gain_target`・channel params）、
  fade は `CompTransition` が担う。clip-level gain/fade field の追加は
  別 slice の schema 拡張として deferred とし、本書では既存の
  mute/comp-fade/strip-gain の配置を契約として固定する。
- **Split の明示 `DerivedFrom` lineage field は現行 schema に存在しない**。
  `SplitClip` は 2 つの新 ClipId を発行するが、Clip struct に lineage
  field を持たない。Issue #10 の「DerivedFrom が保存される」要件に対し、
  現行実装は暗黙関係（同一 `source_id`・隣接 range）のみを持つ。
  明示 lineage field の追加は schema 拡張として deferred とし、
  PM-T-09/AC-08 の gap として `evidence/sv10/` に記録する。
- **Loop は `source_region` window 内 wrap** であり、comp の
  `CompCandidateRef::ExistingSource.play_from` とは別概念。
- **Issue 語彙との対応**: Issue #10 の「Musical/Absolute Anchor」は
  `TimeAnchor::{Musical, Absolute}`（+ `ClipExtent` の同名 variant）に、
  「Natural/Musical/Stretch 設定」は anchor/extent の Musical|Absolute
  domain と `PlaybackMapping::{Natural, Loop, SimpleStretch}` に対応する。
  Musical anchor/extent は tempo map 編集後に再評価され、Absolute は
  media time に固定される（`placement.rs` 冒頭契約が正本）。

- **Linked-source semantics**: 複数 Clip が同一 `SourceId`（media または
  `MidiSource`）を参照する場合、source payload の編集は全 linked clip に
  反映される。1 clip だけ独立させるには明示 op
  （`ProjectOp::DetachClipSource` — source を copy して clip を repoint）
  が必要で、暗黙の copy-on-write は存在しない
  （`midi.rs` 冒頭契約・`linked_clips_share_source_edits_and_detach_is_explicit`
  test が正本）。
- **Note addressing**: MIDI note 編集は `(SourceId, NoteId)` pair で
  addressing し、「ある clip 経由」の編集という概念を持たない
  （`midi.rs`）。clip-relative の見かけ上の移動を Delete+Add として
  表現しない。
- **重複の評価規則**: 同一 lane の重複 clip/take/automation は lane の
  明示評価規則に従い、勝手な素材削除・上書きを行わない。comping では
  TakeGroup/Comp の candidate 選択が唯一の「重複解決」機構である。

## TakeGroup/Comp と Effective Projection

- `TakeGroup` は lane 上の comping 単位で、`take_ids`（comparable
  candidate）・`target_range`・`active_comp_id`・`source_rate` を持つ。
- `Comp` は `CompRegion`（非重複 project span + `CompCandidateRef` +
  `PlaybackMapping`）と `CompTransition`（隣接 region 間の bounded
  double-play overlap）で構成される。
- **Adopt は単一の ProjectController CAS+Undo 操作**であり、comp 切替は
  1 revision・1 undo で行われ、raw take/source/asset を変更しない
  （`comp_switch_is_single_cas_undo_and_leaves_raw_immutable`、
  `comp_ops_never_mutate_takes_sources_or_assets` が正本）。
- **Effective Comp** は adopt 済み comp を playback/evaluation 用に
  project→source 区間へ射影した導出物であり、project state ではない。
  射影は deterministic で provenance を保持する
  （`project_comp_is_deterministic_and_preserves_provenance`）。
- comp の region/transition は安定 ID を持ち、save/reload・undo/redo・
  VCS diff/apply/restore をまたいで維持される
  （`comp_ids_survive_undo_redo_save_reload_and_diff`、
  `comp_stable_ids_survive_save_reload_and_diff`、
  `comp_transition_diff_is_expressed_by_its_own_stable_id`、
  `comp_stable_ids_survive_restore_as_copy` が正本）。

## Branch/Scope/Lineage（Creative Branch）

- Branch は typed `VariantScope` と versioned `BranchContentRef` で表現し、
  branch-local entity ID は project 全体の ID 空間と衝突しない scope を持つ。
- duplicate/undo/redo をまたいで branch-local ID が誤再 bind しないこと、
  scope 外の entity は非破壊で維持されることを不変条件とする。
- Adopt（branch 内容の project への反映）は単一 CAS+Undo 操作であり、
  scope 外への silent overwrite を許さない。実装の詳細は #277/#278 系の
  別 slice が Owner であり、本書は契約のみを固定する。

## Time Domain 体系

Project Model は以下の time domain を区別し、変換はすべて**明示的**に行う:

| Domain | 型（実装） | 単位 | 文脈 |
| --- | --- | --- | --- |
| Musical position/duration | `MusicalTick`/`MusicalDuration`（`time.rs`、i64 tick）・`ProjectTime::Musical`・`TimeAnchor::Musical`/`ClipExtent::Musical` | project PPQN tick（既定 960、有効範囲 `1..=32767`） | project PPQN + TempoMap |
| Rational beat | ruviform `BeatPosition`（`control.rs` で使用） | beats（PPQN 非依存の有理数） | Ruviform primitive。Synvelis は tick↔beat を明示変換 |
| Media/absolute | `MediaTimeValue`/`MediaDuration`（有理数）・`ProjectTime::Media`・`TimeAnchor::Absolute`/`ClipExtent::Absolute`・`ProjectRange` | declared time base の media time | TempoMap 経由で musical と対応 |
| Source-native (audio) | `SourceFrame`/`SourceRange` | sample frame @ source rate | media source の native grid |
| Source-native (MIDI) | `MidiTick`/`MidiRange` + `MidiDivision` | source division tick | `MidiSource` の native grid（project PPQN と独立） |
| Recording session | `SessionFrame`/`SessionRange` | session clock frame | `ClockEpoch` 単位 |
| Engine/device | engine frame / device clock | 実行系 frame | RT/graph 層（project model 外） |

規則:

- **異なる PPQN の整数 Tick を暗黙比較しない**。tick は必ず PPQN 文脈
  （`ProjectPpqn`）付きで扱い、変換は rational beat 経由で行う
  （`tempo.rs` の `TickTimeMapping`・`tim_01_ppqn.rs` が正本）。
- **`MediaTimeValue` は有理数**であり、異 base 間の比較は cross-multiplied
  の exact 比較を使う（`range.rs` の `MediaSpan`）。
- **区間は半開 `[start, end)`** が既定契約（`Range<F>`）。`end < start` は
  `Error::Inverted` で構築時拒否。空区間は型上合法だが capture segment 等の
  非空要求 domain は個別に検証する。
- **checked arithmetic 必須**。`ruviform_media_core` の `Tick`/`TickDuration`
  unchecked 演算子は Synvelis の時間演算に直接使わない
  （`AGENTS.md`/`Cargo.toml` workspace 規約）。overflow は `Error::Overflow`
  として型付きで返す。
- **負 time/pre-roll** は musical domain で表現可能（`MusicalTick`/`ProjectTime`
  は負を許容、`id_time_range.rs::musical_tick_and_project_time_allow_negative`
  が正本）。source/session frame は非負（`session_and_source_frames_are_
  checked_and_non_negative`）。

## PPQN 変更・TempoMap 編集契約

- `ProjectPpqn` は `1..=32767`、既定 960。Ruviform 変換には `u16` への
  checked 変換が必要。
- **PPQN 変更は表現の変更であり、暗黙の quantize/stretch/raw event
  retiming ではない**。`SetProjectPpqn` は明示的 conversion policy
  （exact/round 等）を伴い、musical 位置は rational beat で保存され tick
  が再投影される。PCM・raw MIDI bytes・source-native timestamp は変更
  しない（`ppqn_change_preserves_beat_positions_and_reprojects_ticks`、
  `ppqn_change_preserves_absolute_media_and_source_native_data`、
  `ppqn_exact_policy_rejects_inexact_positions` が正本）。
- TempoMap 編集（`AddTempoEvent`/`AddSignatureEvent`）は `TempoMapData`
  の revision を bump し、`TempoMapRef` で参照する。tempo 変更は musical↔
  media の写像を変えるが、source-native data と録音 PCM を変更しない
  （`tempo_map_mutation_bumps_revision_and_refs_point_at_it`、
  `tick_media_mapping_matches_design_example` が正本）。
- PPQN 変更は double CAS（project revision + tempo map revision）を要求し、
  open recording session 中は拒否される
  （`ppqn_change_enforces_double_cas`、`ppqn_change_blocked_by_open_
  recording_session`）。
- Loop 境界・境界の欠落/二重発火は half-open 区間と明示的写像で防ぐ
  （`regression_loop_window_is_the_repeat_unit`、
  `regression_loop_play_from_outside_handle_rejected` が正本）。

## Stable ID と AmbiguousIdentity

- ID 空間は entity kind ごとに独立（`ProjectId`/`TrackId`/`LaneId`/
  `ClipId`/`SourceId`/`AssetId`/`TakeId`/`NoteId`/`ConnectionId`/routing/
  control/monitoring/plugin instance 各 ID）。`id.rs` の tagged ID が正本。
- `id_watermark` は generator の単調 watermark で、save 時に envelope の
  `Project::id_watermark` に刻まれる。load 時に live ID が watermark を
  超えることは拒否される（`load_rejects_id_watermark_below_live_ids`）。
- **rename/move/duplicate で ID 規則**: rename/move は ID 維持。Independent
  duplicate は新 ID。split は新 ClipId + DerivedFrom lineage。undo/redo・
  save/reload・VCS apply/restore・migration をまたいで ID は維持される
  （`save_reload_preserves_identity_ids_and_revision`、
  `comp_ids_survive_undo_redo_save_reload_and_diff`、
  `comp_stable_ids_survive_restore_as_copy` が正本）。
- **`AmbiguousIdentity`**: 同一 number の衝突・owner reference 衝突・
  cross-kind raw collision は `AmbiguousIdentity`/`ComparisonIssue` として
  拒否し、自動 merge しない（`cross_kind_raw_collision_is_ambiguous_
  identity`、`owner_reference_conflict_is_ambiguous_identity`、
  `monitor_decl_on_ambiguous_track_raw_is_ambiguous` が正本）。
- **stable semantic identity ≠ Blob SHA256 ≠ VCS commit OID ≠ Asset
  RemoteLocator**。digest は `AssetDigest` で分類され、content-addressed
  として扱わない（`digest_kind_classifies_persisted_forms`、
  `crc_digest_collision_never_becomes_content_addressed`）。

## 単一 Writer・CAS・Undo・Revision 会計

- **ProjectController が唯一の project truth**。GUI/RT/Plugin は
  command を発行するだけで、独自の project state を持たない
  （`docs/designs/workspace-model-design.md` の WorkspaceModel 設計と整合）。
- **Command 契約**: `Command<ProjectOp>` は `expected_revision` と `op`
  （対象 ID を含む操作）を持つ（`controller.rs`）。`ProjectId` は aggregate
  の identity であり snapshot envelope/store が保持する。apply は
  (1) 全対象の存在・revision・型を検証、(2) scratch copy に適用、
  (3) 成功時のみ atomic swap、(4) 1 gesture = 1 undo unit、の順。
  失敗時は部分適用しない
  （`stale_revision_and_invalid_target_are_rejected_atomically`、
  `apply_advances_revision_and_is_atomic`、
  `comp_group_cas_rejects_stale_revision_without_partial_apply`、
  `comp_invalid_ops_are_all_or_nothing` が正本）。
- **Undo/Redo** は履歴を巻き戻すのではなく新 revision を発行する
  （`undo_redo_issue_new_revisions_and_keep_saved_intact`）。gesture 境界は
  caller が `Command` の grouping で表現し、1 gesture = 1 undo unit。
- **Revision の分離**: live `ProjectRevision`（単調増加）・Saved revision
  （HEAD snapshot が記録）・Recovery revision（crash recovery 用 checkpoint）
  ・VCS revision（snapshot/commit OID）・Execution revision（active graph
  plan）は別物として扱い、混同しない。`save_reload_preserves_identity_ids_
  and_revision`・`recovery_checkpoint_is_isolated_from_saved_head`・
  `live_capture_is_immutable_across_edits_undo_and_save` が正本。
- **Preview は非変更**。UI preview/inspection は project state を変更せず、
  gesture 確定時に 1 transaction/undo unit として commit する
  （`preview_routing_mutates_nothing_and_receipts_are_deterministic`、
  `preview_ppqn_reports_sites_and_confirms_exact_change` が正本）。

## 永続化・Save/Reload・Migration

- **SnapshotEnvelope**: `schema_version`・`project_id`・`revision`・
  `project` を持つ strict（`deny_unknown_fields`）な versioned envelope。
  deterministic serialization（declaration order・`BTreeMap` sorted key）。
  `SCHEMA_VERSION = 13`（`store.rs`）。
- **DirectoryBundleStore**: `<root>/snapshots/<name>` + `<root>/HEAD`。
  snapshot は immutable（`PublishMode::CreateNew`）、HEAD は atomic に
  repoint。`StagedFile → SealedFile → classified publish`（ruviform-io-file）
  で durability を保証する。
- **HEAD CAS（`save_cas`）**: expected-head compare-and-swap。`HEAD.lock`
  create-new mutex + stale-lock breaking（`HEAD_LOCK_STALE_AFTER`=20s、
  `HEAD_LOCK_BUSY_TIMEOUT`=30s）。conflict は `HeadCasError::Conflict`
  として構造化され、汎用 I/O error に偽装しない。`DirectoryBundleStore`/
  `InMemoryStore` が実装し、default は `Unsupported`（`head_cas.rs` 全件が
  正本）。
- **Snapshot collision**: 同名 snapshot で byte-identical または
  `id_watermark` のみ差異なら `Satisfied`/`Refresh`、真に異なる内容は
  `Conflict`（`snapshot_collision`・`same_revision_resave_*`・
  `concurrent_same_name_publish_admits_one_winner` が正本）。
- **Publish honesty**: `NotPublished` は旧 HEAD を保持、`Published`/
  `Indeterminate` は HEAD が既に repoint 済みかもしれないため `head()` で
  readback して reconcile する。`Indeterminate` の leftover temp は
  recovery のため保持し auto-delete しない（`store_publish.rs` 全件が正本）。
- **Reload 検証**: unknown/too-old schema version・corrupt・malformed
  （不正 tempo map・不正 clip contract・MIDI source payload・note ID 衝突・
  foreign key・id_watermark 不足）は load 時に明示拒否する
  （`unknown_schema_and_corrupt_snapshots_are_rejected`、
  `load_rejects_*` 系、`v5_snapshot_projects_read_only_into_current_schema`
  が正本）。
- **Migration ladder**: `migration.rs` が順序付き step registry を持ち、
  v5→v13 を deterministic・read-only projection で読む。source bytes・
  HEAD・assets を書き換えず、次回 save が新 snapshot を publish する。
  各 step は typed error（unsupported schema・validation/not-found）を返す
  （`alib02_v5_real.rs`・`vcs01_snapshot.rs::v5_snapshot_projects_read_only_
  into_current_schema`・`alib02_digest.rs::v5_snapshot_with_crc_digests_
  migrates_losslessly` が正本）。
- **Restore as copy**: VCS restore は verified new copy を publish し、
  既存 destination・unsafe path・low disk・cancel・interrupt を fail-closed
  で扱う（`vcs05_restore.rs`・`vcs06_restore_matrix.rs` が正本）。

## Remote Asset・Offline 再生

- Asset は `AssetId` + digest（`AssetDigest` 分類）+ `RemoteLocator` を持ち、
  media bytes は `Source` が `SourceFrame` native grid で参照する。
- **fetch 失敗/取消**は `AppliedImport` receipt と manifest に不完全データを
  残さない。receipt は live asset/source entity を指し、non-empty digest を
  要求する（`applied_import_receipt_must_name_live_entities`、
  `empty_digest_is_rejected_at_commit_and_load`、
  `alib-07-import-cancel` 系が正本）。
- **取得済み asset は offline で再生可能**。`AssetAvailabilityManifest`/
  `ContentStatus` が present/unknown/missing を分類し、missing blob は
  degraded として明示される（`missing_blob_classifies_degraded_not_
  rejected`、`unprobed_blob_never_reports_present`、
  `restore_as_copy_preserves_*` が正本）。
- Remote provider（Git/SMB/WebDAV 等）は ALIB 系の別 slice が Owner であり、
  本書は project model 側の契約（locator/digest/availability）のみを固定する。

## 失敗・回復分類

| 失敗 | 扱い | 実装根拠 |
| --- | --- | --- |
| stale CAS（expected revision/HEAD） | `HeadCasError::Conflict`/`Error::Validation` で構造化拒否、部分適用なし | `head_cas.rs`・`stale_revision_*` |
| invalid range（`end < start`） | `Error::Inverted` で構築時拒否 | `range.rs::Range::try_new` |
| overflow | `Error::Overflow`（checked arithmetic） | `range.rs`・`time.rs`・`ppqn_change_rejects_tick_overflow` |
| missing Source/Asset | `MissingAssetsPolicy`/`ContentStatus` で degraded/unknown として明示、silent fallback なし | `restore/mod.rs`・`snapshot/types.rs` |
| corrupt data（snapshot/blob/manifest） | typed error で拒否、destination/HEAD 不変 | `store_publish.rs`・`vcs06_restore_matrix.rs` |
| unsupported mapping/schema | `Error::Validation`/`Unsupported` で明示拒否 | `migration.rs`・`HeadCasError::Unsupported` |
| incomplete remote fetch | receipt/manifest に不完全データを残さない | `alib02_digest.rs`・`alib-07` |
| malformed snapshot | load 時に `deny_unknown_fields` + domain validate で拒否 | `store.rs`・`load_rejects_*` |
| migration failure | ordered step の typed error、read-only のため原本不変 | `migration.rs` |
| cancel | `SnapshotCancel`/`RestoreRequest` で phase boundary 明示失敗 | `vcs01_snapshot.rs::cancelled_token_*`・`vcs05_restore.rs::cancelled_restore_*` |

## Ruviform / Synvelis 境界

- **Synvelis が所有**: DAW semantics、project schema（Track/Lane/Clip/
  Source/Take/Time/Control）、assets/takes/comps、editing commands/undo、
  recording、serializer、visual diff、placement/playback policy。
- **Ruviform が提供**: generic musical time・`TempoMap`・`MediaTime`・
  generic timeline 座標/hit-test・plugin event delivery・audio RT processing。
- **禁止**: Ruviform 共通 Time/Media IO・Graph API に DAW 固有の
  Track/Clip schema 型を流入させない。`ruviform_media_core` の `Tick`/
  `TickDuration` unchecked 演算子を Synvelis の時間演算に直接使わない。
- Project state の変更と execution graph の prepare/publish は分離し、
  graph 更新は非 RT で行い RT へ安全に publish する。project revision と
  execution revision を混同しない。

## 既存実装との対応

| 設計項目 | 実装 module | 代表 test |
| --- | --- | --- |
| Entity schema/ID | `project.rs`/`id.rs`/`ops.rs` | `id_time_range.rs`・`controller_store.rs` |
| Track/Lane/Clip/Source | `project.rs`/`placement.rs` | `controller_store.rs`・`vcs02_diff.rs` |
| MIDI source/note | `midi.rs` | `midi_source.rs`・`tim_02_smf.rs`・`rcd_04_midi_phrase.rs` |
| Time domain/PPQN/TempoMap | `time.rs`/`tempo.rs` | `tim_01_ppqn.rs`・`tempo.rs`・`tim_02_ppqn_smf.rs` |
| Comp/TakeGroup | `comp.rs`/`recording_data.rs` | `md_a.rs`・`rcd_04_comp_cas.rs` |
| Routing/Control/Monitoring/Plugin | `routing.rs`/`control*`/`monitoring*`/`plugin*` | `routing_sf01.rs`・`sf_02.rs`・`ctl_01.rs`/`ctl_02.rs`・`aut_02.rs`・`monitoring_persist.rs`・`plugin_instance_persist.rs`・`plugin_state.rs` |
| Single writer/CAS/Undo | `controller.rs` | `controller_store.rs`・`head_cas.rs`・`store_publish.rs` |
| Snapshot/Store/Migration | `store.rs`/`migration.rs`/`snapshot/`/`restore/` | `store_publish.rs`・`vcs01_snapshot.rs`・`alib02_v5_real.rs`・`alib02_digest.rs` |
| VCS diff/apply/restore | `diff/`/`vcs/`/`apply/` | `vcs02_diff.rs`・`vcs05_apply.rs`・`vcs05_restore.rs`・`vcs06_*`・`rcd_04_vcs_stable_id.rs` |
| Recording session/take | `recording_data.rs` | `md_a.rs`・`rcd_04_*` |

## 検証写像

`docs/test-specs/project-model.md` の PM-T case と `evidence/sv10/` の
case-ledger に、PM-AC-01〜13 の各受入条件を既存 test・設計レビュー・
NotRun/Deferred に写像する。本 slice は `crates/synvelis-project` を変更
しないため、新規 test の追加は行わず、既存 fixture の照合結果を記録する。

## 未決事項・Deferred

- Creative Branch の完全な schema/UI は #277/#278 系の別 slice。
- Execution Graph との統合詳細は #18 系の別 slice（本書は revision 分離
  のみ固定）。
- 実 GUI/実 plugin/実 RT/実 storage の受入は #112/#114/#176/#294/#84/#330
  系の別 Gate。
- Remote provider の fetch/cancel 実装詳細は ALIB 系の別 slice。
