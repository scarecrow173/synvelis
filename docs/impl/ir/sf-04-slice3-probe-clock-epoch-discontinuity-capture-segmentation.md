---
id: "IMPL-034UL7HNJppFLqjkPqHDxX"
type: "impl"
status: "in-progress"
title: "SF-04 slice3 probe clock epoch discontinuity capture segmentation"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/sf04_s3_probe_discontinuity.rs"
      - type: "file"
        path: "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
      - type: "file"
        path: "docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md"
      - type: "file"
        path: "docs/test-specs/sf-04-slice3-clock-epoch-discontinuity.md"
      - type: "file"
        path: "docs/plans/sf-04-slice3-clock-epoch-discontinuity.md"
      - type: "file"
        path: "docs/tasks/sf-04-slice3-clock-epoch-discontinuity.md"
      - type: "file"
        path: "docs/impl/ir/sf-04-slice3-probe-clock-epoch-discontinuity-capture-segmentation.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/probe.rs"
      - type: "file"
        path: "crates/synvelis-player/src/probe.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_engine.rs"
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
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/tasks/sf-04-slice3-clock-epoch-discontinuity.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md"
    - "docs/test-specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/plans/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/impl/ir/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/impl/ir/sf-04-slice2-controlsession-probe-observation-trace-wiring.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-04 slice3 probe clock epoch discontinuity capture segmentation

## Summary

Issue #107 (SF-04) slice 3: cursor-series の clock epoch 不連続
（transport seek）に対する fail-closed 挙動を実装した。
`CaptureSegmentEnd::ClockEpoch` を project 契約に追加し、
`ProbeSession::bump_clock_epoch`/`time_context`（Ruviform
`ProbeBroker::bump_epoch`/`time_context` への委譲）と drain 側の
lazy epoch 照合（`observe_meta`/`observe_status` +
`cut_segment_on_epoch_change`）を配線、`RoutingEngine::seek` が
epoch を bump して `probe_time_context()` で `ProbeTimeContext`
（Ruviform #309）を公開する。headless fixture 5 件で epoch 進行・
segment cut・暗黙再 capture 不在・DSP 不変・bounded fault
（capacity/rebind channel-full atomicity）を検証済み。

## Context

`docs/tasks/sf-04-slice3-clock-epoch-discontinuity.md`
（in-progress）に基づく。上流は spec/design/test-spec/plan の
SF-04 slice3 文書チェーンと slice1/2 の IR。slice1/2 で確立した
semantic 解決・generation-scoped binding・segment ledger・trace
配線の上に、cursor-series 時間不連続の契約を追加する残件。

## Implementation

### Changes

- `crates/synvelis-project/src/probe.rs`: `CaptureSegmentEnd` に
  `ClockEpoch` variant 追加（cursor-series clock epoch 変化、
  transport seek 等）。`ProbeCaptureSegment` doc を「plan
  generation 境界または cursor clock-epoch 変化を跨がない」に更新。
- `crates/synvelis-player/src/probe.rs`:
  - `ProbeSession::time_context()` — broker の
    `ProbeTimeContext` 参照を公開。
  - `ProbeSession::bump_clock_epoch()` — `broker.bump_epoch()`
    への委譲。`Err(CommandChannelFull)` 時も broker epoch は
    進んだまま（Ruviform が increment→wire の順で処理するため）で、
    broker/deck 乖離は観測可能な契約として維持。
  - `observe_meta` — `Captured` chunk の `RecordMeta` で
    generation 照合後に `clock_ref(meta.clock)` を取得し、
    `cut_segment_on_epoch_change` を呼ぶ。cut が起きた場合は
    `open.contains_key` が false となり triggering record を
    捨てる（post-epoch の frame range を旧 segment に fold
    しない）。
  - `observe_status` — closing kind（`CaptureStopped`/`Removed`/
    `RebindDropped`）以外の status で epoch 照合を gap/stop
    bookkeeping **より先**に実行し、post-epoch の
    `Unobserved`/`Lost*` が旧 segment の `discontinuities` に
    入るのを防ぐ。
  - `cut_segment_on_epoch_change` — open segment の `clock` が
    `Some(old)` で `old != clock` の場合のみ `open.remove` +
    `close_segment(ended_by=ClockEpoch)`。`clock_context` は
    `segment.clock.or(status_clock)` により旧 epoch を保持。
    open 無し・`clock` が `None`（unwired）・同一 clock は no-op。
