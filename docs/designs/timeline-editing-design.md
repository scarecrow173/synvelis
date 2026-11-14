---
id: "DESIGN-T4E6PtTJ870Fpg4pFi3aM6"
type: "design"
status: "approved"
title: "Timeline Editing / Selection / Tool System — Snap・Gesture・Undo・Cross-View編集の共通設計（synvelis#11）"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
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
  implemented-by: []
  depends-on:
    - "docs/specs/timeline-editing.md"
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/specs/workspace-model.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/designs/ed-a-editing-model-design.md"
    - "docs/designs/ed-b-media-domain-editing-design.md"
    - "docs/designs/midi-source-note-editing-design.md"
    - "docs/designs/workspace-model-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/specs/context-inspector.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/timeline-editing.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/243"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/243"
    - "https://github.com/scarecrow173/synvelis/issues/12"
  deferred-by: []
---

# Timeline Editing / Selection / Tool System — 共通設計

## 概要

synvelis#11 で合意済みの編集操作体系を、Arrangement / Piano Roll /
Audio Editor / Automation Lane が共有する **製品 UX 契約**として固定
する設計。基盤となる headless PoC（ED-A #38/PR #39・ED-B #65/PR #66）
と Project/Workspace Model（#10・#2）の確定契約を正本として消費し、
本設計はそれらを製品操作へ写像する境界・不変条件・Fixture・判断記録
を定義する。数値（px 閾値・Snap 単位・Hysteresis・OS キーバインド）
は Issue #11 §9 のとおり PoC/実機後決定事項であり、本設計では発明
しない。

## スコープと Owner 境界

| 領域 | Owner | 本設計の扱い |
| --- | --- | --- |
| Selection/Tool/Snap/Gesture の DAW 意味論、Gesture→Project Command→Undo、Link Group での意味的 Mapping | **Synvelis** | 契約を定義（本書） |
| 再利用可能な Pointer/HitTest/Viewport、Media/Musical Time、Window/Docking/Input primitives | **Ruviform** | 既存 API を消費。独自再実装しない。DAW 固有編集命令を持ち込まない |
| 製品 GUI（Arrangement/Piano Roll/Audio Editor/Automation Lane の実画面）、実 OS 入力・Focus・DPI/IME | **synvelis#112**（実装）・**#114**（受入 Gate） | 契約のみ供給。実装・実機判定は別 Gate |
| Project Model（Track/Lane/Clip/Source/Timeline・時間 Domain・Undo/Revision） | **synvelis#10** 系実装 Issue | `ProjectOp`/`ProjectController`/`TickTimeMapping`/`ClipPlacement` 契約を消費。schema 変更を要求しない |
| Recording/Take/Comping の Session lifecycle・Comp 編集 | **synvelis#12** 系実装 Issue | 共通編集 UX（Preview→検証→Atomic Commit→1 Undo）の継承境界のみ定義。Session 録音は別契約 |
| Remote Asset の Fetch+Validate+Commit | **synvelis#9** 系実装 Issue（`synvelis-library` import pipeline） | 「検証完了前に不完全 Clip/Manifest を確定しない」境界のみ参照 |

## 既存実装との対応付け

