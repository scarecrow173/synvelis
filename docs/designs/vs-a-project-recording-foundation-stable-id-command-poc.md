---
id: "DESIGN-034QdO7AB6cp4DRgKeuuEn"
type: "design"
status: "approved"
title: "VS-A Project/Recording Foundation — Stable ID・時間型・Command・永続化 PoC"
created: "2026-11-01"
updated: "2026-11-01"
owners: []
relations:
  source: []
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
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/13"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/19"
  defers: []
  deferred-by: []
---

# VS-A Project/Recording Foundation — Stable ID・時間型・Command・永続化 PoC

## Context

Synvelis には Cargo Workspace と Project モデル実装が存在しない。
`project-asset-persistence-architecture.md`（#17 系設計正本）・録音設計（#12）・
可変 PPQN 方向（#19）が定義済みであり、VS-A はそれらを最小構成で
コード化する F0 Gate。本設計は spec「VS-A Project / Recording Foundation」
（approved）と ADR「VS-A Workspace構成・Ruviform git依存・永続化PoC・
Stable ID生成」（accepted）を実装可能な形に落としたもので、
後続の VS-B〜D・E2E-L/R/X が共有する単一の Project 基盤を固定する。

## Scope

### In Scope

- 3 クレート構成の Cargo Workspace 新設と mise 検証タスク。
- Stable ID 群・時間型・半開区間・TempoMap 参照の型定義と検証。
- Recording Session/Pass/Stream/Segment/Take の参照モデルと状態機械。
- `ProjectController`（Command・ExpectedRevision・原子的 Mutation・
  Undo/Redo・Live/Saved Revision 分離）。
- Directory Bundle 簡略実装による Save/Reload と SchemaVersion Reject。
- Verified Source 参照 Port と `recording-poc` 決定的シナリオ。

### Out of Scope

- 実 Device/GUI/Plugin、RT Capture、Media Writer、Gate 本体、Comp 詳細。
- Journal/Recovery、VCS、GC/Retention の実装（参照保持の記録のみ）。
- 永続化形式の最終公開フォーマット確定、Project 間素材移動の ID 再採番。

## Components and Boundaries

### クレート構成（依存は一方向: poc → recording → project）

| クレート | 責務 | 依存 |
| --- | --- | --- |
| `synvelis-project` | Stable ID・時間型・区間・TempoMap 参照・Project 集約（**永続対象の全データ = authoring state + Recording Registry の POD データ**）・`Operation` trait・`Command`/`ProjectController`（generic）・Revision・Verified Source 参照値・Store Port trait | `serde`, `serde_json`, `ruviform-media-core` |
| `synvelis-recording` | Recording ドメインの**振る舞い**（Session 状態遷移ルール・参照検証・`RecordingOp` 定義と `Operation` 実装・`SynvelisOp` sum type） | `synvelis-project` |
| `recording-poc` | 決定的 headless シナリオ実行バイナリ（Project 構築→Recording 登録→Save→Reload→比較） | 上記 2 クレート |

**Command/Storage 縫合（設計確定点）**: Rust の enum variant は定義
クレートに閉じるため、`Command` を generic `Command<O>` + `trait Operation`
（`fn validate(&self, &Project)->Result<()>` / `fn apply(&self, &mut
Project)->Result<()>`）として `synvelis-project` に定義し、
`ProjectController<O: Operation>` とする。Recording Session/Pass/Stream/
Segment/Take・`SessionState`・`RecordingRegistry`・`ClockEpoch` は
**永続対象の Project 集約データとして `synvelis-project` 側の POD 型**
（#17 で Recording Registry は Project Snapshot の一部）。`RecordingOp` は
`synvelis-recording` で定義し `Operation` を実装、`SynvelisOp{Project(ProjectOp),
Recording(RecordingOp)}` も `synvelis-recording` が提供する sum type で
`Operation` を dispatch 実装する。これで Single Writer・単一 Snapshot・
Undo 網羅・acyclic を同時に満たす。

### `synvelis-project` 内部モジュール