- `crates/synvelis-player/src/routing_engine.rs`:
  - `RoutingEngine::seek` — transport re-anchor + drain settle
    pass の後に `let _ = self.probes.bump_clock_epoch()`。
    失敗は握り潰すが silent ではない（broker epoch は進み乖離が
    観測可能）。
  - `probe_time_context()` — `ProbeSession::time_context()` の
    passthrough。
- `crates/synvelis-player/tests/sf04_s3_probe_discontinuity.rs`
  （新規・5 件）: seek epoch・capture segment cut・DSP
  invariance・probe table capacity・rebind channel-full
  atomicity/recovery。
- `docs/` slice3 文書チェーン 6 件。

### Decisions During Implementation

- **lazy cut（eager ではなく drain 時点）**: `bump_clock_epoch`
  呼出時点では `SetClockMark` が RT command channel を通る前に
  処理された block の旧 epoch chunk が残り得る。eager に open
  segment を閉じるとそれらの正当な旧 epoch chunk が帰属先を
  失うため、「新 epoch evidence が drain された瞬間」に cut する
  設計とした。bump 失敗時は deck が旧 epoch のまま record を
  出し続け segment は切れない — epoch が進んでいない record を
  新 epoch と見なさない正直な挙動。
- **triggering record の破棄**: epoch cut を起こした
  record/status 自体は旧 segment に帰属させない。`observe_meta`
  は cut 後の `open.contains_key` で早期 return、`observe_status`
  は cut 後も通常経路（binding status 更新等）は継続するが、
  gap/`discontinuities` への記録は cut 後の open segment が
  無いため自然に抑止される。
- **closing kind の除外**: `CaptureStopped`/`Removed`/
  `RebindDropped` は自身の end reason を持つため epoch cut の
  対象外とし、既存の close 経路に任せる。
- **`ClockContextRef` 全体の等価比較**: epoch のみでなく domain/
  runtime instance/sample rate の変化でも切断対象 — cursor-series
  同一性が変わった以上同一 span と見なさない契約。
- **seek のみを bump 点とする**: loop/device restart/stop-start
  等の他不連続は本 slice では対象外。`bump_clock_epoch` API は
  再利用可能だが宣言点の網羅は transport 契約の別 slice。
- **bump 失敗の扱い**: `seek` は transport 操作であり probe
  clock 配送失敗で seek を失敗させない。`let _ =` で握り潰すが、
  Ruviform の increment→wire 順序により broker epoch は進むため
  `probe_time_context` と record epoch の乖離として観測可能。

## Related Experiments

### Adopted

<!-- none -->

### Rejected

<!-- none -->

## Validation

- `mise run fmt` / `mise run fmt-check`: pass
- `mise run check`（cargo check --workspace --all-targets）: pass
- `mise run lint`（cargo clippy --workspace --all-targets --
  -D warnings）: pass
- `mise run test`（cargo test --workspace --locked）: pass
- `mise run test-rt`（cargo test --workspace --locked rt_）: pass
- `mise run test-msrv`（rust@1.89.0 cargo test --workspace
  --locked）: pass
- `mise run check-headless-deps`: pass
- `mise run check-ruviform-rev`: **fail（既存・本変更と無関係）**
  — `ui/vendor/ruviform/REV` (`20a24a66`) と `Cargo.lock` 解決 rev
  (`0f5b5737`) の不一致。両ファイルとも本 diff 未変更で base
  時点の既存状態。`ui/`・`synvelis-app` は編集禁止範囲のため
  未対処のまま報告。
- 補助証拠（canonical = mise）: `cargo test -p synvelis-player
  --test sf04_s3_probe_discontinuity` 5/5 pass。

## Risks

- lazy cut の帰属境界は「最初の新 epoch evidence」で決まるため、
  `SetClockMark` 適用前後の block 境界で旧/新 epoch record が
  混在し得る — これは FIFO 順序どおりの正しい帰属であり、
  segment は epoch 単位でのみ閉じる。
- `SetClockMark` 配送失敗時の broker/deck epoch 乖離は観測可能
  だが能動修復しない — fault 注入拡充時に再検討。
- seek 以外の不連続宣言点は未対応 — transport 契約拡張時に
  `bump_clock_epoch` を追加呼出するだけで同経路が効く設計。

## Follow-ups

- #108: UI 側で `probe_time_context`/`probe_segments` の
  `ClockEpoch` 終了を表示に反映する接続。
- #109: 実測・oracle（SF-06 cross gate）— 本 slice 範囲外・NotRun。
- transport 契約での他不連続宣言点（loop/device restart）の epoch
  bump 追加 — 別 slice。
- `check-ruviform-rev` の既存不一致は UI vendor 同期の別件として
  切り分けが必要（`mise run sync-ui-deps` 相当の運用判断）。
