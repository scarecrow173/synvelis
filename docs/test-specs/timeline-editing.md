---
id: "TSPEC-vJFdbqSGXmBCZDnTy2Telx"
type: "test-spec"
status: "approved"
title: "Timeline Editing / Selection / Tool System — テスト仕様・受入 Fixture（synvelis#11）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/11"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/timeline-editing.md"
    - "docs/designs/timeline-editing-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/timeline-editing.md"
    - "docs/designs/timeline-editing-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "evidence/sv11/coverage.md"
    - "docs/plans/timeline-editing.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/11"
  derived-by: []
  verifies:
    - "docs/specs/timeline-editing.md"
    - "docs/designs/timeline-editing-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/243"
  deferred-by: []
---

# Timeline Editing / Selection / Tool System — テスト仕様・受入 Fixture

## 目的

spec の TE-01〜14 と design の画面操作 Fixture（F-UX-01〜08）を、
検証可能な **Feature / Rule / Example / Guarantee** 構造で固定する。
本 slice は docs-only のため、各 Fixture は「既存 headless 証拠の
写像」または「後続実装/Gate が満たすべき契約」として定義し、未実装
の設計動作を `Pass` として主張しない。被覆の現在値は
`evidence/sv11/coverage.md` を正本とする。

## 検証レベルと状態語彙

POLICIES.md の規範に従う:

- `Pass (headless)`: 既存 headless/unit テストが契約を実証済み。
- `Partial`: 一部経路・一部状態のみ実証済み。
- `NotRun`: 設計契約はあるが本 slice で未検証。
- `Deferred`: GUI/実機/実 Plugin 等の後続 Gate に委譲。
- `Unsupported`: 当該 capability を提供しないことを明示。

Requirement 状態（`Required`/`Deferred`/`NotApplicable`）と
実行結果（`Pass`/`Fail`/`NotRun`/`Unsupported`）は混同しない。
Pixel/OS 入力の合格と Headless の合格を混同しない。

## Features

### Feature TE-F1 Tool 解決と Gesture 固定（TE-01/TE-02）

Smart Tool が Hit Test 優先順で操作を解決し、Gesture 開始後は操作
種・対象を固定する。曖昧な hit は候補と推奨 Tool を返す。

- **Rule R1**: Smart Tool の解決優先順は 明示 Handle → 編集対象 →
  Clip Body → Lane Empty → View Background。Dedicated Tool は明示
  固定で Smart 解決を通らない。
- **Rule R2**: Gesture 開始（Pointer Down/HitTest/Resolve）後は
  fixation の tool/operation/targets/primary/revision が不変。
  Pointer が別 zone へ移動しても操作種・対象は変わらない。
- **Rule R3**: 曖昧な hit（極小 Clip・Handle 重複・高密度 Note）は
  `Ambiguous` + 候補 + 推奨 Dedicated Tool を返し、黙って丸めない。
- **Example E1**: Smart Tool で Clip Body を Drag 中に端 Handle 領域
  へ Pointer が入っても Move のまま commit される。
- **Guarantee G1**: `ed_a_02_tool_resolution_and_fixation` の
  headless 契約を満たす（`Pass (headless)`）。実 px Hit Size・
  Cursor/Tooltip・Pointer Device 差は `Deferred`（#112/#114）。

### Feature TE-F2 Selection 両立と Delete 対象（TE-03）

Object Selection・Time Selection・Lane Scope が View ごとに独立し、
Time Selection 単独では破壊的編集を起こさない。

- **Rule R4**: Object/Time Selection は並存するが、Delete 等の
  破壊的 Command は明示的 Object Selection のみを対象とする。
- **Rule R5**: Time Selection 由来の range 削除は独立 Command
  （`clips.delete-range` 相当）で Lane Scope を明示して発行する。
- **Rule R6**: Domain 別 Selection を保持し、domain 非対応 Command
  は `Unresolved` で拒否する。混在 domain の範囲選択は無効化または
  明示 Feedback。
- **Example E2**: Time Selection 設定中に Clip を選択して Delete →
  Clip のみ削除、ripple なし。Time Selection のみで Delete →
  `clip.remove` は Unresolved。
- **Guarantee G2**: `ed_a_01_selection_model_separation`・
  `ed_a_01b_time_selection_and_range_delete` で `Pass (headless)`。
  実画面の Menu/Feedback・OS 別 modifier は `Deferred`（#112/#114）。

### Feature TE-F3 複数対象 Move と 1 Undo（TE-04）

一回の Gesture の多対象編集は全件検証・Atomic Commit・1 Undo で
全件復旧する。

- **Rule R7**: Gesture commit は `ProjectOp::Composite` 相当の単一
  atomic apply = 1 revision = 1 undo。全件検証失敗時は正本不変 +
  Reason 通知。