- `id.rs`: `macro` で生成する Phantom タグ付き newtype。
  `ProjectId/TrackId/LaneId/ClipId/SourceId/AssetId/TakeId/
  RecordingSessionId/RecordingPassId/CaptureStreamId/CaptureSegmentId/
  ClockEpochId`。`u128` 値保持、serde は `"<kind>-<32hex>"` の
  タグ付き文字列で de/serialize（kind 不一致はエラー）。**型混同は
  `compile_fail` doctest で検証**（spec 要件で検証方法固定）。
  `IdGenerator` trait + `CounterIdGenerator`（`next() -> u128`）は
  `ProjectController` 生成時に注入し、controller が所有する。
- `time.rs`: `SessionFrame(i64)`・`SourceFrame(i64)`（内部 `i64`、構築は
  `i64` 入力で `>= 0` 検証、演算は `i64::checked_add/sub/mul` のみ、
  `Negative`/`Overflow` エラー。実効ドメイン `[0, i64::MAX]` を型契約とする）。
  `MusicalTick(i64)`（ruviform `Tick` ではなく Synvelis 固有型・負値許容）。
  `ProjectTime` は `Musical(MusicalTick)` / `Media(MediaTimeValue)` の enum。
  `MediaTimeValue` は Ruviform `MediaTime` の Synvelis 側 serde DTO
  `{value: i64, time_base_num: i64, time_base_den: i64}`（ruviform 側に
  serde impl が無いため）。Ruviform `Tick`/`TickDuration` の `Add`/`Sub`
  は直接呼ばず、Synvelis `checked_*` API 経由のみ。
- `range.rs`: `Range<F>{start: F, end: F}` generic `[start,end)`。
  `SessionRange = Range<SessionFrame>` / `SourceRange = Range<SourceFrame>` /
  `MusicalRange = Range<MusicalTick>` / `MediaSpan{start,end: MediaTimeValue}`
  （ruviform `MediaRange` は start+duration・未検証・saturating end・serde 無し
  のため**使わない**、名称も衝突回避）。構築時 `end >= start` 検証
  （`Inverted` エラー）、`is_empty`/`contains`/`intersection`。
  空区間は型上許容、ドメイン側が非空を要求する場合は別途検証。
- `tempo.rs`: `TempoMapData{revision: u64, ppqn: u32, tempo_events:
  Vec<TempoEventDto>, signature_events: Vec<SignatureDto>}` を Synvelis
  所有の serde 型として永続化（ruviform `TempoMap` は private fields・
  列挙 API・serde 無しで serialize 不能のため）。Load 時に
  `TempoMapBuilder` で Ruviform `TempoMap` を再構築し、`tick ↔ MediaTime`
  変換 `TickTimeMapping` を派生。`TempoMapRef{revision: u64}` は
  `Project.tempo_map.revision` を指す参照値（Snapshot 参照値型）。
  `ProjectPpqn(u32)`（既定 960。ruviform `Ppqn(u16)` との名称衝突を回避）。
- `project.rs`: `Project{project_id, name, ppqn: ProjectPpqn, tempo_map:
  TempoMapData, id_watermark: u128, tracks, lanes, clips, assets, sources,
  clock_epochs, recording: RecordingRegistry}`。
  `Track{id, name, lane_ids: Vec<LaneId>}`、`Lane{id, track_id,
  clip_ids: Vec<ClipId>}`、`Clip{id, lane_id, source_id, anchor: TimeAnchor,
  source_offset: SourceFrame}`（`TimeAnchor = Musical(MusicalRange) |
  Media(MediaSpan)`）。`Asset{id, digest}`、`Source{id, asset_id,
  content_digest, state: SourceState}`（Source 自体が Verified Source 参照値;
  `SourceState` = `Imported | Verified | Committed | Saved` の直和。
  遷移は前進のみ `Imported→Verified→Committed→Saved`、`content_digest`
  が将来の CommitIdentity キャリア。Media bytes は含まない）。
  `RecordingRegistry{sessions, passes, streams, segments, takes}` と
  `ClockEpoch{id, description}` の `BTreeMap` 群を保持。
