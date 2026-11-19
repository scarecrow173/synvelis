---
id: "SPEC-034XzM1qSQBS0dtcltnMBI"
type: "spec"
status: "approved"
title: "TIM-03 Loop/Tempo boundary 自動導出 — TransportMirror scheduled commands からの LoopWrap BoundaryPoint 導出（headless slice）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/ruviform/issues/190"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
  defers: []
  deferred-by: []
---

# TIM-03 Loop/Tempo boundary 自動導出 — TransportMirror scheduled commands からの LoopWrap BoundaryPoint 導出（headless slice）

## What Should Be Built

Issue #127（TIM-03 Plugin Process 境界／Loop・Tempo・Automation
精度 Capability/Degraded Policy）のうち、process boundary oracle
（`docs/specs/tim03-process-boundary.md`）と session→driver/
routing wiring（`docs/specs/tim-03-session-driver-wiring.md`、
PR #308 統合済み）に続く slice を実装する。`plan_window`/
`stage_window`/`stage_plan` は `boundaries: Vec<BoundaryPoint>`
を caller 供給とし、command 由来 point のみ自動付加する現状を
改め、**window 内の loop wrap `BoundaryPoint{LoopWrap}` 列を
`TransportMirror` の commanded state と scheduled transport
commands から非 RT で自動導出する**。

- **`RoutingControlDispatch::boundary_loop_wraps`**:
  `NodeSurface` の `TransportMirror`（commanded status・
  position anchor・loop region）と `scheduled_transport`
  （absolute apply frame 解決済み）+ `staged_transport`
  （callback-head-relative due）を読み、upcoming window
  `[call_start, call_start + call_frames)` 内で発生する
  loop wrap の window-relative offset 列を
  `BoundaryKind::LoopWrap` の `BoundaryPoint` として返す
  純粋な読み取り API。導出結果は caller が既存の
  `boundary_plan_window`/`boundary_stage_window` の
  `boundaries` 引数へそのまま渡せる。
- **導出アルゴリズム**: scheduler と同一の scalar
  transport state（status・anchor・loop region）を
  `WrapSim` として各 control node に維持し、
  `observe_node_block` が観測する全 window を同一
  pending command 列で walk して進める。導出呼出時は
  sim を clone して upcoming window の head で
  再錨定し、window 内 apply frame をフレーム順に
  畳み込みながら `next_wrap`（Playing かつ loop
  設定時に raw position が loop end に到達する
  frame）を逐次評価する。apply は同 frame の wrap
  より先に適用される（upstream scheduler の tie 規則と
  同一）。導出されるのは
  **要求された** wrap frame であり、degraded deferral の
  実適用 frame は従来どおり oracle/scheduler 側の責務
  （`LoopWrap` point は deferral scan を bound しない
  既存規則を維持）。
- **frame-linear position domain の明示**: 本 PoC の
  position domain は `position_base == frame_base`（engine
  frame、1 tick == 1 frame）であり、tempo map は musical
  情報専用で wrap frame を動かさない。plan install
  （tempo revision 変更を含む）は transport scalar state
  （status/anchor/loop）を変えないため導出に影響しない
  — この不変条件をコードと文書に明記し、fixture で
  plan/tempo install 跨ぎの wrap 位置不変を検証する。

## Why Now

`process_boundary.rs` の window contract は「position-derived
loop wraps are caller-supplied; tempo/loop offset auto-derivation
is a documented non-goal」(process_boundary.rs:963-964) と
明記していた。caller 供給の wrap 列は呼び出し側が transport
状態を再実装しなければならず、mirror が既に保持する
commanded state・scheduled commands との二重管理になる。
TIM-03 session→driver/routing wiring（PR #308）で
`TransportMirror` が `NodeSurface` に配線済みのため、
wrap 導出を composition root の非 RT orchestration に
集約できる時期にある。Loop/Tempo/Automation の同時
イベント順序と Process Length を Fake Plugin で再現する
受入条件のうち、loop wrap の boundary 供給を自動化する
slice として本 Issue の残件を前進させる。

## Users and Value

- **composition root / RT run loop の caller**: upcoming
  callback window の `BoundaryPoint` 列を transport 状態から
  自動導出でき、loop wrap の手動列挙・再実装が不要になる。
  `boundary_stage_window` へ渡す `boundaries` が常に
  mirror/scheduler と一致した最新状態になる。
- **process boundary oracle の利用者**: loop wrap が
  window 内の正しい offset で legality 検査され、degraded
  配送時の deferral 判定・receipt 記録が実際の wrap frame
  と一致する。
