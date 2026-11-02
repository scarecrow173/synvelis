---
id: "DESIGN-034PCB03Mn8xRs4uZvWdEf"
type: "design"
status: "approved"
title: "PC-B Parameter Control dispatch/note-scope/RT/offline 設計"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/43"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/adr/pc-b-dispatch-boundary-note-entity-prepared-eval.md"
  implemented-by: []
  depends-on:
    - "docs/designs/pc-a-parameter-control-design.md"
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/adr/pc-b-dispatch-boundary-note-entity-prepared-eval.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/adr/pc-b-dispatch-boundary-note-entity-prepared-eval.md"
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/197"
  defers: []
  deferred-by: []
---

# PC-B Parameter Control 設計

## 1. Note entity と scope 解決(`synvelis-project`)

```text
pub struct Note {
    pub id: NoteId,
    pub clip: ClipId,          // 所有 clip(参照整合性)
    pub key: u8,               // 0..=127 — note 同一性
    pub start: BeatPoint,      // clip-relative beats
    pub len: BeatPoint,        // > 0
}
Project.notes: BTreeMap<NoteId, Note>           // schema v4
ProjectOp::AddNote { note } / RemoveNote { note }
```

`resolve_scope_pos` の `ControlScope::Note(id)` arm:

```text
note = project.notes[id]?                       // NotFound
clip = project.clips[note.clip]?                // NotFound
clip_pos = resolve_scope_pos(project, Clip(note.clip), pos)?
           // project→clip-relative(既存 arm 再利用、Musical 前提)
if clip_pos < note.start || clip_pos >= note.start + note.len
    -> ScopeUnavailable("position outside note range")
return clip_pos - note.start                    // note-relative beats
```

- 評価 domain は `[note.start, note.start+len)` の半開区間。
  domain 外は `ScopeUnavailable` — 値を捏造しない。
- `Project::validate_references`: note.clip 存在、start/len>0、
  target/source の `ControlScope::Note` → note 存在、`len` 有限。
  clip 削除時の note 残存は `RemoveClip` 側で note も消す
  (整合性: clip 削除は note cascade 削除 — headless PoC では
  clip の lane から外す既存契約に合わせ note を一緒に削除)。
- note-scoped target は note→clip の参照を pin する:
  `RemoveClip` 適用後も `validate_references` が post-state oracle
  として走るため、note-scoped target(→lane)が残る限り clip 削除は
  atomic に reject される。dangling owner を commit しない既存
  方針と整合 — 削除したい場合は lane→target→clip の順で解除する。
- `AddNote` validate: `key <= 127`、`len > 0`、`start >= 0`、
  clip が musically anchored、かつ `start+len <= clip extent`
  (beats 換算)。clip extent を超える note は作れない。
- `SplitClip`: 分割点を跨ぐ note は commit せず `Validation` で
  拒否(尾を失った domain を残さない)。分割点以降の note は
  clip B へ re-parent し `start` を分割 offset で re-base する —
  評価 domain の絶対位置は保存される。

## 2. 記録経路の scope 正規化(`synvelis-workspace`)

`record_points(&mut self, project, target, points)`:

```text
scope = Self::target(project, target).owner
for (pos, v) in points:
    local = resolve_scope_pos(project, scope, pos)
    Ok(local)  -> seg.push({pos: local, value: v})
    Err(ScopeUnavailable) -> skip   // domain 外は記録しない
    Err(e) -> propagate
```

- `gesture_end(project, target)` / `record_write(project, target,
  at, v)` に `&Project` を追加。`transport_advance` は既に project
  を持つのでそのまま渡す。`session.rs` は `control_with` 経由。
- `build_commit_ops` は変更不要 — transaction 内の点は既に scope
  座標。`Note` 削除で lane points の domain が消えても、書き込み済
  region はそのまま残る(評価時に scope unavailable → 点は到達
  不能になるのみ)。

## 3. Dispatch 経路(`synvelis-player::dispatch`)