| 契約領域 | 既存正本 | 状態 |
| --- | --- | --- |
| Selection 分離・domain 別保持・Time Selection 独立・range 削除 command | ED-A `selection.rs`/`command.rs`（`ObjectSelections`/`TimeSelection`/`LaneScope`/`clips.delete-range`） | headless 実装済み（`ed_a_01*`） |
| Smart/Dedicated Tool 解決・HitProbe 優先順・Gesture fixation | ED-A `tool.rs`/`gesture.rs`（`resolve_tool`/`GestureFixation`/`Ambiguous`） | headless 実装済み（`ed_a_02`） |
| Grid/Snap 分離・SnapUnit・候補選択・Hysteresis・SnapWinner label | ED-A `snap.rs`（`GridConfig`/`SnapConfig`/`SnapEnvironment`/`resolve_snap`） | headless 実装済み（`ed_a_03`） |
| Gesture 状態機械・Preview 非破壊・Atomic Commit・1 Undo・Cancel・StaleRevision 拒否 | ED-A `gesture.rs`/`edit.rs`/`workspace.rs` + `ProjectOp::Composite` | headless 実装済み（`ed_a_04`） |
| Linked Context/EditBinding 分離・Gesture 中の retarget 禁止 | ED-A `edit_binding`/link 配送 + `uic5_*`/`ws_a_*` | headless 実装済み（`ed_a_05`/`uic5_02_*`） |
| Media domain の Move/Trim/Slip/Marquee・混在 domain 規則・`EditTimePoint` | ED-B `edit.rs`/`selection.rs`（`ed_b_01..07`） | headless 実装済み |
| Clip/Note の時間配置・Playback Mapping・Linked Source 共有・明示 Detach | MD-A（`ClipPlacement`/`ClipPlayback`/`TimeAnchor`/`ClipExtent`）+ MIDI Source #52（`MidiSource`/`DetachClipSource`） | headless 実装済み（`md_a.rs`/`midi_source.rs`） |
| View/Window/Project の寿命分離・Command routing・入力文脈別 Undo lane・Link Group | WS-A + workspace-model #2（`ViewStateRegistry`/`CommandRouter`/`InputContext`） | headless 実装済み（`ws_a_*`/`workspace_model.rs`） |
| History/Visual Diff の read-only Selection・Apply の明示 Command 化 | VCS-01〜06（`vcs04_history.rs`/`vcs06_*`）+ Issue #7 契約 | headless 実装済み |
| Remote Asset の Fetch→Verify→Publish→Commit（失敗時に部分成果を残さない） | `synvelis-library` import pipeline（`import.rs`/`import_recovery.rs`） | headless 実装済み |
| 実画面での Pointer/Keyboard/IME/Drag・Snap Guide 描画・2 Window 実競合 | **#112/#114・ruviform#243** | **NotRun（別 Gate）** |

## Selection Model（合意①）

View ごとに独立して保持する 7 概念。Project State・他 View の
Selection を暗黙に書き換えない。

| 概念 | Owner/意味 | 編集への影響 |
| --- | --- | --- |
| Object Selection | Active Editor View の選択 Clip/Note/Track/Automation Point 等。Domain 別に保持 | Command の明示対象。domain 非対応 command は適用しない |
| Time Selection | View の Timeline 区間（Musical/Media domain を明示） | Object Selection と並存。**単独では破壊的編集を起こさない** |
| Lane Scope | 対象 Track/Lane の適用範囲 | range 由来の対象導出（marquee/delete-range）を絞る。明示 object selection の command target は絞らない |
| Primary Target | 選択基準・Gesture 起点（複数 Drag のアンカー） | Gesture fixation の `primary`。Snap は Primary のみ |
| Inspection Focus | 調査対象（Context Inspector 等） | Editing Selection と独立。Project を mutate しない |
| Active View/Input Focus | Command Router の入力文脈 | Undo lane 配送（View/TextInput/Ime/PluginGui/Modal）を決める。Selection と同一ではない |
| Linked Context | Link Group の非破壊ナビゲーション通知 | 他 View の Selection/Project を書き換えない |

規則:

- **TE-S-01** Click=単独選択、Shift=同一編集 Domain の範囲/追加、
  Ctrl/Cmd=トグル、空白 Click=アクティブ Domain の選択解除、
  矩形 Drag=Marquee、Select All=Active View Domain。OS/User
  Preferences による再マッピングを許容するが、既定の意味は全 View
  で共通。
- **TE-S-02** Range Selection の順序は Domain ごとに定義
  （Track Display Order / Note の Time+Pitch / Clip の Lane+Time）。
  異種 Domain を跨ぐ曖昧な範囲選択は無効化または明確な Feedback
  を返す（暗黙の混在を許さない）。
- **TE-S-03** 右クリック時、既選択集合内なら複数選択を保持して
  集合向け Context Menu を開く。未選択対象の右クリックは Menu
  Target ID を明示し、既存 Selection への影響と Menu の編集対象を
  混同しない。