- **後続の capability/precision certification slice**:
  wrap 導出が authoritative な一箇所に集約され、consumer
  側の照合（AL f64/native PCM）や実 plugin/OS matrix の
  前提となる boundary 供給が安定する。

## Scope

### In Scope

- `crates/synvelis-player/src/process_boundary.rs`:
  window 内 loop wrap を導出する非 RT 純粋関数
  （scalar transport state + pending applies →
  `Vec<BoundaryPoint{LoopWrap}>`）。scheduler の
  `pos_at`/`next_wrap`/`wrap`/command apply と同一の
  algebra を plain values 上に実装。
- `crates/synvelis-player/src/routing_control.rs`:
  `RoutingControlDispatch::boundary_loop_wraps`
  （surface/provenance ゲート + wrap_sim clone +
  scheduled/staged transport の収集 + 導出呼出）。
- `crates/synvelis-player/tests/tim03_loop_boundary.rs`
  （新規）: Fake Plugin + 実 `RoutingEngine`/
  `RoutingControlDispatch` 経路で、導出された wrap offset
  が実 scheduler の wrap frame と一致することを検証。
- `docs/` の spec/design/plan/task/test-spec/impl 文書
  チェーンと `evidence/sv127/` の検証証拠。

### Out of Scope

- consumer 照合（AL f64/native PCM 出力の独立 Oracle）—
  別 slice・外部 gate。
- 実 CLAP/VST3/AU plugin・実 OS matrix・実 GUI・
  precision certification/CapabilityMatrix — 別 slice・
  要設計（Issue #127 の独立受入 Gate）。
- `TransportMirror`/`TransportScheduler` 本体の変更
  （Ruviform 側の責務。mirror は commanded-state
  projection の既存契約をそのまま消費する）。
- driver.rs（`HeadlessDriver`）側の wrap 導出配線 —
  本 slice は `RoutingControlDispatch` 経路のみ。
  driver 側 consumer は後続 slice で同一 helper を
  再利用できる設計とする。
- plan install の scheduled tracking — 現行配線では
  install は head apply（due=0）で transport scalar
  state を変えないため wrap 導出に影響しない。
- `Cargo.toml`/`Cargo.lock`/`mise.toml` の変更
  （依存追加なし）。
- 他 crate・他 test file・他 src file の変更。

## Requirements

- 導出関数は非 RT orchestration のみ: mirror/scheduled
  state の読み取りと pure integer math で構成し、
  RT callback 経路・plugin process には一切触れない。
- window 契約は既存 `observe_block`/`boundary_*` と同一:
  `call_start: EngineFrame`（node cursor domain）+
  `call_frames: u32` で upcoming physical window を記述し、
  返す `BoundaryPoint.offset` は window-relative
  （`offset < call_frames`、offset 0 は head apply）。
- scheduled commands は upstream scheduler と同一の
  validity 規則で畳み込む: `Seek` は `media_time_ticks`
  で非負 ticks のみ、`SetLoop` は非空 region
  （`end > start`、start 非負）のみ有効。無効 command
  は apply されず wrap 計算に影響しない
  （`note_transport_command` と同一規則）。
- 同一 frame の apply と wrap が衝突した場合、apply が
  先に適用され wrap は新 state で再評価される
  （scheduler の tie 規則）。同一 offset の wrap が
  cascade した場合も `BoundaryPoint` は offset ごとに
  1 件へ coalesce される。
- loop 未設定・`Paused`/`Stopped`・loop region が
  不正（`end <= start`）の場合は wrap を導出しない
  （empty）。
- 導出結果は要求された wrap frame の列であり、
  degraded deferral による実適用 frame ではない。
  `BoundaryKind::LoopWrap` point は既存 oracle の
  deferral 規則（apply-kind boundary のみ scan を
  bound する）を変更しない。
- `position_base == frame_base`（1 tick == 1 engine
  frame）の frame-linear domain でのみ正しい —
  tempo map revision は musical 情報であり wrap frame
  を動かさないことを前提として明記する。
- surface gating は既存 `boundary_plan_window` と同一:
  未 attach は `NoSurface`、pin 不整合は `StaleSurface`、
  processor の control node 不在は `NoSession`。
  boundary session 未宣言の node でも wrap sim は
  全 vended control node で維持されるため導出自体は
  可能とする（session を要求しない pure read）。
- 導出は `observe_node_block` が sim を post-window
  state へ進める**前**の pre-window state を読む —
  caller は upcoming window に対して `observe_block`
  の前に呼ぶ契約とし、文書に明記する。

## Acceptance Criteria

