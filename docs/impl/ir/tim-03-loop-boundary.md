---
id: "IMPL-034XB9xQB2ivo6a7X5ITmw"
type: "impl"
status: "completed"
title: "TIM-03 Loop boundary 自動導出 — WrapSim 実装記録"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/tim03_loop_boundary.rs"
      - type: "file"
        path: "docs/specs/tim-03-loop-boundary.md"
      - type: "file"
        path: "docs/designs/tim-03-loop-boundary.md"
      - type: "file"
        path: "docs/plans/tim-03-loop-boundary.md"
      - type: "file"
        path: "docs/tasks/tim-03-loop-boundary.md"
      - type: "file"
        path: "docs/test-specs/tim-03-loop-boundary.md"
      - type: "file"
        path: "docs/impl/ir/tim-03-loop-boundary.md"
      - type: "file"
        path: "docs/impl/exp/tim-03-loop-boundary.md"
      - type: "file"
        path: "evidence/sv127/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/process_boundary.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_control.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/tim-03-loop-boundary.md"
    - "docs/plans/tim-03-loop-boundary.md"
    - "docs/designs/tim-03-loop-boundary.md"
    - "docs/specs/tim-03-loop-boundary.md"
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
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/tim-03-loop-boundary.md"
    rejected: []
---

# TIM-03 Loop boundary 自動導出 — WrapSim 実装記録

## Summary

`RoutingControlDispatch::boundary_loop_wraps` を追加し、
upcoming window 内の loop wrap `BoundaryPoint{LoopWrap}` を
caller の手列挙なしで導出可能にした。scheduler と同一
algebra の scalar state `WrapSim` を `NodeSurface` ごとに
維持し、`observe_node_block` が観測する全 window を
pending command 適用列で walk して進める。
`boundary_loop_wraps` は sim を clone して read-only で
preview walk し、`boundary_plan_window`/
`boundary_stage_window` の `boundaries` へそのまま渡せる
`Vec<BoundaryPoint>` を返す。

## Context

Issue #127 / TIM-03 の loop-boundary slice。前提: TIM-03
session→driver/routing wiring（PR #308）統合済みで
`plan_window`/`stage_window`/`stage_plan` は
`boundaries: Vec<BoundaryPoint>` を caller 供給とし、
command 由来 point のみ自動付加していた。`TransportMirror`
は scheduled SetLoop/seek/tempo を保持するが、unfold
line を *現在の* loop region で lazy fold する投影のため、
途中の `SetLoop`/`ClearLoop` をまたぐと scheduler の
`pos_at`（wrap ごとに再錨定する scalar state）を再構成
できない。このため mirror 読み出しではなく、scheduler
等価 state を node ごとに維持する方式を採用した
（`docs/impl/exp/tim-03-loop-boundary.md`）。

## Implementation

### Changes

- `crates/synvelis-player/src/process_boundary.rs`:
  `pub(crate)` `WrapSim`（`Copy` scalar state:
  `status`/`anchor_frame`/`anchor_pos`/`loop_ticks`/
  `frame_base`）を追加。`pos_at`/`next_wrap`/`wrap`/
  `apply_command`/`walk_window` を実装。
  `apply_command` は `media_time_ticks` + scheduler
  validity 規則（Seek 非負・SetLoop 非空）を
  `note_transport_command` と同一に適用。generation/
  receipt/plan は持たない。`walk_window` は window head で
  `pos_at` から再錨定し、due <= head の applies を畳み込み、
  `(ws, we)` を walk して wrap frame を offset 昇順・
  同一 offset coalesce で `Vec<BoundaryPoint>` として返し、
  `self` を post-window state へ進める。window end の wrap
  は次 window へ帰属（end-exclusive）。同一 frame の
  apply は wrap より先に効く（upstream tie 規則）。
  unit test 9 件を同 module に追加。
- `crates/synvelis-player/src/routing_control.rs`:
  `NodeSurface.wrap_sim: WrapSim` を追加（vend 時に
  scheduler-initial state で初期化）。`observe_node_block`
  が staged→scheduled 解決と due sort の後、`due <
  end_frame` の pending を集めて
  `node.wrap_sim.walk_window(start_frame, window_frames,
  &pending)` を呼び sim を進める（mirror fold・segment
  dispatch は従来どおり、command 適用列は同一
  `scheduled_transport` を共有するため二重適用なし）。
  `fast_control` の内部 `Play` も staged_transport へ
  追加し sim の初期状態が実 activation と一致するように
  した。`RoutingControlDispatch::boundary_loop_wraps
  (&self, processor, project, call_start, call_frames)`
  を追加: `surface_is_current` で `NoSurface`/
  `StaleSurface` を検査し、node 不在で `NoSession`。
  boundary session は要求しない。scheduled（absolute
  due）+ staged（`ws + due` 解決、push 順保持）を
  `(due, cmd)` へ merge し stable sort（同 due は
  scheduled → staged の queue 順、`observe_node_block`
  と一致）、`we` 未満のみを `wrap_sim.clone()` に対して
  `walk_window` し結果を返す。実 state は変更しない。
