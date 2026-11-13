---
id: "DESIGN-034X1y3Clx3u80e2MVL9IH"
type: "design"
status: "approved"
title: "RCD-03 monitor/punch fake session fixture design"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added:
      - "crates/synvelis-recording/tests/monitor_punch_session.rs"
      - "evidence/sv117/rcd-03-monitor-punch-session.md"
      - "docs/specs/rcd-03-monitor-punch.md"
      - "docs/designs/rcd-03-monitor-punch-design.md"
      - "docs/plans/rcd-03-monitor-punch-plan.md"
      - "docs/tasks/rcd-03-monitor-punch-task.md"
      - "docs/test-specs/rcd-03-monitor-punch.md"
      - "docs/impl/ir/rcd-03-monitor-punch.md"
      - "docs/impl/exp/rcd-03-monitor-punch.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - "docs/specs/README.md"
      - "docs/designs/README.md"
      - "docs/plans/README.md"
      - "docs/tasks/README.md"
      - "docs/test-specs/README.md"
      - "docs/impl/ir/README.md"
      - "docs/impl/exp/README.md"
  implements:
    - "docs/specs/rcd-03-monitor-punch.md"
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-03-monitor-punch.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-03-monitor-engine.md"
    - "docs/designs/rcd-03-monitoring-punch-latency.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-monitor-punch.md"
    - "https://github.com/scarecrow173/synvelis/issues/117"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  defers: []
  deferred-by: []
---

# RCD-03 monitor/punch fake session fixture design

## Context

PR #214 で `decide`/`MonitorProbe`、PR #373 で `MonitorAdapter`
の engine 搭載は確定済み。しかし device fault 系の既存
fixture（`device_recording.rs`）はすべて `MonitorState::Off`
の tap で駆動され、monitor 状態が fault 挙動・verified
prefix・commit 分類へ副作用を与えないことは未検証。また
`MonitorProbe` の route 区間と実際に staging へ記録された
gate-open 区間が session レベルで一致すること、commit
payload（Take Session 意味論）が monitor 非依存であることも
未 pin だった。本 design は `synvelis-recording` の範囲で、
既存 `DeviceRig`（fake device → `InputSession` →
`StreamWriter` → `StagingArea`）と `MonitorProbe` を合成する
Fake Session fixture の構成を定義する。

`src/` への変更は不要 — monitor/arm は `InputTapSpec.binding`
の metadata として capture path に入らず（`arm` のみが
capture resource を構築する）、gate/route 権威は
`GateScheduler`/`evaluate_gate`/`decide` に既に集約されて
いる。本 slice はその契約を session・staging・commit 層まで
通して pin する検証 fixture の追加である。

## Scope

### In Scope

- `crates/synvelis-recording/tests/monitor_punch_session.rs`
  （新規）: `DeviceRig` + `MonitorProbe` 合成の Fake Session
  fixture と 4 系統の test。
- `evidence/sv117/rcd-03-monitor-punch-session.md`: Expected/
  Observed・fake/headless Pass と実機/GUI NotRun の区別を
  記録する証拠文書。
- 本 doc-chain（spec/design/plan/task/test-spec/impl ir+exp）
  と docs index の再生成。

### Out of Scope

- `src/` の製品コード変更（既存契約で充足するため不要）。
- `synvelis-player`/`synvelis-app`/その他 crate の変更。
- in-session device resume、実 GUI 2Window/Focus、実 device/
  GUI/plugin の L2/L3 受入。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI workflow。

## Components and Boundaries

- `SessionProbe`（test 内 helper）: `MonitorProbe<'a>`
  （`&'a PreparedRecordingPlan` 借用）+ `TrackMonitorContext`
  + 収集 `Vec<MonitorSegment>`。`feed_block(n, phase)` が
  `probe.feed` を呼び segment を連結する。
