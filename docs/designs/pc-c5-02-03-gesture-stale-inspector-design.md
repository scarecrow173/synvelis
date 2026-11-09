---
id: "DESIGN-034UxXM5FBBTGn5JWkSBss"
type: "design"
status: "approved"
title: "PC-C5 slice2 Gesture 固定・stale drop・Inspector mode 設計"
created: "2026-11-09"
updated: "2026-11-09"
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
  implements:
    - "SPEC-034UxXLvNqgbDGOypo7Uu4"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "SPEC-034UxXLvNqgbDGOypo7Uu4"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice2 Gesture 固定・stale drop・Inspector mode 設計

## 概要

`synvelis-workspace` に 2 つの独立した仕組みを追加する。

1. **Inspector mode 状態**（state/link/workspace）: `InspectionMode::Explore`、
   `FollowSource`、explore 履歴を `InspectionFocus` に追加し、
   link deliver と inspector navigation API を mode/source 照合付きにする。
2. **Control Layer edge gesture**（新規 `control_edge.rs` + workspace API）:
   `ModulationSource → ParameterTarget` の connect を Begin/Preview/Commit/Cancel で
   固定 semantics に乗せ、window epoch と project revision で stale を drop する。

両者とも既存の `gesture.rs`（Audio Routing = clip/lane 編集 gesture）・
`controls_ui.rs`（parameter gesture、generation 照合済み）とは別経路であり、
`synvelis-project` への変更は `ProjectOp`/`ProjectSession` の既存消費のみ。

## 詳細設計

### 1. Inspector mode（state.rs / link.rs / workspace.rs）

#### 型

```rust
pub enum InspectionMode { Follow, Explore, Pinned }

/// Inspector が Follow で追従する対象の出所。
/// View = 指定 view の editing selection の primary、Group = link group。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FollowSource {
    View(ViewInstanceId),
    Group(LinkGroupId),
}

pub struct InspectionFocus {
    pub target: Option<SemanticTarget>,
    pub mode: InspectionMode,
    #[serde(default)]
    pub follow_source: Option<FollowSource>,
    #[serde(default, skip)]           // session-only: preset/復元に持ち越さない
    pub explore_back: Vec<SemanticTarget>,
    #[serde(default, skip)]
    pub explore_fwd: Vec<SemanticTarget>,
}
```

- `explore_back`/`explore_fwd` は session-only（Issue #5「Explore の履歴は既定で
  リセット」）。`PresetViewState` は mode のみ永続化する既存方針を維持。
- `follow_source` は serde 永続化する（follow 設定は view の性質であり session
  状態ではない）。`None` = follow 無効（Pinned 相当の追従なし）。

#### link.rs deliver の変更

現行: `mode == Follow` なら無条件で `inspection_focus.target` を更新。
変更後:

- `linked_context` の更新は従来通り全 receiver に適用（mode 不問）。
- `inspection_focus.target` の更新は以下を全て満たす場合のみ:
  - `mode == Follow`
  - `follow_source` が `Some` で、deliver された link group/対象と一致する
  - editing selection（`object_selections`）は一切変更しない（既存維持）

`follow_source` の照合規則:
- `FollowSource::Group(g)` → deliver された `LinkGroupId == g`
- `FollowSource::View(v)` → deliver の発生源 view が `v`（link 経由の伝播元）

#### workspace.rs inspector API

```rust
pub fn inspect(&mut self, view: ViewInstanceId, target: SemanticTarget)
    -> Result<(), WorkspaceError>
pub fn set_inspection_mode(&mut self, view: ViewInstanceId, mode: InspectionMode)
    -> Result<(), WorkspaceError>
pub fn set_follow_source(&mut self, view: ViewInstanceId,
    source: Option<FollowSource>) -> Result<(), WorkspaceError>
pub fn explore_back(&mut self, view: ViewInstanceId) -> Result<(), WorkspaceError>
pub fn explore_forward(&mut self, view: ViewInstanceId) -> Result<(), WorkspaceError>
```

- `inspect`: `mode` を `Explore` に遷移させ、現在 target を `explore_back` に push、
  `explore_fwd` を clear、`target` を新対象に。editing selection・project 不変。
- `set_inspection_mode`: Follow→Pinned 等の遷移。Follow に入る際 `follow_source`
  が `None` なら view 自身の selection primary を source として推定しない
  （明示設定のみ — 暗黙の retarget を避ける）。Pinned/Explore への遷移で
  follow_source は保持（Follow 復帰時に再利用）。
- `explore_back`/`explore_forward`: `mode == Explore` のときのみ履歴を辿る。
  戻る先が無ければ no-op。各 step で `explore_back`/`explore_fwd` を更新。
- 全 API は `states.get_mut(view)` が無ければ `UnknownView`。

### 2. Control Layer edge gesture（control_edge.rs 新規）

#### 型