- `crates/synvelis-player/tests/tim03_loop_boundary.rs`
  （新規）: 7 integration test。Fake Plugin が
  scheduler span ごとに `(position, frames)` を記録し、
  call head を累積 frame 数から再構成して実 scheduler の
  span cut（= wrap frame）と導出 offset の一致を検証。
  steady loop・no loop/not-playing・mid-window
  SetLoop/Seek/ClearLoop・oracle 連携・provenance
  gating/sessionless node を網羅。

### Decisions During Implementation

- **maintained `WrapSim` 採用（mirror 読み出しを棄却）**:
  初期設計は `TransportMirror` から scalar state を
  読み出して window start で再構成する方式だったが、
  mirror の unfold line は現在の loop region でしか
  fold できず、`SetLoop`/`ClearLoop` をまたぐ履歴で
  scheduler `pos_at` と乖離することが upstream
  `TransportScheduler`/`SimState` の精読で判明。
  wrap は scheduler 側で逐次再錨定されるため、同じ
  適用列を見た node ごとの scalar sim を維持する方が
  正しい境界（scheduler-equivalent state の所有は
  routing_control の観測経路）であり、mirror の
  commanded-state approximation（scheduler reject・
  out-of-band `control()` を見ない既知 limitation）を
  そのまま継承できる。
- **frame-linear position domain**: 本 slice の position
  domain は `position_base == frame_base`（1 tick == 1
  engine frame、Playing 中のみ前進）。tempo map
  revision は musical 情報専用で wrap frame を動かさない
  — `pos_at`/`next_wrap` は tempo を参照しない upstream
  不変条件をコードコメントと文書に明記。
- **`boundary_loop_wraps` は session を要求しない**:
  wrap sim は全 vended control node で維持されるため、
  boundary session 未宣言の node でも導出可能。
  session-backed node のみが oracle へ point を
  消費できる（`boundary_session` が `None` の node は
  `boundary_plan_window`/`boundary_stage_window` が
  `NoSession` を返す既存契約）。
- **offset 0 の扱い**: window head の wrap は offset 0 の
  `BoundaryPoint` として返す。oracle 側では offset 0 は
  window head の implicit boundary として正規化され、
  command 由来 point と offset dedup される。

## Related Experiments

### Adopted

- `docs/impl/exp/tim-03-loop-boundary.md` — mirror
  読み出し vs maintained WrapSim の比較試行。maintained
  WrapSim を採用（上記 Decisions 参照）。

### Rejected

- `TransportMirror` からの window-start state 再構成
  （unfold line + 現在 loop での lazy fold）—
  SetLoop/ClearLoop またぎで scheduler `pos_at` を
  再構成できないため棄却。

## Validation

- `cargo test --locked -p synvelis-player --test
  tim03_loop_boundary` — 7 tests all ok（導出 offset と
  実 scheduler span cut の frame-exact 一致を含む）。
- `cargo test --locked -p synvelis-player` — unit 9 件
  （WrapSim algebra）+ 既存 integration suite 全 pass。
- `cargo check --locked --all-targets -p synvelis-player`
  — clean。
- `mise run fmt-check` / `mise run check
  synvelis-player` / `mise run lint synvelis-player` /
  `mise run test synvelis-player` — `evidence/sv127/
  verification.md` に結果を記録。

## Risks

- `WrapSim` は mirror と同じ commanded-state
  approximation 上に立つ: scheduler 側 reject や
  out-of-band `control()` push は sim に見えない。
  receipts が generation 軸を補正する既存契約は変更なし。
- staged `due_frames` の絶対解決は「`boundary_loop_wraps`
  の `call_start` == 次の `observe_block` call_start」
  契約に依存。不一致時は導出がずれ得る — doc comment
  に明記済み。
- `position_base == frame_base` 前提: frame 以外の
  position base を持つ構成では wrap frame 導出は
  無効。現行 PoC では engine frame 固定。

## Follow-ups

- consumer 照合（AL f64/native PCM）・実 plugin/OS
  matrix・precision certification — 別 slice（spec の
  Out of Scope / Deferred と同一）。
- `HeadlessDriver` 側での wrap 導出配線 —
  `loop_wrap_boundaries` 相当は `pub(crate)` で
  再利用可能だが本 slice では未配線。
- degraded deferral の wrap frame 予測 — 要求 frame のみ
  導出し、実適用 frame は oracle/scheduler の責務
  （既存規則維持）。