- `controller.rs`: `trait Operation{validate,apply}`、`Command<O>
  {expected_revision, op: O}`、`ProjectController<O: Operation>{live:
  Project, live_revision: ProjectRevision(u64), generator: Box<dyn
  IdGenerator>, undo: Vec<Project>, redo: Vec<Project>, saved:
  Option<SavedState>}`（`SavedState{revision: ProjectRevision, head: String}`;
  SavedRevision は保存時点の `ProjectRevision` の別名として記録）。
  `apply` は ExpectedRevision 照合 → `op.validate` → `op.apply` →
  `live_revision += 1` → 旧 snapshot を undo へ push・redo クリア。
  `undo()/redo()` は内容のみ差替え、`live_revision` は新規発行
  （`prev + 1`）、Saved は不変、generator は snapshot の
  `id_watermark` で再 seed。空 stack は `Ok(false)`（no-op）で返す。
  `save()` は generator の watermark を snapshot に書き、`saved` を更新。
  `load()` 成功時: `live_revision = 保存時 revision`、`saved = Some(..)`、
  **undo/redo stack はクリア**（新セッション境界）。
- `ops.rs` (`ProjectOp`): `SetProjectName`/`AddTrack`/`RemoveTrack`/`AddLane`/
  `RemoveLane`/`AddClip`/`RemoveClip`/`MoveClip`/`RegisterAsset`/
  `RegisterSource`/`SetSourceState`/`RegisterClockEpoch`/`AddTempoEvent`/
  `SetSignature`/`SetPpqn`（`SetSourceState` が Verified Source 参照 Port の
  遷移口）。全 variant は対象存在・参照・遷移合法性を検証。
- `recording_data.rs`（project crate 内 POD）: `RecordingSession{id, name,
  state: SessionState}`、`RecordingPass{id, session_id, index}`、
  `CaptureStream{id, session_id, lane_id, clock_epoch_id}`、
  `CaptureSegment{id, stream_id, pass_id, session_range: SessionRange,
  source_range: SourceRange, source_id}`（両 range 非空を登録検証で要求）、
  `Take{id, session_id, pass_id, stream_id, lane_id, source_id,
  segment_ids: Vec<CaptureSegmentId>`（非空必須）`, state: TakeState}`。
  `SessionState = Created | Open | Closed | Finalized`、
  `TakeState = Raw | Verified`。
- `store.rs`: `ProjectStore` trait（`write_head`/`read_head`/`publish`/
  `read_snapshot`）。`DirectoryBundleStore`（FS 実装: temp→sync→
  `snapshots/r<revision>.json` immutable publish→HEAD 更新を rename で
  実施）と `InMemoryStore`（**canonical JSON bytes を保持**し round-trip が
  実際の encoding を経る）。`SnapshotEnvelope{schema_version: u32,
  project_id, revision, project: Project}` は serde `deny_unknown_fields`
  で strict 化し、未知必須要素・schema 不一致・parse 失敗を明示 Reject。
  `recording-poc` は `DirectoryBundleStore` を temp dir で使い、
  Save プロトコル自体を決定的に検証する。

### `synvelis-recording` 内部モジュール

- `ops.rs`: `RecordingOp{CreateSession{name}, OpenSession, CloseSession,
  FinalizeSession, RegisterPass{index}, RegisterStream{lane_id,
  clock_epoch_id}, RegisterSegment{...}, RegisterTake{...}, MarkTakeVerified}`
  と `impl Operation`（`validate`/`apply`）。
  `SynvelisOp{Project(ProjectOp), Recording(RecordingOp)}` もここで定義。
- `session.rs`: `SessionState` 遷移表（`Created→Open→Closed→Finalized`
  のみ合法、逆/飛び級は `IllegalTransition`）。`CreateSession` が
  `Created` 状態の Session を生成（これが `Created` への唯一の入口。
  Session `name` は P0 では変更不可・再 rename command は持たない）。
  `OpenSession`=Created→Open、`CloseSession`=Open→Closed、
  `FinalizeSession`=Closed→Finalized。登録系 Command
  （`RegisterPass`/`RegisterStream`/`RegisterSegment`/`RegisterTake`）は
  **`Open` 状態のみ**受理（`Created`/`Closed`/`Finalized` 拒否）。
  `MarkTakeVerified` は `Open|Closed` で受理（`Raw→Verified`、
  Finalized では拒否）。
- `validate.rs`: 登録前の全参照検証。「順序違反」= Session 非 Open・
  依存 entity 未登録（Segment 登録時に pass/stream が無い、Take 登録時に
  segment が無い等）・session 所属不整合・pass index 重複。
  Temporal containment（segment ⊆ pass extent）は P0 範囲外・Deferred。

## Data and Control Flow