- **TE-S-04** Time Selection と Object Selection は両立する。
  Delete は Active Editor の明示的 Object Selection を対象とし、
  Time Selection だけで区間の Ripple Delete を実行しない。
  Time Delete/Ripple Delete は独立 Command であり、対象 Domain・
  Lane Scope を明示して発行する。
- **TE-S-05** Domain ごとに Selection を保持する。同時ハイライト
  されても Domain 非対応の Command を誤適用しない。複合編集は専用
  Command と明示的対象宣言で行う。

## Tool System（合意②）

- **TE-T-01** Smart Tool を標準とし、Select/Draw/Split/Trim/Slip/
  Stretch/Fade/Erase の Dedicated Tool を提供する。初期 Toolbar は
  Smart/Select/Draw/Split 中心、その他は Palette/Shortcut/Context
  に露出可能。
- **TE-T-02** Smart Tool の Hit Test 解決は原則優先順
  **明示 Handle → Editor 内の編集対象 → Clip Body → Lane Empty →
  View Background**。数値の Hit Area は Pointer Device と Zoom に
  応じて調整する（具体 px は PoC 後決定）。
- **TE-T-03** Pointer Down 時に Command Domain と Target を解決し、
  Gesture 開始後は操作種・対象を固定する。Move の途中で Pointer が
  Clip 端へ来ても Trim に切替えない（Smart/Dedicated いずれでも
  同じ）。
- **TE-T-04** 極端に短い Clip/高密度 Note/複数 Handle 重複など
  Smart Tool が曖昧な場合は `Ambiguous` として候補と推奨 Dedicated
  Tool を返し、Cursor/Tooltip/Alternative Tool の明確な入口を提供
  する。曖昧さを黙って一つの操作へ丸めない。
- **TE-T-05** Active Tool は Editor View State であり、Tool 切替は
  Project の音楽的 Undo や VCS Semantic Diff に入れない。
- **TE-T-06** 録音・Remote Asset Import・Automation Write など副作用
  の強い操作は、表示用 Tool の状態で暗黙に実行しない。これらは独立
  した状態機械/Command の対象である。

## Snap / Grid と時間 Domain（合意③）

- **TE-G-01** Grid Display と Snap Enabled は独立。Snap Unit・
  Targets・Mode・Threshold は表示 Grid と別に指定できる。Grid 描画
  を Zoom で間引いても明示 Snap Unit を変更しない。自動分解能
  （`Auto`）のみ Zoom に追従する別モードとして扱う。
- **TE-G-02** 時間 Domain は対象の意味で決まる: MIDI Note/Musical
  Anchor Clip は **Musical Time**、Absolute Anchor Clip は **Media
  Time**、Source 内 Audio は **native sample/Source-local**、
  Automation Point は **Owner Lane の Time Domain**。基準時間の型と
  変換を維持し、暗黙の domain 変換を行わない。
- **TE-G-03** 複数 Clip Move では **Primary Target のみ Snap** し、
  残りには同一移動量を適用して相対位置を維持する。複数対象を個別
  Grid に吸着しない。
- **TE-G-04** 異なる Anchor Domain の選択を一斉移動する場合は、
  共通の移動 Domain/基準を明示しなければ曖昧な編集を拒否する。
  現行の定義済み規則は ED-B の混在 domain 結果表（move=anchor
  domain で commit、trim=定義済み拒否、slip=定義済み変換/拒否）
  を正本とし、未宣言変換を要求する操作は `UnsupportedOperation`/
  `MixedTimeDomains` で拒否する。
- **TE-G-05** Snap 候補は対象・Time Domain で絞り、表示距離と明示
  Priority で選択する。Hysteresis により微細な Pointer 移動で候補が
  頻繁に入れ替わらないようにする。**採用した候補と時間を Snap
  Guide/ラベルで説明可能**にする（`SnapWinner { position, source,
  label }` に相当）。
- **TE-G-06** Clip/Note 編集時の Snap と、既存 Note イベント群への
  Quantize は別操作。Ruviform PPQN960 への強制丸めで録音時の高精度
  MIDI タイムスタンプを失わない（#19/#20 の正本を破壊しない）。
