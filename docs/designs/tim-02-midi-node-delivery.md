---
id: "DESIGN-623tim02midiNodeDelivery01"
type: "design"
status: "draft"
title: "TIM-02 MidiClipPlan→plugin node 配送 seam — single-plan merge + instrument node 解決"
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
  implements:
    - "docs/specs/tim-02-midi-node-delivery.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-02-player-midi-rt-offline.md"
    - "docs/specs/tim-02-player-midi-rt-offline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-02 MidiClipPlan→plugin node 配送 seam — single-plan merge + instrument node 解決

## Context

TIM-02 (#126) は `MidiClipPlan`（`plan_midi_clips`）→
`compile_midi_events`（engine-frame `(i64, MidiEvent)` 列）→
`ParamDispatch::dispatch_midi`（generation-tagged plan の
`install_plan(due=0)` staging）までを確立したが、app 生産経路
（`RoutingEngine` → `RoutingControlDispatch`）への配送 seam は
未配線のままだった（`tim-02-player-midi-rt-offline.md` の
Deferred「MIDI clip playback (MidiSource clip → instrument node
routing)」）。本設計はその最終 hop を確定する:

1. node の live plan が単一（upstream `install_plan` wholesale
   replace）という変更不可契約の下での MIDI+param merge 方針、
2. clip → 配送先 plugin/instrument node の解決規則、
3. TIM-03 `ProcessBoundarySession` 経路との整合。

既存事実（調査済み・コード正本）:

- `RoutingControlDispatch` は `attach`/`sync` で
  `take_processor_control`（one-shot vend）を通じ全 plugin
  processor の `ParamDispatch`+`PluginBindingSession` を
  `NodeSurface` に保持する（routing_control.rs `vend_nodes`）。
- `flush_batches`/`fast_control` は
  `PluginBindingSession::dispatch` → `ParamDispatch::dispatch`
  → `install_plan` を呼ぶ — **param batch は tick ごとに live
  plan を wholesale replace する**。別途 install した MIDI plan
  は最初の param flush で消える。これが merge を必須にする
  直接的な制約。
- `TransportPlanBuilder<EventKind>`（upstream ruviform）は
  ParamValue+Midi 混在を許容し、`build()` が
  `(position_ticks, source_sequence)` で安定ソートする
  （transport_schedule.rs:702-759）— upstream 変更不要。
- `record_plan` は `ParamValue` event のみ ledger 記録するため
  merged plan でも ledger は安全（param 観測に影響しない）。
- `dispatch()` は install 前に `Play` を要求する（param path
  契約）。`dispatch_midi` は `Play` を要求しない — `Play` が
  position を再 anchor するため、HeadlessDriver が所有する
  transport anchor 契約を侵さない既存設計。
- `ProcessBoundarySession::dispatch_midi(dispatch, current,
  events)` は provenance 照合 + `ParamDispatch::dispatch_midi`
  委譲の generation-gated 経路（TIM-03, process_boundary.rs）。
- `Processor` には role/instrument フィールドが**存在しない**。
  `SignalDomain` は `Audio` のみ（MIDI port 概念なし —
  routing.rs:957「a control relation is the PC-C2 model, not a
  port」）。plugin descriptor の `features` は自由形式文字列で
  canonical な instrument flag はない。

## Decision 1 — merge semantics: `ParamDispatch` 内の staged MIDI による merged plan

**採用: `ParamDispatch` が `staged_midi: Vec<(i64, MidiEvent)>`
を保持し、mint する全 plan に自動 merge する。**

- `dispatch(writes)`: param event を `source_sequence 0..W`、
  staged MIDI を `W..W+M` で同一 builder に載せ 1 本の
  `TransportPlan<EventKind>` を mint → `install_plan`。
  `staged_midi` が空なら生成 plan は従来と bit 同一（param-only
  非回帰）。
- `dispatch_midi(events)`: `staged_midi` を `events` で置換し
  merged plan を install（Play 要求なし — 既存契約維持）。
- `clear_midi_plan()`: `staged_midi` を空にし **空 plan を
  install** する明示的 clear。plan generation 変化が scheduler
  の discontinuity になり発音中 note は `release_all` で解放
  される（tim-02 design:182-186 調査済み）。`dispatch` 系の
  `Empty` 拒否は「うっかり live plan を空で上書きする」事故の
  抑止であり、意図した clear は別 API に分離する。
- 同 position の ordering: param 先行 → MIDI 後続
  （source_sequence 順で deterministic — 記録上の選択、ドメイン
  要件なし）。

この配置により `PluginBindingSession::dispatch` と
`ProcessBoundarySession::dispatch_params`/`dispatch_midi` は
**無変更で merged 意味論を得る** — merge は plan を mint する
唯一の層（`ParamDispatch`）の不変条件になる。TIM-03 boundary
経路との整合（sv#127 same_area 関係）は構造的に保たれる。

却下した選択肢:

- **順次 install（MIDI plan と param plan を別々に install）**:
  live plan 単一・後勝ち契約の下で先に install した側が消える。
  `flush_batches` が tick ごとに `install_plan` を呼ぶ以上、
  MIDI plan は最初の param flush で失われる — 成立しない。
- **node 役割分離（instrument=MIDI only、processor=param
  only）**: `tim-02-player-midi-rt-offline.md:182-186` の住み
  分け記述は HeadlessDriver 試験世界の doc 契約であり、
  instrument node 自身が automatable param を持ち得る現実
  （filter cutoff の automation 等）を切り捨てる。却下。
- **upstream `install_plan` の multi-plan 化**: 「変更禁止」欄
  に抵触 — 検討対象外。
- **NodeSurface 側に staged MIDI を持ち session/dispatch の別
  API で merge**: plan mint のたびに呼出し側が merge を忘れ得る
  分散設計になり、`ParamDispatch` の public API を直接使う経路
  （`dispatch_params` 等）が silently MIDI を落とす。不変条件を
  mint の所有者に置く方が整合 — 却下。

既知のコスト（受容）: param batch のたびに staged MIDI 全件を
再 mint するため、大きな clip × 高頻度 automation tick で
allocation が増える。non-RT orchestration 契約内では合法;
reuse/diff install は必要になった時点の後続検討とする。

`Play` 契約: `dispatch` は従来どおり install 前に `Play` を
要求し、`dispatch_midi`/`clear_midi_plan` は要求しない
（HeadlessDriver 経路の transport anchor 契約を維持）。
`RoutingControlDispatch` 側の seam が配送時に `Play(due=0)` を
push する — 下記 Decision 3。

## Decision 2 — clip→node 解決規則: strip chain 先頭 processor（instrument slot）

**採用: `clip.lane_id` → `lane.track_id` →
`routing.strip_of_track(track)` → `routing.chains[strip.chain]`
→ `processors[0]`（chain 先頭）を instrument slot とする。**

- DAW の insert chain 慣例（instrument が chain 先頭、effect が
  後続）に一致し、CB-04 `VariantScope::InstrumentSlot` が仮定する
  「clip が feed する slot は chain 上の 1 processor」という語彙
  （sv#281 instrument.rs adapter 文書）と整合する。
- **strict 先頭のみ**: `processors[0]` が surface 上に vended
  control node を持たない（Unresolved binding・static host・未
  vend）場合は **配送しない** — typed evidence（`NoInstrument`
  系）に記録する。「先頭以外で最初に利用可能な node」へ迂回
  配送すると、instrument placeholder が未解決のまま effect に
  note が届く misdelivery になり得るため fail-closed に倒す。
- 複数 clip が同一 node に解決される場合は event を連結して
  1 回 stage する（clip 単位の sequential stage は後勝ちで
  前の clip を消すため）。
- 配送先が解決不能（lane/track/strip/chain 不在、chain 空）は
  `None` → typed evidence。muted clip はそもそも evidence を
  生成しない既存契約（`compile_midi_clip_evidence`）に従う。

却下した選択肢:

- **chain 内の全 Resolved processor に配送**: effect に note
  event が届く — 意味的に誤り。
- **plugin descriptor `features` で instrument 判定**: canonical
  な instrument flag が存在せず（free-form 文字列）、headless
  fixture で何でも名乗れるため authority にならない。
- **`VariantScope::InstrumentSlot` を authority に**: variant
  scope であり永続 designation ではない（タスク分析どおり）。
- **`Processor` に role/instrument フィールドを追加**: project
  model 変更で本 slice の lease 外 — 後続 Issue 候補として記録
  （Deferred Design Concerns）。

## Decision 3 — `RoutingControlDispatch` seam 配置と gating

**採用: 同一 `NodeSurface.dispatch`（`ParamDispatch`）を使う
seam — routing_engine 側の別 vend は作らない。**

`take_processor_control` は one-shot で `attach` が全 processor
を消費済みのため、別 vend は不可能。`NodeSurface` の既存
`ParamDispatch` がそのまま merge/stage の器になる（KISS）。

API:

- `stage_midi_plan(project, processor, events)`: surface
  current 検査（`surface_is_current` — capability/graph pin の
  live project 照合）→ `Play(due=0)` push → boundary session
  あれば `session.dispatch_midi`（provenance gate）、なければ
  `dispatch.dispatch_midi`（同一 NodeSurface の pin は
  construction 上同一 — instance/node は drift 不能）→ 成功時に
  `staged_transport` へ `(Play, 0)` を記録（mirror 契約 —
  `flush_batches` が param `dispatch` の内部 `Play` を記録する
  のと同じ扱い）。
- `clear_midi_plan(project, processor)`: 同検査 →
  `dispatch.clear_midi_plan()`（boundary session に clear 経路が
  ないため `dispatch` 直接 — provenance gate 相当の検査は
  `surface_is_current`+同一 NodeSurface pin で済む。boundary
  session を迂回するのは「wrong node への write 抑止」が目的で
  あり、同一 surface 内の self-dispatch に誤配送面はない）。
- `sync_midi_plans(project, &BTreeMap<ProcessorId, Vec<(i64,
  MidiEvent)>>)`: reconcile API — 列挙 node には stage、
  `staged_midi` を持つが列挙にない node には clear を適用し、
  `MidiStageReport{staged, cleared, errors}` を返す。app 側が
  rebuild ごとに呼び clip 追加・削除・編集を一次処理で reconcile
  する。
- `instrument_node_for_clip(project, clip_id) -> Option<
  ProcessorId>`: Decision 2 の解決規則の公開 helper（app 側は
  clip 単位の evidence 記録に使う）。

gating 方針: staging は `active_generation` を要求**しない**。
clip plan は generation-tagged な持続 content であり、activation
前の pre-stage が意図した使い方（瞬間 write の `fast_control`/
`boundary_stage_window` とは性格が異なる — これらが activation
gate を取るのは「届いた瞬間に発火する」write だから）。install
済み plan は node が generation を activate してから配送される。

`Play` push の根拠: `RoutingControlDispatch` 世界には
transport anchor を所有する driver が存在しない
（`HeadlessDriver` は test 専用）。param path が `dispatch`
内部で `Play` を push する既存慣例に倣い、「clip content の
staging = 再生意図」と解釈して seam が `Play(due=0)` を push
する。`Play` は既に Playing な node では idempotent。Stop 後の
再 anchor で position が 0 に戻る caveat は param path と同一で
あり、この surface で追加の悪化はない（engine cursor との
divergence は transport owner 層の既存責務 — 本 seam は
position anchor を新規発明しない）。

## Data and Control Flow

```text
Project { clips, lanes, tracks, routing, midi_sources }
  │ plan_midi_clips → compile_midi_comp → compile_midi_events
  ▼
MidiClipEvidence { clip_id, source_id, events, error }   [synvelis-app rebuild]
  │ instrument_node_for_clip → strip chain 先頭 processor
  │ （同一 node へ複数 clip は event 連結）
  ▼
RoutingControlDispatch::sync_midi_plans
  │ stage_midi_plan: Play(due=0) → [boundary gate] → dispatch_midi
  │ clear_midi_plan: dispatch.clear_midi_plan（空 plan install）
  ▼
ParamDispatch { staged_midi } → merged TransportPlan<EventKind>
  │ install_plan(due=0) — single live plan, wholesale replace
  ▼
PluginNodeControl → PluginNode.scheduler (RT)
  │ param batch dispatch でも staged_midi を自動 merge
  ▼
EventKind::{Midi, ParamValue} → plugin endpoint (EventList)
```

## Non-Functional Constraints

- RT 契約: `stage_midi_plan`/`clear_midi_plan`/`dispatch`/
  `dispatch_midi`/`clear_midi_plan` は全て non-RT orchestration
  （bounded control channel への push のみ; callback 内の
  allocation-free 契約は upstream `PluginNode` に帰属 —
  `mise run test-rt` で検証）。
- single live plan 契約不変: `install_plan` wholesale replace
  と `TransportPlanBuilder` に upstream 差分なし。
- deterministic: 同 position 順序は `(position,
  source_sequence)` 安定ソート; param→MIDI の seq 配置を固定。
- fail-closed: 解決不能 clip・stale surface・vend なし node・
  stage 失敗は全て typed evidence — silent skip なし。
- param 非回帰: `staged_midi` 空の node の plan 形状は従来と
  同一（fixture で固定）。

## Risks and Trade-offs

- **instrument 判定が chain 先頭慣例**: 先頭に effect を挿した
  project では誤配送になり得るが、strict 先頭+typed evidence で
  検出可能。永続 designation（`Processor` role）は project
  model 変更の後続 Issue — Decision 2 記録どおり。
- **staged MIDI の再 mint コスト**: Decision 1 に記録 —
  param 活動比例。現状受容、差分 install は後続。
- **`Play` push が surface の transport 意味論を増やす**:
  param path と同じ慣例の延長として記録。dedicated transport
  owner（app 層の transport surface）が確立された時点で
  移譲されるべき暫定責務（Deferred Design Concerns）。
- **boundary 迂回 clear**: `ProcessBoundarySession` に clear
  経路がなく `dispatch` を直接呼ぶ — pin 検査は
  `surface_is_current` + 同一 NodeSurface で同等（Decision 3）。

## Deferred Design Concerns

- `Processor` への永続 instrument designation（role フィールド
  等）— project model 変更 | instrument が chain 先頭以外に
  載る UI/model 要件が出た時 | 先頭慣例の誤配送が UX に見える | 中
- dedicated transport owner への `Play` 責務移譲 — app session
  transport（`syn.transport.set`）が node transport を正式に
  所有した時 | staging-side Play push が semantic noise になる |
  中
- merged plan の diff/reuse install（staged MIDI の再 mint
  削減）— 大規模 clip × 高頻度 automation でプロファイル問題
  が観測された時 | 現状の再 mint コストが支配的になる | 低
- `HeadlessDriver` への同 seam 波及 — test 専用 driver が app
  生産経路と別 semantic を要する場合 | 現状 driver 側
  `dispatch_midi` 直接呼出で充足 | 低