1. `ProjectController::<O>::new(project, generator)` で
   `live_revision = ProjectRevision(0)`、undo/redo 空、saved なし。
2. `apply(Command{expected_revision, op})`:
   a. `expected_revision == live_revision` でなければ `StaleRevision`。
   b. `op.validate(&live)` で対象 ID・参照・状態を**全件検証**
      （失敗時は一切変更しない＝原子的）。
   c. `op.apply(&mut live)` → `live_revision += 1` → 変更前 snapshot を
      undo へ push・redo をクリア。
3. `undo()`: undo から内容を pop → Live Project へ適用 → 現内容を redo へ
   push → **`live_revision += 1`（新規発行・巻戻さない）** → generator を
   復元 snapshot の `id_watermark` で再 seed。Saved HEAD/SavedRevision
   不変。空 stack は `Ok(false)`。redo() は対称。
4. `save()`: `live.id_watermark = generator.peek()` を snapshot に反映 →
   `SnapshotEnvelope{schema_version=1, project_id, revision, project}`
   canonical JSON → temp 書出し → flush/sync → `snapshots/r<revision>.json`
   へ immutable publish（revision で決定的命名）→ `HEAD` を同一 FS 内
   rename で更新 → `saved = SavedState{revision, head}`。失敗時は旧
   HEAD・旧 Snapshot を保持。
5. `load()`: `HEAD` 参照 → snapshot bytes parse（`deny_unknown_fields`）→
   `schema_version` 照合（未対応は `UnsupportedSchema` Reject）→
   参照検証 → 成功時のみ `live=project, live_revision=revision,
   saved=Some(..)`、undo/redo クリア、generator を `id_watermark` で
   seed。失敗時は従前状態を保持。
6. `recording-poc`: `DirectoryBundleStore`（固定相対 path `./poc-out/`）を
   使い、Project+Session 構築→Command 適用→Save→Reload→
   `serde_json::to_value` 比較で同一性確認、結果サマリを stdout へ
   決定的出力。

## Non-Functional Constraints

- 全時間/区間/フレーム演算は `checked_*` のみ（panic/wraparound 禁止）。
- PoC は乱数・時刻・実デバイスに依存しない（決定的 ID は注入済み
  カウンタの初期値のみ）。
- `serde_json` による正規 JSON（キー順は `BTreeMap` で固定）で
  同一 Snapshot が同一 Bytes を再現。
- Media Blob bytes は一切 Project Snapshot に含めない。
- MSRV 1.89 / toolchain 1.99.0 / edition 2024 / resolver 2（Ruviform 照合）。

## Risks and Trade-offs

- Ruviform `Tick` 直接演算は unchecked → Synvelis checked wrapper で包む
  （Ruviform 側 checked API 不足は別 Issue で追跡・恒久重複実装にしない）。
- Snapshot Undo はメモリを履歴分消費 → P0 では正しさ優先、履歴上限・
  構造共有は後続最適化。
- カウンタ ID は単一 Project 内のみ一意 → Project 間移動の再採番は
  Deferred（Import/移行機能で対応）。
- Undo で `id_watermark` が戻り ID 再採番が起き得る → 同一 Revision 上の
  同時存在はないため P0 許容、VCS/差分機能で再評価（ADR Consequences）。

## Deferred Design Concerns

- Physical Serializer 最終形式・Journal/Recovery・VCS・GC 実装・
  TakeGroup/Comp 詳細・Project 間素材移動・Read-only Degraded/
  Migration 経路（SchemaVersion Reject を既定として後続 Issue）。
- 有理数 Beat Position 正本化（#19/Ruviform #189）— 現 PoC は
  `tick + ppqn` コンテキスト保存の 960 固定。

## Verification Notes

- [ ] `mise.toml` に `fmt`/`fmt-check`/`check`/`lint`/`test`/`build`/
  `test-msrv` を定義し `mise run fmt-check`/`check`/`lint`/`test`/`build`
  が全緑。
- [ ] `compile_fail` doctest で ID 型混同を検証（spec 固定の検証方法）。
- [ ] 全 AC を網羅するテスト（test-spec で AC→test 対応を固定）。
- [ ] `recording-poc` 2 回実行で同一出力。
- [ ] `mise run test-msrv`（rust 1.89、toolchain 取得可能な環境で）。