- **TE-G-07** Snap/Time Selection の境界は Project の half-open
  `[start,end)` と Editor の Gesture/描画用範囲表示を区別し、
  Loop/NoteOff のイベント処理契約へ接続する。

## Editing Gesture / Atomic Transaction（合意④）

状態機械:

```text
Idle → PointerDown/HitTest → ResolveTool+Target → GestureStarted
     → Previewing → Commit or Cancel
```

- **TE-E-01** Gesture 開始時に少なくとも次を固定する
  （`GestureFixation`）: Origin ViewId/Window、Input Device、
  Tool/Operation、Target IDs + Primary Target、Project/Target
  Revision、Time Domain/Anchor、必要な TempoMap Revision、
  Snap/Selection Snapshot、変更前 State と Preview Delta の表現。
- **TE-E-02** Preview は View 側の一時状態であり、Drag の毎
  Frame/Pointer 移動を Project State/Undo へ追記しない。Ghost・
  Snap Guide・Delta Indicator・Valid/Invalid・Affected Targets を
  可視化する。Preview 中に Project revision・Undo 履歴・engine
  graph が不変であること。
- **TE-E-03** Release 時に Project Controller へ検証付き Command
  を発行する。多対象編集は原則 **All-or-Nothing** — 全件検証失敗
  時は Project 正本を変更せず Reason を通知する。
- **TE-E-04** Project/Target Revision が Gesture 開始後に変化し、
  適用可否を検証できなければ拒否する。安全な自動 Rebase は後続
  候補であり、古い Target を別 Target に自動すり替えない
  （`StaleRevision`/`MixedTimeDomains`/`UnsupportedOperation` 等の
  型付き拒否）。
- **TE-E-05** PointerCancel・Esc・Window 閉鎖/破棄・Device 切断・
  Target 削除・Input Scope 喪失を検出した場合は未確定 Preview を
  破棄し Project を変更しない。AutoScroll/Zoom 中も編集対象・
  Tool・時間意味を維持する。
- **TE-E-06** 複数 Clip の Move/Resize 等、一回の Gesture は
  **1 Undo Transaction**。`ProjectOp::Composite` による単一
  atomic apply = 1 undo = 全件復旧。録音/Automation 記録は各々の
  Transaction 契約（#8/#6）を使用し、本 Gesture 契約と混同しない。
- **TE-E-07** Undo 系は **入力文脈で lane を分ける**: Project Undo
  （`edit.undo`/`edit.redo` → session engine hook 経由）、Text・
  IME Undo（text lane）、View Navigation/Workspace Layout History
  （View/Workspace）、VCS Revision History（Provider）は別物。
  Active Input Focus に基づき正しい Undo へ配送する。
- **TE-E-08** Project 変更の確定と非 RT で構築した Audio/MIDI/
  Control 実行 Plan の公開は別プロセス。反映待ち・失敗を誤魔化さず
  表示する。Gesture Preview・履歴操作は RT Audio Callback を
  ブロックしない（Preview/履歴は非 RT の View/Workspace 側処理）。

## Cross-View / Linked Context（合意⑤）

- **TE-C-01** Arrangement の MIDI Clip を開くと Piano Roll へその
  Clip/Source の**編集バインディング**（`EditBinding { clip, source
  }`）を設定する。Arrangement 側 Clip Selection を保持し、Piano
  Roll の Note Selection は別に保持する。Binding は command target
  に解決されない。
- **TE-C-02** Editor は Follow/固定できる表示 Target を持つが、
  **Editor Display Target と編集進行中の Gesture Target は独立**。
  Follow 更新・他 Window の Selection 変更で Gesture の対象を変更
  しない。
- **TE-C-03** Linked MIDI Instance では Editing Clip と Editing
  Source を別表示する。Source 編集は共有先に反映し、Clip 配置編集
  はその Clip のみ。Independent Copy（明示 `DetachClipSource`）は
  Source 変更の波及が起きない。
