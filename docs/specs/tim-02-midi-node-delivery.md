---
id: "SPEC-623tim02midiNodeDelivery01"
type: "spec"
status: "proposed"
title: "TIM-02 MidiClipPlan→plugin node 配送 seam — merged plan + delivery evidence (synvelis#623)"
created: "2026-11-23"
updated: "2026-11-23"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/623"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/tim-02-midi-node-delivery.md"
  depends-on:
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/specs/tim-02-app-ops-smf.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/623"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-02-midi-node-delivery.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-02 MidiClipPlan→plugin node 配送 seam — merged plan + delivery evidence (synvelis#623)

## What Should Be Built

Issue #623（TIM-02 後続）: compile 済み `MidiClipPlan` が対象
plugin/instrument node に install される最終 hop を、app 生産
経路（`SessionRoutingEngine::rebuild` → `RoutingControlDispatch`）
に配線する。merge 方針・解決規則の設計判断は
`docs/designs/tim-02-midi-node-delivery.md`（status:draft）が
決裁材料を保持し、実装はその方針と一致する。

## Scope

### Player seam（`crates/synvelis-player`）

- **`ParamDispatch` merged plan**（`dispatch.rs`）:
  `staged_midi: Vec<(i64, MidiEvent)>` を保持し、`dispatch`
  （param batch）が mint する全 plan に staged MIDI を merge
  する。upstream `install_plan` の single-live-plan
  wholesale-replace 契約は不変 — staged MIDI が空の node の
  plan 形状は従来と同一（param 非回帰）。
- **`dispatch_midi(events)`**: staged 集合を `events` で置換し
  merged plan を install（`Play` を要求しない既存契約を維持）。
- **`clear_midi_plan()`**: staged 集合を空にして空 plan を
  install する明示的 clear（`dispatch` 系の `Empty` 拒否とは
  別 API — 意図した消去と「うっかり空上書き」を分離する）。
- **`RoutingControlDispatch` midi seam**（`routing_control.rs`）:
  - `stage_midi_plan(project, processor, events)`: surface
    current 検査 → `Play(due=0)` push → boundary session が
    あれば `ProcessBoundarySession::dispatch_midi`（provenance
    gate）、なければ `NodeSurface.dispatch.dispatch_midi`。
    成功時に `staged_transport` へ `(Play, 0)` を mirror 記録。
  - `clear_midi_plan(project, processor)`: 同検査 →
    `dispatch.clear_midi_plan()`。
  - `sync_midi_plans(project, &BTreeMap<ProcessorId, Vec<(i64,
    MidiEvent)>>) -> MidiStageReport`: reconcile API — 列挙
    node に stage、列挙外で staged を持つ node に clear を
    適用し `{staged, cleared, errors}` を返す。
  - `instrument_node_for_clip(project, clip_id) -> Option<
    ProcessorId>`: `clip.lane_id` → `lane.track_id` →
    `routing.strip_of_track(track)` → `chains[strip.chain]`
    → `processors[0]`（chain 先頭 = instrument slot）の strict
    解決。途中リンク欠落・空 chain は `None`。
  - `MidiStageError` / `MidiStageReport`: 型付き失敗
    （`NoNode`・`StaleSurface`・`Boundary`・`Dispatch` —
    `BoundaryStageError` 慣例に倣う）。

### App 配線（`crates/synvelis-app`）

- **`MidiClipDelivery`**（`engine.rs`、crate-public・
  `#[non_exhaustive]`）: `NotCompiled` / `Unresolved` /
  `Staged(ProcessorId)` / `Failed { processor, error }` の
  正直な配送結果 evidence。
- **`MidiClipEvidence.delivery`**: compile 成功は `Unresolved`
  初期値、失敗は `NotCompiled` で mint し、rebuild 内の
  `stage_midi_delivery` が clip ごとに書き戻す。
- **`stage_midi_delivery`**: `rebuild` の末尾で毎回実行 —
  `Built` publish 後の新 generation、`Equivalent`（clip 内容
  変更のみ）時の継続 generation、build 失敗時の last-good
  surface のいずれでも reconcile する。clip → node の解決で
  同一 node に複数 clip が集約される場合は event を連結して
  1 回の `sync_midi_plans` で reconcile（順次 install は後勝ち
  で前の clip を消すため禁止）。
- **capabilities `midi.player_clip`**（`app.rs`）: 配送 seam
  配線済みであることを正直表示する（headless 実走のみを記録、
  実 plugin 受入は外部 gate のため `'partial'` を維持）。

## Acceptance Criteria

1. compile 済み MidiClipPlan が対象 plugin/instrument node へ
   install され、headless 検証で配送が確認できる。
   - player: `tim02_midi_node_delivery.rs` の
     `clip_events_deliver_to_chain_head_node` が fake
     instrument の `EventList` ログを oracle に配送を証明。
   - app: `tim02_midi_delivery.rs` の
     `compiled_clip_stages_on_chain_head_instrument` が
     `MidiClipDelivery::Staged(processor)` evidence と
     `engine.run` 後の engine-frame 一致配送を証明。
2. node live plan merge の設計判断が
   `docs/designs/tim-02-midi-node-delivery.md` に記録される
   （status:draft — 選択肢・採用理由・却下理由・single-plan
   契約との整合）。
3. RT 契約（callback 内非干渉）と既存 routing plan 非回帰が
   検証される — seam/dispatch は non-RT orchestration
   （bounded control channel への push のみ）に留まることを
   `mise run test-rt` で固定し、param-only node の plan 形状が
   変わらないことを fixture で固定する。

## Out of Scope

- upstream ruviform 変更・`install_plan` wholesale-replace
  契約の変更。
- 実 Plugin GUI/実機での MIDI 動作受入（L3 gate — sv#176）。
- midi clip タイミング精度保証の再設計（TIM-03 boundary 契約
  自体の変更はしない — provenance gate 経路は既存のまま使う）。
- offline render 経路の MIDI 再生（synvelis-render は
  `UnrenderableClip` で拒否する既存契約を維持）。
- synvelis-gateway への midi install op 公開。
- `HeadlessDriver` への seam 波及（test 専用 driver — app 生産
  経路は `RoutingControlDispatch`）。
- `Processor` への永続 instrument designation（project model
  変更 — design の Deferred Design Concerns に記録）。
- docs index（`docs/**/README.md`）の編集・再生成。