- `punch_plans()`（test 内 helper）: `GateSpec::Punch`
  （punch_in tick 12 → session frame 300、punch_out tick 28
  → 700、stop tick 40 → session_end 1000、25 f/tick）の
  `PreparedRecordingPlan` を **2 個**構築する。`plan` は
  `Clone` ではなく `MonitorProbe<'a>` が `&plan` を借用する
  一方 `DeviceRig` は plan を所有して `&mut self` で駆動
  するため、同一 spec から `preflight` を 2 回呼んで
  「capture 側 plan」と「monitor 観測側 plan」を得る
  （決定的構築のため gate schedule は同一。実系でも monitor
  session は録音側と同一の prepared plan を参照する契約で
  あり、fixture では同一内容の別 instance で表現する）。
- `drive_with_probe(rig, probe, frames, block, phase)`:
  `rig.drive` の per-block 手順（`deliver_all` → `pump_all`）
  を展開し、各 block で `probe.feed_block(n, phase)` を呼ぶ。
  `probe.finished()` と `session_end` 到達を検証する。
- 既存 helper の再利用: `project_with_lanes`/`budget`/
  `input_config`/`tmp_dir`/`session_id`、`DeviceRig::open_one`/
  `drive`/`finish`、`capture_of`/`all_journal_spans`/
  `assert_session_coverage`/`assert_prefix_contiguous`/
  `assert_source_contiguous`/`verify_capture_pcm`/
  `span_frame_total`/`span_obs`、`build_payload`、
  `FAULT_INPUT_DISCONTINUITY` 等の fault bits、
  `StopReason`/`FinalizedOutcome`。

## Data and Control Flow

### T1 route/capture coherence + raw/commit 独立性

1. `MonitorState` ∈ {Off, Auto, On}（On は `input_output_ready`
   true/false の 2 相）で rig を構築し、`TransportPhase::
   Playing` 固定・block 384（session_end 1000 を割り切らない
   値で block 途中境界を跨ぐ）で 1000 frame 駆動。
2. `SessionProbe` が同 block で feed され、segment 列
   （`session.start()/end()`・`gate_open`・`route`・
   `notices`）を収集。
3. `rig.finish()` → `capture_of` → `all_journal_spans` で
   capture 済み gate-open 区間を取得。
4. 検証:
   - probe の `gate_open == true` 区間を merge したものが
     `[(300, 700)]` であり、journal span の session 被覆が
     `assert_session_coverage(&[(300, 700)])` と一致。
   - route 区間が decide 規則どおり: Off+Arm → Muted@[300,700)
     + Playback@他、Auto+Arm+Playing → Input@[300,700)+
     Playback@他、On+ready → Input@全域、On+!ready →
     Playback@全域+`input_unavailable` notice。
   - On+ready のとき route Input が gate 閉 [0,300)∪[700,1000)
     にも及ぶ一方、capture は [300,700) のみ — Raw 独立性の
     session レベル pin。
   - 3 状態の `cap.media` bytes・`span_obs`・journal records
     が一致（monitor が raw file を変えない）。
   - `build_payload` の `streams[0].segments` の
     `(session_start, session_end, pass_index, project_in,
     project_out)` 列が monitor 状態間で一致し、session 範囲が
     gate-open [300,700) を cover、`takes` が pass 0 で
     segment を参照する。

### T2 device fault の monitor 非依存性

1. `MonitorState` ∈ {Off, Auto, On} で Punch rig を構築し、
   gate-open 区間途中（frame 600 = punch_in 300 通過後）まで
   正常駆動。
2. `jump_to(+777)` で frame gap を注入 → 次 block で
   `FAULT_INPUT_DISCONTINUITY` を観測 → fail-stop。
3. 各状態で `finish` → `assert_fail_stop_prefix` 相当
   （capture-fault marker・prefix contiguous・PCM verified）
   を検証し、`(fault bits, span_obs, prefix 総 frame)` が
   状態間で一致することを確認。
4. 別途 `raise(DeviceError::DeviceNotFound)`（device loss）
   でも同じ観測を `MonitorState::Auto` で pin する。

### T3 punch 中 device loss → fail-stop → recovery