- **TE-C-04** Time Selection は基本 View-local。明示的に Linked
  Time Range を使う場合は Clip-local ↔ Project Musical 等の Mapping
  を検証する。編集 Selection や Command を自動同期しない。
- **TE-C-05** Inspector/Signal Flow の Linked Highlight・Navigate・
  Version Diff の Explore は Active Editor の Edit Target と無関係。
  Read-only View（Signal Probe/VCS Diff 等）の選択だけでは現行
  Project に編集が発生しない。過去 Snapshot の Apply Selected
  Change は #7 の明示的 Project Command を使用し、通常の Drag と
  混線しない。

## 状態の保存境界

| State | Owner |
| --- | --- |
| Track/Clip/Note/Automation 実データ | Project State（`synvelis-project`） |
| Object/Time Selection、Primary Target、Active Tool | Editor View State（`synvelis-workspace` ViewState、session scoped） |
| Zoom/Scroll/Grid/Snap、必要に応じ Ripple Tool State | View/Workspace/User Preferences（永続化範囲は View schema で管理） |
| Shortcut Configuration | User Preferences |
| Project Undo | Project Controller（`ProjectSession` engine hook 経由） |
| View Navigation/Workspace Layout History | View/Workspace |
| Revision/Commit History | VCS Provider |
| Gesture（進行中） | View State の一時状態 — `#[serde(skip)]` 相当で永続化しない |

**Snap・Tool・Selection・Zoom の変更だけで Project Dirty /
Semantic Diff を発生させない**。Tool に基づき確定した Clip の新しい
位置は Project State である。

## 画面操作 Fixture（UX 操作契約）

headless/実機を問わず「ユーザー操作 → 期待される意味」を固定する
Fixture。各 Fixture は TE-AC・既存 headless 証拠・後続 Gate を併記
する。数値（px・ms・tick 閾値）は含めない。

### F-UX-01 Tool 解決と Gesture 固定（TE-01/TE-02）

- **操作**: Smart Tool で Clip Body を Pointer Down→Drag。途中で
  Pointer が Clip 端 Handle 領域へ入っても操作は Move のまま。
  Dedicated Trim Tool 選択中に Body を Drag しても Trim にならない
  （Ambiguous または Select に留まる）。
- **極小/近接 Handle**: 複数 Handle が重なる・Clip が表示上極小の
  場合、Smart Tool は `Ambiguous { candidates, suggested_tools }`
  を返し、Cursor/Tooltip と Dedicated Tool への入口を提供する。
  曖昧な hit を黙って一つの操作へ丸めない。
- **証拠**: `ed_a_02_tool_resolution_and_fixation`（headless）。
  実 px Hit Size/Drag Threshold・Cursor/Tooltip 描画は #112/#114。

### F-UX-02 Selection の両立と Delete の対象（TE-03）

- **操作**: Time Selection（例: 小節 2〜4）を設定した状態で Clip を
  Click 選択 → Delete。Clip のみ削除され、Time Selection 区間の
  Ripple Delete は起きない。Time Selection のみで Delete →
  Object Selection が無ければ no-op または明示的な range command
  の確認を経て、暗黙の Ripple はしない。
- **証拠**: `ed_a_01_selection_model_separation`・
  `ed_a_01b_time_selection_and_range_delete`（headless: Time
  Selection 単独で `clip.remove` は Unresolved、range 削除は独立
  `clips.delete-range`）。実画面の Menu/Feedback は #112/#114。

### F-UX-03 複数 Clip Move と 1 Undo（TE-04）

- **操作**: 5 Clip を選択して Drag → Release。相対位置が保持され、
  1 回の Undo で全件が元位置へ戻る。Redo で全件が再適用される。
- **証拠**: `ed_a_04_preview_atomic_undo`（複数対象 composite=1
  undo）・`ed_b_01_media_multi_move_is_one_undo`（media domain）。
  実画面 Drag/Undo UX は #114。

### F-UX-04 時間 Domain の明示と混在拒否（TE-05）