- **Rule R8**: Preview/Drag の途中経過は Project revision・Undo・
  engine graph を進めない。
- **Rule R9**: Undo/Redo は session engine hook 経由で配送され、
  graph_builds が進む。Text/IME 文脈の undo は text lane、
  PluginGui/Modal は suppressed。
- **Example E3**: 5 Clip を選択して Drag→Release→Undo 1 回で全件
  が元位置へ戻る。ghost target 混入で commit 全体が拒否され部分
  適用が残らない。
- **Guarantee G3**: `ed_a_04_preview_atomic_undo`・
  `ed_b_01_media_multi_move_is_one_undo`・
  `controller_store.rs`（atomic apply/stale revision/undo-redo）で
  `Pass (headless)`。実 Drag/Undo UX は `Deferred`（#114）。

### Feature TE-F4 時間 Domain 分離（TE-05）

Musical/Absolute/Source-local/Owner-Lane の時間 Domain を明示し、
暗黙変換を行わない。

- **Rule R10**: 時間 Domain は対象意味で決定（Musical Anchor→
  Musical Time、Absolute Anchor→Media Time、Source 内 Audio→
  native sample、Automation Point→Owner Lane Domain）。
- **Rule R11**: probe/gesture の domain 不整合は開始時に拒否。
  未宣言変換を要する操作は `UnsupportedOperation`/
  `MixedTimeDomains` で拒否。ED-B の定義済み結果表（move=anchor
  domain commit・trim=拒否・slip=定義済み変換/拒否）のみ許容。
- **Rule R12**: Mixed Domain 一斉移動は共通 Domain/基準の明示なしに
  拒否。Snap 候補は gesture の time domain で filter する。
- **Example E4**: Musical+Absolute 混在 clip の Move は anchor
  domain で commit、Trim は定義済み拒否、Musical anchor×Absolute
  extent の Slip は tempo map 経由の定義済み変換で commit。
- **Guarantee G4**: `ed_a_03_snap_grid_time_domain`・
  `ed_b_04_*`・`ed_b_06_domain_mismatch_rejects_at_start`・
  `ed_b_07_failed_media_commit_is_atomic` で `Pass (headless)`。

### Feature TE-F5 Grid/Snap 独立性と Snap Guide（TE-06/TE-07）

Grid 表示と Snap 設定は独立し、明示 Snap Unit は Zoom で不変。
Snap Guide は採用候補と時間を説明し、Hysteresis で振動しない。

- **Rule R13**: `GridConfig.visible/unit` と `SnapConfig.enabled/
  unit/targets/mode/threshold/hysteresis` は別設定。Zoom は明示
  `SnapUnit::Explicit` を変えず、`Auto` のみ追従する。
- **Rule R14**: Snap 候補は対象・domain で絞り、距離+明示
  Priority+Hysteresis で採用。`SnapWinner` が採用候補・位置・
  label を返す（「何に snap したか」を説明可能）。
- **Rule R15**: Clip/Note 編集の Snap と既存イベントへの Quantize
  は別操作。PPQN 正本を破壊する強制丸めをしない。
- **Example E5**: Grid 非表示で Snap Enabled のまま Marker へ
  Clip が吸着し winner=Marker を label で返す。微小 Pointer 移動で
  winner が hysteresis 内なら保持される。
- **Guarantee G5**: `ed_a_03_snap_grid_time_domain` で `Pass
  (headless)`。Snap Guide の実描画・視認性・Threshold 数値は
  `Deferred`（#112/#114・Issue #11 §9）。

### Feature TE-F6 Gesture Cancel・競合拒否（TE-08/TE-09）

Cancel 経路と外部変更で Project を変更せず、誤 Target/Revision
へ適用しない。

- **Rule R16**: `CancelReason = Esc|PointerCancel|WindowClosed|
  DeviceLost|TargetDeleted|InputScopeLost` で未確定 Preview を
  破棄し Project 不変。
- **Rule R17**: commit は fixation の `project_revision` を
  `expected_revision` として検証し、外部変更を `StaleRevision` で
  拒否。自動 rebase・target すり替えをしない。
- **Rule R18**: Window/View 破棄・Detach は該当 View の gesture を
  破棄する（WindowClosed 相当）。
- **Example E6**: Drag 中に別 Window で対象 Clip を削除→Release →
  拒否され project 不変。Drag 中に `SetProjectName` で revision
  前進→`StaleRevision`。
- **Guarantee G6**: `ed_a_04_preview_atomic_undo`（cancel/stale/
  window destroy）・`ws_a_03_command_routing`・
  `uic5_02_commit_drops_on_stale_revision`・
  `uic5_04_foreign_change_conflicts_then_explicit_rebase_pending`・
  `ed_a_05`（link event が fixation を変えない）で `Pass
  (headless)`。実 Window 破棄・2 Window 実競合・実 Esc/PointerCancel
  入力は `Deferred`（#114）。