```text
pub struct ParamDispatch {
    table: ParamTargetTable,             // TargetId -> (node, param)
    ledger: ParamDispatchLedger,         // Dispatched/Dropped 観測
    control: PluginNodeControl,          // attach() の制御ハンドル
    frame_base: TimeBase,                // plan の frame_base
    gen: PlanGeneration,
}

ParamDispatch::new(control: PluginNodeControl, frame_base)
bind(target: ParameterTargetId, param: ParamId, info: &[ParamInfo])
    -> Result<ParamBinding, DispatchError>
    // table.bind — ParamWriteRejection::UnknownParam 等を
    // DispatchError::WriteRejected へ型保持で変換。
    // 追加 guard: target が u64 bridge id 空間を超えるなら
    // `param_target_out_of_range`、同一 (node, param) が他 target に
    // 所有済みなら `param_in_use`(ledger は (node,param) 軸で
    // 帰属するため共有 bind は誤帰属する)。
resolve_write(target) -> Result<ParamBinding, DispatchError>
    // table.resolve: Unresolved -> WriteRejected("param_unresolved")
    //                Defunct    -> WriteRejected("param_defunct")
dispatch(writes: &[(ParameterTargetId, TimedParamEvent)])
    -> Result<Arc<TransportPlan<EventKind>>, DispatchError>
    // 全 write の binding/param_id を先に検証(param_mismatch で
    // 早期拒否) -> TransportPlanBuilder -> plan
    // -> control.request(Play, 0)       // 先に運ぶ
    // -> control.install_plan(plan, due=0)
    // Play を先に要求するのは atomicity のため: install_plan が
    // QueueFull で落ちても orphan plan が node に残らない
    // (queue された Play は冪等な transport 状態であって staged
    // write ではない)。成功後に ledger.mark_plan_replaced +
    // record_plan + last_plan 保持。
observe(span, call_start, call_frames)
    -> Vec<(ParameterTargetId, ParamValueObservation)>
    // 常に「最後に staged した plan」で project_call — 呼び出し側が
    // 陳腐な plan を渡して誤帰属する余地を API から消す。
    // drain_reports -> apply_report で観測を返す。
```
- `TimedParamEvent.position` は `MediaTime` — plan の frame_base で
  `media_time_ticks` に変換。同一 base ならそのまま。
- PluginNode 自体の RT 配送は bridge が所有(上流で alloc-free)。
  PoC では plan install + Play が dispatch、endpoint 観測が
  成功の外部証拠。

## 4. RT prepared eval(`synvelis-project`)

```text
pub struct EvalPlan {
    pub order: Vec<ParameterTargetId>,          // topo 順(既存)
    prepared: Vec<PreparedTarget>,              // order と同 index
}
struct PreparedTarget {
    base: f64, kind: ValueKind, mapping: ParamMapping,
    min: f64, max: f64,                         // capability range
    series: Box<[(BeatPoint, f64, Interpolation)]>,   // merged
    assignments: Box<[PreparedAssignment]>,
}
struct PreparedAssignment {
    source_idx: usize,        // project.control.sources とは別に
                            // SourceKind を clone 保持
    source: SourceKind,
    depth: f64, polarity: Polarity, transform: Transform,
    depth_source_order: Option<usize>,  // order 内 index or None
}

EvalPlan::compile(project) -> Result<EvalPlan>  // 既存 + prepared 構築
EvalPlan::eval_rt(&self, project, target, pos, manual,
                  values: &mut [f64]) -> Result<f64, Error>
    // 1. order[..=target_idx] を順に評価
    // 2. resolve_scope_pos(成功パス alloc-free) で local pos
    // 3. series binary search → automation または manual/base
    // 4. assignments: source.value(local_pos)、depth_source は
    //    values[driver_idx](topo 順で既計算)
    // 5. values[i] に host_normalized を書き込み、返す
```

- `values` scratch は caller が order.len() で事前確保 — 測定窓の
  alloc 源は排除。エラーパスの `format!` は RT 契約外(成功パスのみ
  測定する、ruviform の assert_no_alloc 慣行どおり)。
- `evaluate_control` は変更しない(丰富な breakdown は non-RT 用)。

## 5. offline render 一致性

- テストで同一 `TimedParamEvent` 列から同一 plan を作り、
  (a) `RtRuntime::process` block drive(playback 相当)と
  (b) `ruviform_audio::render::render()`(offline、実 WAV 出力)
  に同一 `PluginNodeControl` を持つ graph を組んで配送。
- endpoint fixture が (param, absolute frame, value) を記録し、
  両系の観測列を直接比較。block サイズ差は許容(絶対座標で評価)。
- gate: `PluginNode::offline_support()` は upstream で `Unknown`
  (bridge node の offline 等価性は ruviform 側未検証 — rf#197 系統)。
  `render()` は `Unknown` を preflight で拒否するため、テスト側は
  `VerifiedOfflineNode` ラッパ(委譲 + `offline_support()==Verified`
  のみ差し替え)を `NodeFactory` 経由で装着し、この fixture
  インスタンスに限って Verified を宣言する。宣言の検証根拠は
  一致性アサートそのもの(不一致ならテストが落ちる)。
- follow-up: ruviform 側で bridge node の `offline_support` が
  Verified 化されたら wrapper を撤去して裸の `PluginNode` で
  `render()` を通す(ruviform issue 起票候補)。

## 6. Error surface

- `synvelis_project::Error::WriteRejected` は既存。player の
  `DispatchError` は同型(reason token 保持)で、session 上位層が
  `Error::WriteRejected` として扱える形に写像可能。
- schema v4: `notes` を `#[serde(default)]` で追加。v3 body は
  deserialize 可能だが `load()` は厳格に `schema_version == 4` を
  要求し v3 snapshot は `UnsupportedSchema` で拒否 — v1/v2 と同じ
  PoC 段階の no-migration ポリシー(store.rs の SCHEMA_VERSION
  コメントどおり)。`SCHEMA_VERSION = 4`。