```rust
/// Begin 時に固定される Control Layer edge gesture の fixation。
/// Audio Routing（gesture.rs の GestureFixation）とは別型。
#[derive(Clone, Debug)]
pub struct ControlEdgeFixation {
    pub view: ViewInstanceId,
    pub source: ModulationSourceId,     // typed — ControlEntityRef ではない
    pub target: ParameterTargetId,      // typed
    pub revision: ProjectRevision,      // CAS 用 expected revision
    pub window: Option<WindowId>,       // Begin 時の window epoch
}

pub struct ControlEdgeGesture {
    pub fixation: ControlEdgeFixation,
    pub preview: ControlEdgePreview,    // depth/polarity 等の編集途中値
}

pub enum ControlEdgeOutcome {
    Applied { revision: ProjectRevision },
    Cancelled,
    Stale,        // window epoch 移動 or revision CAS 失敗 → drop
    Rejected { error: WorkspaceError },
}
```

- fixation は Begin 時に `view`・両 endpoint の typed id・`session.revision()`・
  `dock.owner_of(view)` を固定。以降どの入力でも fixation の値を使う。
- `ControlEntityRef`（relation_ui の UI anchor）から typed id への解決は Begin で
  1 回だけ行い、不正 kind（source が Parameter、target が ModulationSource 等）は
  `Rejected`。これが「UI selection ≠ Command Target」の型レベル区別の実装点。

#### workspace API

```rust
pub fn control_edge_begin(&mut self, view: ViewInstanceId,
    source: ControlEntityRef, target: ControlEntityRef)
    -> Result<(), WorkspaceError>
pub fn control_edge_preview(&mut self, view: ViewInstanceId,
    depth: f64, polarity: Polarity) -> Result<(), WorkspaceError>
pub fn control_edge_commit(&mut self, view: ViewInstanceId)
    -> ControlEdgeOutcome
pub fn control_edge_cancel(&mut self, view: ViewInstanceId, reason: CancelReason)
    -> Result<(), WorkspaceError>
pub fn control_edge_gesture(&self, view: ViewInstanceId)
    -> Option<&ControlEdgeGesture>
```

- Begin: 既存 `gesture`（Audio Routing）や `control_edge` が走っていれば
  `GestureActive`。endpoint kind 検証 → typed id 解決 → fixation 固定 →
  `ViewState.control_edge_gesture = Some(...)`。
- Preview: fixation の gesture があれば preview 値を更新。project 不変。
  gesture が無ければ `NoGesture`/`Rejected`。
- Commit: fixation を取り出して即座に `control_edge_gesture = None`（1 回限り）。
  - `dock.owner_of(view) != fixation.window` → `Stale`（window 移動済み）。
  - `session.apply(AddModulationAssignment{...}, fixation.revision)` が
    `Err(Conflict)` → `Stale`（revision 前進済み）。その他 error → `Rejected`。
  - `Ok(rev)` → `distribute_revision(rev)`（全 view の revision_inbox + render）。
    preview 値（depth/polarity/transform=Identity/depth_source=None/enabled=true）
    を assignment に使用。undo 境界は `session.apply` が 1 project op = 1 境界。
- Cancel: gesture を破棄。project 不変。`reason` は既存 `CancelReason` を再利用。

#### stale drop の網羅

| 事象 | 検出点 | 結果 |
| --- | --- | --- |
| dock transfer / window 移動 | `owner_of(view)` vs `fixation.window` | commit → `Stale` |
| 別経路の project 変更 | `session.apply` の expected revision CAS | commit → `Stale` |
| view detach | `states.get(view)` 喪失 or owner 変化 | commit → `Stale`/`Rejected` |
| focus/selection 変更 | fixation は Begin 時値を保持 | 影響なし（retarget しない） |

### 3. 既存機構との関係（変更しないもの）

- `controls_ui.rs`: generation 照合・stale diagnostics は既に完備。本 slice では
  fixture でその既存保証を再確認するのみ（必要なら test 追加）。
- `gesture.rs`/`edit.rs`/`tool.rs`/`command.rs`: Audio Routing gesture と
  command routing は変更しない。inspect≠edit は「inspect API がこれらを
  呼ばない」ことで保証し、fixture で証明。
- `session.rs`: `apply`/`undo`/`redo` は既存のまま。undo が project 変更時のみ
  発火することは `session.apply` 成功時のみ revision が進む構造で保証済み。

### 4. ファイル変更一覧

| ファイル | 変更 |
| --- | --- |
| `src/state.rs` | `InspectionMode::Explore`、`FollowSource`、`InspectionFocus` 拡張 |
| `src/link.rs` | deliver の mode/source 照合 |
| `src/workspace.rs` | inspector API 5 本 + control edge API 5 本 + `ViewState` 参照 |
| `src/control_edge.rs` | 新規: fixation/gesture/outcome |
| `src/lib.rs` | `control_edge` module export |
| `src/preset.rs` | `into_view_state` の `InspectionFocus` 構築更新（新 field） |
| `tests/uic5.rs` | 新規 fixture |

`state.rs` の `ViewState` に `control_edge_gesture: Option<ControlEdgeGesture>`
（`#[serde(skip)]` — session-only）を追加。

## リスクと対策

- `PresetViewState::into_view_state` が `InspectionFocus` を再構築する箇所の
  追従漏れ → `cargo check` で検出、serde default で後方互換。
- `FollowSource::View` の照合で link deliver の発生源 view が取れない場合 →
  deliver シグネチャを確認し、取れなければ `Group` のみサポートに縮小
  （設計時に link.rs を再確認）。
- control edge commit の `AddModulationAssignment` が duplicate assignment を
  拒否する場合 → `Rejected` として返し fixture で確認。