1. Session A: Punch rig・Monitor Auto・Arm On。frame 500
   （gate-open [300,700) の内部）まで駆動後、stream に
   `raise(DeviceNotFound)` → fail-stop → `finish`。
   verified prefix が session 0 から連続で fault 前まで
   残ること、`build_payload` が stream を `capture-fault:`
   理由で recoverable 分離し `payload.streams`/`passes` が
   空であることを検証。旧 staging の media/journal が
   保持されることも確認。
2. Session B（recovery）: 回復した device 相当として**新規**
   `FakeInputDevice` + 新規 staging root + 新規 session id で
   同一形状の rig を構築し、1000 frame を clean 駆動 →
   `finish` → finalize Complete → `build_payload` で
   `streams[0]` が commit され、segments が [300,700) を
   cover することを検証。旧 session の staging は参照も
   変更もされない。

### T4 2 session 並行の fault 分離

1. Session A（Monitor On）と Session B（Monitor Off）を別
   device・別 staging root・別 session id で構築。
2. block 単位で interleave 駆動し、A の途中で frame gap を
   注入して A を fail-stop させた後も B を完走させる。
3. B が Complete finalize・`assert_session_coverage(
   [(300,700)])`・PCM verified・`build_payload` commit 成功
   であること、A の fault bits/marker が B の area・session・
   payload に一切現れないことを検証（`FaultLatch` は
   session 共有であり session 間で構造的に分離される契約の
   pin。実 GUI の 2Window/Focus 挙動は対象外）。

## Non-Functional Constraints

- fixture は headless・deterministic・bounded（session_end
  1000 frame・block 384・単一 stream）。
- probe feed は非 RT の test 駆動側で行い、RT callback 内へ
  `Vec`/probe 処理を持ち込まない（`MonitorProbe` は元来
  非 RT 観測経路）。
- fake/headless の Pass を実機・実 GUI の Pass と主張しない
  （evidence と PR 本文で明示）。
- `DeviceRig`/`common` helper は既存のものを再利用し、
  harness 自体は変更しない（`common/mod.rs` は無変更）。

## Risks and Trade-offs

- `MonitorProbe` の session 位置と capture の journal span
  session 位置が block 境界でずれるリスク → `drive_with_probe`
  が `rig.drive` と同一の block 刻み・同一 frame 数で feed
  することで 1:1 を保証し、`probe.finished()` を assert
  する。
- fail-stop 後も probe を feed し続けると route 区間が
  capture prefix を越える → fault 系 test（T2/T3/T4）では
  route 区間の一致は主張せず、fault 挙動・prefix・commit
  分類のみを pin する。coherence 主張は T1 の clean 経路に
  限定する。
- `build_payload` の `draw`（Stable ID 発行器）に定数を
  使うと ID が重複する → 比較対象は ID ではなく
  session/project 範囲・pass 構造に限定し、既存
  `device_recording.rs` と同じ `&mut || 0` を使う。
- recovery session を同一 `FakeInputDevice` の 2 回目 open で
  表現すると `DeviceRig::open` の per-device ordinal が
  旧 stream を指す → recovery は新規 `FakeInputDevice`（device
  再列挙相当）で構築し、旧 session の in-session resume を
  主張しない。

## Deferred Design Concerns

- 実 device monitor/latency 受入・実 GUI 受入 | Issue #117 の
  L2/L3 gate | 実機・GUI slice | fake 成功の実機誤認
- in-session device resume | fail-stop が製品契約 | resume
  設計 slice | verified prefix 保証の崩壊
- 実 GUI 2Window/Focus・app Command 配線 | `synvelis-app`
  別 Worker 占有 | app slice | UI 挙動との乖離
- `synvelis-player` 側 monitor PCM 供給の session 統合 |
  player 占有 | player/app slice | monitor bus 未接続

## Verification Notes

- [ ] `monitor_punch_session.rs` の 4 test が pass し、
      route/capture 一致・fault 非依存・loss→recovery・
      session 分離・commit 非依存を pin する。
- [ ] `mise run test synvelis-recording` で既存 test を含め
      全 pass。
- [ ] `evidence/sv117/` に Expected/Observed・fake Pass・
      実機 NotRun を記録する。