- **操作**: Musical Anchor Clip と Absolute Anchor Clip を混在選択
  して Move → 共通 Domain 未宣言なら開始時/commit 時に拒否。
  Snap 中に Musical↔Media の暗黙変換を行わない。ED-B の定義済み
  結果表（move=anchor domain、trim=拒否、slip=定義済み変換/拒否）
  のみを許容する。
- **証拠**: `ed_a_03_snap_grid_time_domain`・
  `ed_b_04_mixed_clip_move_ok_trim_rejects`・
  `ed_b_04_mixed_slip_uses_tempo_map`・
  `ed_b_04_abs_musical_move_ok_trim_rejects`・
  `ed_b_04_abs_musical_slip_rejects`・`ed_b_06_domain_mismatch_
  rejects_at_start`（headless）。

### F-UX-05 Grid/Snap の独立性と Snap Guide（TE-06/TE-07）

- **操作**: Grid を非表示にしても Snap Enabled のまま Move → Snap
  が動作する。Zoom を変えても明示 Snap Unit は不変（Auto のみ
  追従）。Drag 中に Snap Guide/ラベルが採用候補（Grid/Marker/
  Clip Edge/Playhead）と時間を説明し、微小 Pointer 移動で候補が
  振動しない（Hysteresis）。
- **証拠**: `ed_a_03_snap_grid_time_domain`（grid 非表示・zoom
  不変・winner label・hysteresis、headless）。Snap Guide の実
  描画・視認性は #112/#114。

### F-UX-06 Gesture Cancel と競合拒否（TE-08/TE-09）

- **操作**: Drag 中に Esc/PointerCancel/Window を閉じる → Project
  不変。Drag 中に別 Window/別操作で対象 Clip が削除・変更される
  → Release 時に `StaleRevision`/対象不存在で拒否し、誤った
  Target/Revision に適用しない。Preview は途中で破棄される。
- **証拠**: `ed_a_04_preview_atomic_undo`（cancel paths・stale
  revision・window destroy）・`ws_a_03_command_routing`・
  `uic5_02_commit_drops_on_stale_revision`・
  `uic5_04_foreign_change_conflicts_then_explicit_rebase_pending`。
  実 Window 破棄・2 Window 実競合は #114。

### F-UX-07 Cross-View 編集と Linked Source（TE-10/TE-11/TE-12）

- **操作**: Arrangement で MIDI Clip を開き Piano Roll で Note を
  編集 → 共有 Source が更新され他の Linked Clip にも反映されるが
  Arrangement の Clip Selection は変わらない。Inspector/Signal
  Flow/Version Diff で対象を Explore/Highlight → Editor の Edit
  Command 対象・Selection は変わらない。VCS の過去 Snapshot を
  選択 → Live Project は不変（Apply は明示 Command のみ）。
- **証拠**: `midi_source.rs::linked_clips_share_source_edits_and_
  detach_is_explicit`・`ed_a_05_linked_context_separation`・
  `uic5_02_inspect_is_not_edit`・`uic5_03_*`・
  `vcs04_history.rs::view_mode_and_clear_selection_are_display_
  only`/`vcs_ops_never_touch_project_engine_or_transport`。
  実 Piano Roll 画面・実 VCS Diff UI は #112/#114。

### F-UX-08 Remote Asset 取込と RT 境界（TE-13/TE-14）

- **操作**: Remote Asset を Timeline へ Drag&Drop → Fetch+Verify
  完了前に Clip/Manifest を確定しない。失敗時は不完全な Clip/
  Manifest を残さず、理由を通知する。Audio/MIDI 再生中に Gesture
  Preview・Undo/Redo を行っても RT Audio Callback をブロック
  しない。
- **証拠**: `synvelis-library/tests/import.rs`（verify/publish/
  digest mismatch/publish failure resume/missing lane reject）・
  `import_recovery.rs`・`workspace_model.rs::playback_dock_window_
  churn_preserves_transport_revision_and_history`・`ws_a_05`。
  実 OS Drag&Drop・実 RT 計測は #114/EXT-L3。

## Decision Log

