---
id: "DESIGN-034QAlPiE7atUoGKXNG7Lr"
type: "design"
status: "approved"
title: "ED-A Editing Model 設計 — Selection/Tool/Snap/Gesture/LinkedContext headless PoC"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/38"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md"
    - "docs/designs/ws-a-workspace-foundation-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/designs/ws-a-workspace-foundation-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/190"
  defers: []
  deferred-by: []
---

# ED-A Editing Model 設計 — Selection/Tool/Snap/Gesture/LinkedContext headless PoC

## Context

Issue #38: #11 合意 5 項目を `synvelis-workspace` + `synvelis-project` 上に
headless PoC として実装する詳細設計。ADR Option 1 に従い、session scoped
の編集機構 (Selection/Tool/Snap/Gesture/Preview) は `synvelis-workspace`
の ViewState 側、atomic 複数対象 mutation は `synvelis-project` の
`ProjectOp` 側に置く。

前提: `ProjectController::apply` は既に validate → scratch clone apply →
atomic swap → 1 undo push を提供する。ruviform 契約は `ruviform-ui` の
dock/window のみ消費し、時刻変換は `synvelis-project::TickTimeMapping` 経由。
`Tick`/`TickDuration` unchecked 演算子は使わない (規約)。

## Scope

### In Scope

- `synvelis-workspace` 新 module: `selection.rs` (domain 別 Object
  Selection / TimeSelection / LaneScope / EditBinding / EditTimeDomain)、
  `tool.rs` (Tool・HitProbe・解決規則)、`snap.rs` (GridConfig・SnapConfig・
  SnapEnvironment・候補選択)、`gesture.rs` (fixation・preview・state
  machine)、`edit.rs` (gesture → `ProjectOp` 構築 + anchor 演算)。
- `ViewState` 拡張: `editing_selection` を `object_selections`
  (domain 別 map + active domain) へ移行、新フィールド `time_selection`
  `lane_scope` `active_tool` `grid` `snap` `edit_binding` `gesture`
  (`#[serde(skip)]`)。`WORKSPACE_SCHEMA` 1 → 2。
- `synvelis-project`: `ProjectOp::Composite` / `SplitClip` /
  `RemoveClipsInRange` 追加。
- `command.rs`: `edit.undo`/`edit.redo` (input context で lane 配送)、
  `clips.delete-range` (time selection + lane scope 由来)、`tool.activate`。
- `tests/ed_a.rs` 受入 test (ED-A-01..05)。

### Out of Scope

- 実 GUI・px hit test・OS keybinding・pointer device 差・実 AutoScroll。
- Note/Automation entity、Take/Comp、Remote Asset、Quantize、Slip/Stretch/
  Fade の実 commit 意味 (resolve まで)。
- ruviform 側の変更・rf#189/#190 の再実装。

## Components and Boundaries

```text
synvelis-workspace (lib)
  ├─ selection.rs  SelectionDomain / ObjectSelections / TimeSelection /
  │                LaneScope / EditTimeDomain / EditBinding
  ├─ tool.rs       Tool / HitZone / HandleKind / HitProbe /
  │                EditOperation / ToolResolution / resolve_tool
  ├─ snap.rs       GridConfig / SnapConfig / SnapUnit / SnapTarget /
  │                SnapMode / SnapEnvironment / SnapWinner /
  │                resolve_snap
  ├─ gesture.rs    Gesture / GestureFixation / GesturePreview /
  │                GesturePhase / CancelReason / GestureOutcome
  ├─ edit.rs       gesture → Vec<ProjectOp> 構築、anchor delta 演算、
  │                time-domain 導出
  ├─ state.rs      ViewState (拡張) / ViewStateRegistry
  ├─ command.rs    CommandKind / CommandContext / CommandOutcome +
  │                edit.undo/edit.redo/clips.delete-range/tool.activate
  ├─ session.rs    ProjectSession (不変 — apply/undo/redo が hook)
  └─ workspace.rs  WorkspaceManager + editing API
                   (pointer_down/drag_update/commit_gesture/cancel_gesture/
                    set_tool/set_time_selection/set_lane_scope/open_in_editor)

synvelis-project (lib)
  └─ ops.rs        ProjectOp += Composite / SplitClip / RemoveClipsInRange
```