### Feature TE-F7 Cross-View・Linked Source・Read-only View（TE-10/11/12）

Linked Context は非破壊 navigation、EditBinding は command target
に解決せず、Read-only View の選択は Live Project を変更しない。

- **Rule R19**: `EditBinding { clip, source }` は表示 binding で
  あり Selection/command target/linked_context を変更しない。
  Gesture fixation は link event・Follow 更新・他 View の
  Selection で不変。
- **Rule R20**: Linked MIDI Source の編集は共有先へ反映するが、
  Arrangement の object selection を変更しない。Independent Copy
  は明示 `DetachClipSource` のみ。
- **Rule R21**: Inspector/Signal Flow/Version Diff の
  Explore/Linked Highlight/選択は Editor の Edit Command 対象・
  Selection・Project を変更しない。VCS 過去 Snapshot の Apply は
  #7 の明示 Command のみ。
- **Example E7**: Piano Roll で Note を commit → 共有 Source 更新・
  Arrangement selection 不変。VCS History で過去 commit を選択 →
  project/engine/transport/undo 不変。
- **Guarantee G7**: `midi_source.rs::linked_clips_share_source_
  edits_and_detach_is_explicit`・`midi_source.rs::note_edits_
  reject_stale_revision_bad_target_and_out_of_range`・`ed_a_05_
  linked_context_separation`・`uic5_02_inspect_is_not_edit`・
  `uic5_03_*`・`vcs04_history.rs::view_mode_and_clear_selection_
  are_display_only`/`vcs_ops_never_touch_project_engine_or_
  transport` で `Pass (headless)`。実 Piano Roll/VCS Diff UI は
  `Deferred`（#112/#114）。

### Feature TE-F8 Remote Asset・RT 境界（TE-13/TE-14）

Remote Asset の Drag&Drop は Fetch+Validate 完了まで Clip/Manifest
を確定せず、失敗時に部分成果を残さない。再生中の Preview/履歴
操作は RT Audio Callback をブロックしない。

- **Rule R22**: Remote Asset import は Fetch→Verify→Publish→
  Commit の順で、検証失敗・publish 失敗・target 不正では Clip/
  Manifest を残さず理由を通知する（`synvelis-library` import
  pipeline 契約）。
- **Rule R23**: Gesture Preview・Undo/Redo・View 状態操作は非 RT
  の View/Workspace/ProjectController 側で行い、RT Audio Callback
  をブロックする処理を載せない。
- **Rule R24**: Project 変更確定と非 RT の実行 Plan 公開は別段階
  であり、反映待ち・失敗を誤魔化さない。
- **Example E8**: import 中に digest mismatch → job/clip が残らず
  resume 可能。再生中に layout churn + undo → transport/revision/
  graph_builds/undo stack が意図せず変化しない。
- **Guarantee G8**: `synvelis-library/tests/import.rs`（verify/
  publish/digest mismatch/publish failure resume/missing lane・
  wrong track reject）・`import_recovery.rs`・
  `workspace_model.rs::playback_dock_window_churn_preserves_
  transport_revision_and_history`・`ws_a_05_layout_ops_leave_
  project_and_engine_alone` で `Pass (headless)`。実 OS
  Drag&Drop・実 RT glitch/alloc 計測は `Deferred`（#114/EXT-L3）。

## Non-goals（テスト対象外）

- 実 Window/OS GUI・実 device・実 plugin を要する product E2E —
  #112/#114・EXT-L3 の別受入 Gate（`Deferred`）。
- Smart Tool の px Hit Size/Drag Threshold・OS 別 keybind・Snap
  数値 — Issue #11 §9 の PoC 後決定事項。
- Take/Comp Editor の UI 詳細 — #12 §14/§15 の後続設計。
- Ruviform GPUI Driver/共通 Gesture primitive の再実装 — Ruviform
  責務。
- Pointer/Selection の暗黙 Project/Graph Commit 化 — 禁止事項。

## Risk

- **被覆の過大主張**: headless 実装済みを製品 Pass と誤記する
  危険。→ `evidence/sv11/coverage.md` で TE-AC 毎に根拠テストを
  列挙し、無いものは `NotRun`/`Deferred`/`Unsupported`。
- **語彙の分岐**: 新しい selection/tool/snap 状態名を発明すると
  既存 `ObjectSelections`/`TimeSelection`/`LaneScope`/`EditBinding`/
  `GestureFixation`/`SnapWinner`/`CancelReason` と不整合。
  → 既存型名・ED-A/ED-B 契約名のみ使用。
- **Fixture と実装の乖離**: F-UX-* が実装と異なる操作を仮定。
  → 各 Fixture/Guarantee に対応する既存テスト名を併記し、差異は
  coverage の `Partial`/`Deferred` で明示。
