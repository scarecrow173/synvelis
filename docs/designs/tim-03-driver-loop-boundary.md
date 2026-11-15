---
id: "DESIGN-034XI7yLnh4SLMxH8BU2lg"
type: "design"
status: "approved"
title: "TIM-03 driver-side loop-wrap boundary derivation"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim-03-driver-loop-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-loop-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-03-loop-boundary.md"
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

# TIM-03 driver-side loop-wrap boundary derivation — Design

## Overview

`AttachedDispatch`（`crates/synvelis-player/src/driver.rs`）に
scheduler-faithful な `WrapSim`（`process_boundary.rs`、routing 側と
同一の `pub(crate)` 実装 — 複製しない）を追加し、driver の
`stage_window` 3 呼出点へ位置由来 `LoopWrap` boundary を供給する。
routing 側 `NodeSurface`/`boundary_loop_wraps` の契約を driver の
観測ループへ写像するのが本設計の核心であり、routing 側コードは
一切変更しない。

## State model

`AttachedDispatch` に追加する field:

| field | role |
| --- | --- |
| `wrap_sim: WrapSim` | `WrapSim::new(dispatch.frame_base())` で初期化。scheduler-equivalent `(status, anchor_frame, anchor_pos, loop_ticks)`。 |
| `sim_window: u64` | 次の node callback window の先頭 frame — **node cursor ドメイン**。`observe_attached_block` ごとに `block_frames` 進み、`set_clock` 成功時に即座に re-anchor 先へ更新される。seek quiesce pass で driver `cursor` より先行するため独立に追跡する。 |
| `staged_transport: Vec<TransportCommand>` | node に queue 済み・未観測の transport command。driver command は全て `due = 0` のため、次 callback head（= 観測時の `sim_window`）に push 順で apply される。routing の `staged_transport` と同じ役割。 |
| `last_window_plan: Option<ProcessBoundaryPlan>` | 最後の `stage_window` が評価した plan — `attached_window_plan(node_id)` accessor で公開する診断証拠（`last_span` と同型）。 |

`TransportMirror` は従来どおり observed-span context と generation
receipt 補正の責務を維持する。`WrapSim` は wrap 導出専用で、
mirror の lazy-fold では再現できない `SetLoop`/`ClearLoop` 越境の
`scheduler pos_at` を担当する — 責務を混ぜない。

## Walk protocol

- **Preview（read-only）**: `AttachedDispatch::loop_wrap_boundaries`
  が `wrap_sim` を clone し、`staged_transport` + 今回 stage する
  command を全て `(sim_window, cmd)` の pending として
  `walk_window(sim_window, window_frames, &pending)` を呼ぶ。
  due は全て window head なので head-apply 規則で正しく畳まれる。
  戻り値の `Vec<BoundaryPoint>`（`LoopWrap` のみ）をそのまま
  `stage_window` の `boundaries` に渡す。live sim は触らない。
- **Advance（1 回だけ）**: `observe_attached_block` が各 node につき
  `walk_window(sim_window, call_frames, pending)` を実行し、
  `sim_window += block_frames`、`staged_transport.clear()`。
  preview で進めた分をここで実際に消費するため二重進行しない。

## Call-site mapping

| site | 変更 |
| --- | --- |
| `prepare_attached` → `AttachedDispatch::anchor_link` | `set_clock` 成功後に `sim_window = cursor`。sessioned 経路は preview → `stage_window(boundaries, [(Seek,0),(Play,0)])` → `staged_transport.extend` → `last_window_plan`。legacy 経路も `staged_transport` に記録（mirror と同じ「実際に queue できたものだけ」規則）。 |
| `stage_node_transport`（`stage_transport`/`set_loop`/`clear_loop`/`panic` の共通路） | preview に `[command]` を加えて `stage_window`；成功後 `staged_transport.push` + `last_window_plan`。legacy 経路は push のみ。 |
| `stage_seek` | sessioned: `set_clock` 成功後 `sim_window = frame` → preview → `stage_window` → push。legacy: `set_clock` 成功時のみ `sim_window` 更新、queue 失敗は従来どおり早期 return。 |
| `observe_attached_block` | 冒頭で wrap_sim advance（上記 protocol）。span 投影・receipt drain は不変。 |
| `resume_attached_transport` | `attached.anchor_link(epoch, cursor, window)` に集約（旧 `Self::anchor_link` 自由関数を method 化）。 |

## Invariants

- `sim_window` は node が次に処理する window の先頭 — `set_clock`
  成功で cursor へ即時追従、観測ごとに `block_frames` 進む。
  driver `cursor` とは seek quiesce の間だけ乖離し、re-anchor で
  再同期する。
- 全ての node 行き transport command は `staged_transport` に
  push 順で記録され、次の `observe_attached_block` で due=head の
  pending として 1 回だけ walk に消費される。`set_clock` は
  command ではなく cursor rebase なので `sim_window` 直接更新で
  表現する。
- `plan.boundaries` は oracle 正規化済み（sort/dedup、offset 0 の
  head boundary は applied set から除かれる）— driver は
  `BoundaryPoint` を供給するだけで正規化を再実装しない。
- 失敗は fail-closed: oracle refusal / `QueueFull` は
  `BoundarySessionError` のまま伝播し、sessioned node を legacy に
  降格しない。`boundary: None` node は既存 lenient 経路のまま。
- 既知の盲点: `ParamDispatch::dispatch`（params write 経路）が
  node queue に直接積む内部 `Play` は driver の
  `staged_transport` を通らない。node が `Playing` の間は
  re-anchor が位置 no-op のため無害だが、`panic` 後
  `resume_attached_transport` 前の非 playing 状態で param dispatch
  すると wrap 導出が古いまま残る。routing は自らの call site で
  この `Play` を `staged_transport` に補記するが、driver 経路の
  call site は consumer 所有の accessor（`attached_dispatch` /
  `attached_boundary`）のため driver.rs 内では捕捉できない —
  doc comment で契約を明示し、根本対応（dispatch 層からの
  command 可視化）は別 slice の検討事項とする。

## Testing strategy

`tests/tim03_driver_loop_boundary.rs`（DB-01..DB-08、
`docs/test-specs/tim-03-driver-loop-boundary.md`）: recording plugin
が各 physical `process` call の folded `TransportInfo.position` と
frame 数を記録し、call start 累積で window 内 span cut を再構成。
`attached_window_plan(node).boundaries`（oracle が実際に見た applied
offset）と interior cut が一致することを、activation anchor・
steady loop・window-end/head wrap・SetLoop 変更・ClearLoop・
Seek cascade・non-playing・sessionless node の各経路で検証する。