## Data and Control Flow

### Selection Model (合意①)

`ObjectSelections { active_domain, by_domain: BTreeMap<SelectionDomain,
EditingSelection> }`:

- `SelectionDomain = Clips | Lanes | Tracks | Sources` (現行
  `SemanticTarget` で表現可能な domain のみ。Note/AutomationPoint は entity
  未導入のため未実装 — 拡張点は domain enum の variant 追加)。
- `set(sel)` は targets の domain を推論し、混在なら
  `EditError::MixedDomains` (#11「曖昧な異種 Domain の範囲選択は無効化」)。
  成功時はその domain の selection を格納し active_domain にする。
- `primary()` は active domain selection の最後尾 (gesture anchor)。
- Command 側: `CommandSpec` に `required_domain: Option<SelectionDomain>` を
  追加し、`clip.remove`/`clip.mute` は `Clips` を要求 — active domain が
  違えば `Unresolved` (domain 非対応 command の誤適用を構造的に防ぐ)。
- `time_selection: Option<TimeSelection>` (`Musical(MusicalRange)` |
  `Media(MediaSpan)`) — object selection と並存。`clip.remove` は time
  selection を読まない。range 削除は別 command `clips.delete-range`
  (origin view の musical time selection + lane scope →
  `RemoveClipsInRange`)。
- `lane_scope: LaneScope = All | Lanes(BTreeSet<LaneId>)` — marquee /
  delete-range など **range 由来の対象対象導出**を絞る。既に explicit な object
  selection の command target は scope で拒否しない (対象は発行時に確定
  済みという WS-A 契約と整合)。
- `edit_binding: Option<EditBinding { clip, source }>` — Editor の
  Display/Edit binding (#11 §5)。`open_in_editor` で設定。Selection でも
  command target でもない。

### Tool System (合意②)

`Tool = Smart | Select | Marquee | Draw | Split | Trim | Slip | Stretch |
Fade | Erase` — ViewState の `active_tool` (Project とは無関係)。

`HitProbe { position: i64, zone: HitZone, target: Option<SemanticTarget> }` —
headless な意味的 probe (px 座標・実 hit test は対象外)。`HitZone =
Handle(HandleKind) | Body | LaneEmpty(LaneId) | Background`、`HandleKind =
TrimStart | TrimEnd | FadeIn | FadeOut | Stretch`。

`resolve_tool(tool, probes, active_selection) -> ToolResolution`:

- Smart tool は probe 群から最高 priority の zone を採用
  (Handle > Body > LaneEmpty > Background)。同順位候補が複数あれば
  `ToolResolution::Ambiguous { candidates, suggested_tools }` — cursor/
  tooltip/dedicated tool への入口に相当 (headless では結果値として観測)。
- Smart + Handle{TrimStart|TrimEnd} → `TrimClipStart|TrimClipEnd`、
  Handle{FadeIn|FadeOut} → `FadeEdge`、Handle{Stretch} → `StretchClip`、
  Body{clip} → `MoveClips`、LaneEmpty → `Marquee`、Background → `Select`。
- Dedicated tool は明示固定 (Smart 解決を通らない): Select→Select、
  Marquee→Marquee、Draw→DrawClip (LaneEmpty のみ合法)、Split→SplitClip、
  Trim→edge 必須 (Handle の edge 情報使用、Body は `Ambiguous`)、
  Erase→EraseTargets、Slip→SlipContent、Stretch/Fade→対応 op。
- `MoveClips`/`EraseTargets` の対象: hit clip が active object selection に
  含まれる → selection 全体、なければ hit clip 単独 (primary = hit clip)。

### Snap / Grid (合意③)

- `GridConfig { visible, unit_ticks }` — 表示のみ。`SnapConfig { enabled,
  unit: SnapUnit, targets: BTreeSet<SnapTarget>, mode: SnapMode,
  threshold_ticks, hysteresis_ticks }` — 独立。
- `SnapUnit = Explicit(i64) | Auto` — Explicit は zoom で変わらない。
  Auto は `auto_unit(zoom)` (PoC 決定則: `zoom >= 1.0 → 240`, else `960`) の
  み zoom に追従する。
- `SnapTarget = Grid | Marker | ClipEdge | Playhead`。
  `SnapEnvironment { domain, markers, clip_edges, playhead, zoom }` —
  candidates は gesture の time domain の i64 で供給される (domain が異なる
  候補は `SnapEnvironment::candidates` で除外 — env 構築側が domain を宣言)。
- `SnapMode = Absolute | Relative`。Absolute は pointer 位置を候補位置へ
  snap、Relative は `pointer - start` の delta を unit 倍数に丸める。
- `resolve_snap(position, start, config, env, prev) -> SnapResolution
  { position, winner }`: 有効 targets の最近傍候補を threshold 内で集め、
  明示 priority (PoC 固定: Marker > ClipEdge > Playhead > Grid) → 距離の順
  で採用。`prev` が残り candidate なら新 winner が hysteresis 分以上に近く
  ない限り prev を保持 (微振動防止)。`SnapWinner { position, source,
  label }` — 「何に snap したか」を返す。
- 複数対象 Move: primary のみ snap 解決、全対象に同一 delta を適用
  (edit.rs で実施)。

### Editing Gesture / Atomic Transaction (合意④)

```text
Idle ─pointer_down(probes)→ GestureStarted (fixation 確定)
     ─drag_update(position, env)→ Previewing (preview 更新のみ)
     ─commit_gesture→ validate-all → session.apply(1 op) → Committed|Rejected
     ─cancel_gesture(reason)→ Idle (project 不変)
```

`GestureFixation` — pointer down 時に固定: `tool`, `operation`,
`origin_view`, `targets`, `primary`, `time_domain` (primary clip の anchor
domain 由来), `project_revision`, `tempo_revision` (tempo_map.revision),
`start_position`, `start_anchors`, `draw_source`/`split_b_offset` (op 固有
payload)。

- `time_domain` 導出: primary `TimeAnchor::Musical → Musical`、
  `Media(span) → Media{start の base}`。MoveClips で対象 clip の anchor
  domain が全て gesture domain と一致しない → `MixedTimeDomains` で開始拒否
  (共通 domain 明示演算は deferred)。
- `GesturePreview { current_position, delta_ticks, proposed_anchors, snap,
  valid, invalid_reason, affected }` — `drag_update` はこれだけを更新し、
  fixation は不変 (Smart tool でも Move→Trim 等への途中切替なし)。
- `drag_update` の preview 検証: proposed anchor の `end > start`、対象
  clip が project に尚存。存しない → `valid=false` + reason (commit は拒否)。
- `commit_gesture`: `build_gesture_ops(fixation, preview, project)` →
  `Vec<ProjectOp>`。0 件 (Select/Marquee) → ViewState の object selection
  更新のみで `Committed{revision:None}`。1 件 → 直接 apply。複数 →
  `ProjectOp::Composite` として **1 回** `session.apply` (= 1 undo)。
  `SlipContent|StretchClip|FadeEdge` → `UnsupportedOperation` で拒否。
  失敗時は project 不変 + reason。apply は `expected_revision =
  fixation.project_revision` — gesture 中の外部変更は `StaleRevision` で
  拒否 (rebase/すり替えなし)。
- `CancelReason = Esc | PointerCancel | WindowClosed | DeviceLost |
  TargetDeleted | InputScopeLost` — `cancel_gesture` は preview を破棄
  するのみ。`remove_view`・`on_window_event(Destroyed)` でも該当 view の
  gesture を破棄する (WindowClosed)。
- `edit.undo`/`edit.redo` は command dispatch の先頭で input context により
  lane 配送: `View` → `session.undo()/redo()` (engine hook 経由 →
  `graph_builds` 進む + revision 配信)、`TextInput|Ime` → workspace の
  `text_undos` stub 計数 (project 不変)、`PluginGui|Modal` → `Suppressed`。

### Cross-View / Linked Context (合意⑤)

- `open_in_editor(view, clip)` → `state.edit_binding = {clip, source}`。
  editing_selection・linked_context・command 解決を一切変えない
  (binding は command target に解決されない — resolve_target は
  object_selections のみ読む)。
- Gesture 進行中の link event 配送 (`select` → publish/deliver) は
  `linked_context`/`inspection_focus` のみ更新 — `gesture.fixation` は不変。
- Inspector/SignalFlow 等の read-only view の選択は linked_context を
  書くだけで project に mutation を発行しない (WS-A 契約継続)。

### 新規 `ProjectOp` (synvelis-project)

- `Composite { ops: Vec<ProjectOp> }`: validate = 空でない + 入れ子
  Composite 禁止 + 全 inner validate。apply = scratch 上で逐次 apply。
  1 `apply` = 1 undo = all-or-nothing。
- `SplitClip { clip_id, at: MusicalTick, clip_b_source_offset:
  SourceFrame }`: validate = clip 存在・anchor が Musical・
  `start < at < end`・`clip_b_source_offset >= clip.source_offset`。
  apply = A を `[start, at)` へ縮小、B を `[at, end)` で新規 ID 発行して
  `lane.clip_ids` の A 直後へ挿入 (source_id/muted 継承)。
- `RemoveClipsInRange { lanes, range: MusicalRange }`: validate = lanes 非
  空・存在・range 非空。apply = 対象 lane の Musical anchor clip で range
  と overlap するものを全て除去 (Media anchor clip は domain 外 — 触らない)。

## Non-Functional Constraints

- 全 API non-RT・headless。px・OS keybind・pointer device 差は test input
  の意味的 probe のみで、数値として持ち込まない。
- 時間演算は `MusicalTick::checked_add`/`ticks_since`・`SourceFrame::
  checked_add` 等 checked API のみ。`MediaTimeValue` は同一 base 内で
  value 演算 (delta は base ticks の i64)。
- `#[serde(skip)]` gesture、`#[serde(default)]` 新 field、`WORKSPACE_SCHEMA`
  = 2。`missing_docs`・clippy pedantic (`-D warnings`) 準拠。
- 依存追加なし (`serde`/`serde_json`/`synvelis-project`/`ruviform-ui` のみ)。

## Risks and Trade-offs

- `SplitClip` は Musical anchor のみ・`clip_b_source_offset` は caller 供給
  (tick→source-frame の rate 変換が model にない) — PoC で正直に記録。
- Media anchor の MoveClip delta は base 単位の i64 — `TickTimeMapping` による
  Musical↔Media 変換は mixed-domain 演算契約の後続設計へ deferred。
- `SnapUnit::Auto` の zoom→unit 則は任意の PoC ルール。実分解能表は UI PoC。
- `editing_selection` 廃止で WS-A snapshot の互換が切れる → schema 2 で
  明示 (PoC 内で前向き移行、旧 snapshot の復元保証なし)。

## Deferred Design Concerns

- px Hit Size/Drag Threshold/Hover/Pen/Touch | real-GUI lane (#243)。
- Magnetic mode・Threshold/Hysteresis 数値・ranking 設定化 | UI PoC。
- 混合 Time Domain の共通基準演算 (TickTimeMapping 経由) | 後続設計。
- Lane Scope の explicit object selection への適用範囲 | Arrangement 詳細。
- Note/Automation domain の SelectionDomain 拡張 | entity model 追加後。

## Verification Notes

- [ ] ED-A-01..05 が `tests/ed_a.rs` に個別 test として存在し PASS。
- [ ] `mise run fmt-check`/`check`/`lint`/`test` 全 PASS。
- [ ] gesture 中の preview が revision/undo/graph_builds を進めない。