- **D-01 Selection を 7 概念に分離し Object/Time/Lane/Primary/
  Inspection/Focus/Linked を別 state とする** — 採用。Issue #11 合意
  ①および ED-A `ObjectSelections`/`TimeSelection`/`LaneScope`/
  `edit_binding` の実装と一致。代替「単一 Selection に mode flag」
  は domain 混在・Time/Object 両立・Linked 非破壊を同時に表せず
  不採用。
- **D-02 Time Selection 単独では破壊的編集を起こさず range 削除は
  独立 Command** — 採用。`clip.remove` は object selection のみを
  読み、`clips.delete-range` が time selection+lane scope 由来の
  対象を明示導出する。暗黙 Ripple を構造的に排除。
- **D-03 Smart Tool を標準とし Gesture 開始で操作種・対象を固定、
  曖昧時は `Ambiguous`+推奨 Dedicated Tool** — 採用。ED-A
  `resolve_tool` の優先順と fixation 契約を製品 UX へ写像。途中
  切替を許す案は「Drag が Trim に化ける」誤操作を生むため不採用。
- **D-04 Grid Display と Snap を別設定軸にし、明示 Snap Unit は
  Zoom で不変・Auto のみ追従** — 採用。ED-A `SnapUnit::
  Explicit|Auto` と一致。Grid 密度連動案は「Grid を消すと Snap が
  効かない/Zoom で Snap が変わる」混乱を生むため不採用。
- **D-05 Snap は Primary のみ・複数対象へ同一 delta・混在 domain
  は共通 Domain 明示必須・未宣言変換は拒否** — 採用。TE-G-03/04。
  個別吸着・暗黙変換案は相対位置破壊・domain 意味破壊のため不採用。
  ED-B の定義済み変換（musical slip の tempo map 経由等）のみを
  許容する現行契約を正本とする。
- **D-06 Gesture を fixation 型で固定し Preview は View 側一時状態、
  commit は全件検証→`ProjectOp::Composite` 1 回→1 undo、stale/
  cancel は拒否・破棄** — 採用。ED-A `GestureFixation`/
  `commit_gesture` の実装と一致。frame ごとの Project 書込み・
  自動 rebase・target すり替えは不採用。
- **D-07 Undo を入力文脈で lane 分離（Project/Text/IME/View/VCS）**
  — 採用。WS-A/WSM の `InputContext` 配送と一致。単一 Undo stack
  案は Text 編集と Project 編集の混線を生むため不採用。
- **D-08 Linked Context は非破壊 navigation、EditBinding は
  command target に解決しない、Gesture 中の retarget 禁止** — 採用。
  ED-A `ed_a_05`・`uic5_02_*` と一致。Selection 自動同期案は
  「Piano Roll の Note 選択が Arrangement を書き換える」事故を
  生むため不採用。
- **D-09 状態所有を表で固定し View 状態変更は Project Dirty/
  Semantic Diff にしない** — 採用。WSM State 所有者表と整合。
- **D-10 Remote Asset は Fetch+Validate 完了まで Clip/Manifest を
  確定しない** — 採用。`synvelis-library` import pipeline の
  verify→publish→commit 契約を Timeline 取込へ適用。部分 commit
  案は不完全 Manifest を残すため不採用。
- **D-11 数値閾値（px Hit Size・Snap Threshold/Hysteresis・OS
  キーバップ）を本設計で発明しない** — 採用。Issue #11 §9 の
  PoC 後決定事項。headless は意味的 probe のみで数値を持ち込ま
  ない（ED-A と同じ境界）。
- **D-12 Read-only View（VCS Diff/Inspector/Signal Flow）の選択は
  Live Project を mutate せず、Apply は #7 の明示 Command** — 採用。
  `vcs_ops_never_touch_project_engine_or_transport`・
  `view_mode_and_clear_selection_are_display_only` と一致。

## 代表 Scenario と Expected Oracle

### Normal Scenarios

- **S-N-01 Smart Tool Move**: Clip Body Pointer Down→Drag→Release。
  Oracle: fixation={tool:MoveClips, targets, primary, revision}、
  preview 中 project/undo/graph_builds 不変、commit で 1 revision
  進行・1 undo で全件復旧。