Issue #127 本文の受入条件を本 slice で検証可能な行に
写像して採用する。実 VST3・実 OS・GUI・consumer 照合は
対象外で `NotRun`/`Unsupported` を明記する。

- [ ] 100/128/256 frame・block 境界 0/1/100/last の
  loop wrap offset を Fake Plugin + 実
  `RoutingControlDispatch` 経路で導出し、実 scheduler
  が観測する wrap frame（plugin `TransportInfo` の
  position fold）と一致することを検証。
- [ ] scheduled `SetLoop`/`Seek`/`Play`/`Pause`/`Stop`/
  `ClearLoop` が window 内にある場合、apply 前後で
  wrap 位置が正しく遷移することを検証
  （例: window 中盤の `SetLoop` で旧 region の wrap
  が消え新 region の wrap が現れる、`Seek` で wrap
  offset が再計算される）。
- [ ] SetLoop 未設定・loop clear・非 Playing status
  で導出が empty になることを検証。
- [ ] plan/tempo install（plan generation・tempo
  revision 変更を伴う dispatch）跨ぎで wrap 位置が
  不変であることを検証 — frame-linear position
  domain の不変条件を fixture で確認。
- [ ] window 端の wrap: window 最終 frame
  （offset `call_frames - 1`）の wrap は導出され、
  window end ちょうど（offset `call_frames`）の
  wrap は次 window の責務として導出されないことを
  検証。
- [ ] 導出結果を `boundary_plan_window`/
  `boundary_stage_window` の `boundaries` へ供給し、
  oracle が loop wrap を含む window を正しく
  legality 検査することを検証（既存経路との合成）。
- [ ] `cargo`/`mise` 正規タスク（fmt-check/check/
  lint/test、対象 package `synvelis-player`）が
  pass。RT 非回帰・依存非回帰を確認（本 slice は
  非 RT orchestration のみ追加し RT 経路を変更
  しない）。

## Deferred Design Concerns

- consumer 照合（AL f64/native PCM 出力の独立
  Oracle）| 本 slice は boundary 供給のみ・出力 PCM
  照合は別 Issue | consumer 照合 slice 着手時 |
  boundary が正しくても出力精度未検証のまま
  Pass と誤認する誤り。
- 実 CLAP/VST3/AU・OS 別 Plugin Process・実
  Window/GUI・実 Host crash の L2/L3 受入 | 実
  plugin/OS/Format が必要・headless slice の範囲外 |
  実利用機能の受入時 | Fake Plugin 証拠を実 plugin
  証拠に換算する誤り。
- precision certification/CapabilityMatrix・
  degraded policy の数値閾値確定 | Issue #127 の
  独立設計 Gate・実測データが必要 | certification
  slice 着手時 | 未測定 precision を推測で
  格上げする誤り。
- `HeadlessDriver` 側の wrap 導出配線 | 本 slice は
  `RoutingControlDispatch` 経路のみ・driver は
  別 consumer | driver boundary slice 着手時 |
  driver 経路の caller 供給 wrap が残存する。

## Sources and Relations

- Issue #127（対象 Issue、受入条件・Capability/
  Degraded Policy 語彙の正本）。
- `docs/specs/tim03-process-boundary.md`（process
  boundary oracle — `BoundaryPoint`/`BoundaryKind`/
  `ProcessCapability::evaluate`/`ProcessBoundarySession`
  の既存契約を消費・変更しない）。
- `docs/specs/tim-03-session-driver-wiring.md`
  （session→driver/routing wiring — `NodeSurface.
  transport`/`staged_transport`/`scheduled_transport`
  の配線元、PR #308）。
- Ruviform #190（`TransportScheduler`/`TransportPlan`/
  `SpanContext` — `pos_at`/`next_wrap`/`do_wrap`/
  command apply の upstream semantics を投影する
  mirror の根拠。複製しない）。
- `crates/synvelis-player/src/driver.rs`
  `TransportMirror`（commanded-state projection —
  `position_at`/`span_context`/`note_*` の既存契約を
  消費。変更しない。wrap 導出は mirror ではなく
  `WrapSim` を使う — mirror は SetLoop/ClearLoop を
  またぐと scheduler `pos_at` を再構成できないため）。
- `crates/synvelis-player/src/routing_control.rs`
  `NodeSurface`/`observe_node_block`/
  `note_transport_command`/`boundary_*`（scheduled/
  staged transport の解決規則と validity 変換の
  先例）。
- `crates/synvelis-player/src/process_boundary.rs`
  `command_boundary`/`window_offset`/`evaluate`/
  `degraded_plan`（command 由来 point 付加と
  LoopWrap deferral 規則の既存契約）。