- **S-N-02 Time+Object Selection 併存 Delete**: Time Selection 範囲
  内の Clip を object 選択して Delete。Oracle: 選択 Clip のみ削除、
  区間 ripple なし、Time Selection は View state として残る。
- **S-N-03 Snap 付き複数 Move**: Marker/Clip Edge が Snap 候補に
  ある状態で 3 Clip を Drag。Oracle: Primary のみ snap、他は同一
  delta、winner label が候補を説明、相対位置保持、1 undo。
- **S-N-04 Piano Roll Linked Source 編集**: Arrangement で MIDI Clip
  を EditBinding→Piano Roll で Note Move commit。Oracle: 共有
  Source 更新・他 Linked Clip に反映、Arrangement object selection
  不変、Piano Roll note selection は独立。

### Failure Scenarios

- **S-F-01 Gesture 中 Cancel**: Drag 中に Esc。Oracle: preview 破棄、
  project revision/undo/clip 位置が開始前と一致。
- **S-F-02 Gesture 中の外部変更**: Drag 中に別 Window で対象 Clip
  削除→Release。Oracle: `StaleRevision`/対象不存在で commit 拒否、
  project 不変、別 Target へのすり替えなし。
- **S-F-03 混在 Domain Move**: Musical+Absolute Clip 混在選択で
  共通 Domain 未宣言 Move。Oracle: 開始時または commit 時に型付き
  拒否、project 不変。
- **S-F-04 Remote Asset 取込失敗**: Fetch/Verify 失敗の Drag&Drop。
  Oracle: Clip/Manifest が部分的に残らず、理由が通知される
  （`import.rs` の digest mismatch/publish failure 系契約）。
- **S-F-05 Read-only Snapshot 選択**: VCS History で過去 commit を
  選択/Highlight。Oracle: Live project・engine・transport・undo が
  不変。変更は Apply Selected Change の明示 Command のみ。

## 検証戦略と Gate 対応

| Gate | 内容 | 状態 |
| --- | --- | --- |
| headless 契約 | ED-A/ED-B/MD-A/MIDI-Source/WS-A/WSM/VCS/ALIB の既存 unit/integration test | `evidence/sv11/coverage.md` で TE-01〜14 へ写像 |
| 文書整合 | spec/design/test-spec の関係・front matter・index | `doc-maintenance` + `doc-status` |
| 製品 GUI | 実 Window/Pointer/Keyboard/IME/DPI・Snap Guide 描画・2 Window 実競合 | **Deferred — synvelis#112/#114・ruviform#243** |
| 実機/EXT | 実 plugin・実 device・RT Audio 計測・実 OS Drag&Drop | **Deferred — EXT-L3（#114/#103 系）** |
| 数値 PoC | px Hit Size・Snap Threshold/Hysteresis・OS keybind 既定 | **Deferred — Issue #11 §9** |

## 未決定・PoC 後に決める事項（Issue #11 §9 の引き継ぎ）

1. Smart Tool Hit Test の Handle Size・Priority・Drag Threshold・
   Cursor/誤操作対策（Mouse/Pen/Touch 別）— 実装 PoC/#112/#114。
2. Modifier/ショートカットの OS 別既定値、Shift の範囲/複数選択、
   Context Menu/Drag 開始の競合解決 — UI PoC。
3. Snap 単位・Threshold/Hysteresis・Candidate Ranking・Grid 密度と
   Zoom の関係 — UI PoC。
4. Mixed Time Domain 複数編集の明示操作と Snap/TempoMap 編集競合 —
   後続設計（現行は ED-B の定義済み規則を正本）。
5. Time Selection の境界規則、Note/Automation の半開区間と
   Selection の写像 — entity model 追加後。
6. Project Controller の Atomic Commit/Undo History の製品 UI、
   複数 View の競合/失敗 UI、Engine Plan 更新 — #10/#112 実装時。
7. Pinned/Follow Editor と Link Group/Inspector の関係、Linked MIDI
   編集の通知と Content IDs — #5/#12 実装時。
