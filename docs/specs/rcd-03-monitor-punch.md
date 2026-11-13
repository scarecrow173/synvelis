---
id: "SPEC-034X1wtNCuKVrXsdtPH1r1"
type: "spec"
status: "proposed"
title: "RCD-03 monitor/punch fake session fixture — synvelis#117"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-03-monitor-engine.md"
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  defers: []
  deferred-by: []
---

# RCD-03 monitor/punch fake session fixture — synvelis#117

## What Should Be Built

Issue #117 の `synvelis-recording` 残スライスとして、Fake Input/
Monitor を用いた **Fake Session fixture** を追加し、monitor/punch
の製品意味論を session・staging・commit 層まで統合して検証する。

具体的には `crates/synvelis-recording` に、既存の
`DeviceRig`（fake device → `InputSession` → `StreamWriter` →
`StagingArea`）と `MonitorProbe`（`GateScheduler`/`evaluate_gate`
を共有する monitor route 観測）を同一 block 駆動で合成する
headless fixture を実装し、次を pin する。

- Off/Auto/On × Arm × Transport × Punch × Mute/Solo の可聴 route
  モデルと Raw File（journal/media/staging）の独立性を session
  レベルで再現する。
- Punch in/out 境界（callback block 途中を含む）・Loop/
  Tempo/Latency・Raw Placement・Frame Gap/二重補正が session
  経路でも成立することを検証する。
- Device loss / frame gap / clock 異常時の fail-stop・verified
  prefix 保存が Monitor 状態（Off/Auto/On）に依らず同一で
  あることを検証する。
- Punch 中の device loss → fail-stop → 新規 session での
  recovery が、旧 session の verified prefix を壊さずに成立
  することを検証する。
- 2 session 並行（2Window/Focus の fake-session 相当）で fault
  が session 間に漏れず、RT/Project に誤副作用を起こさない
  ことを検証する。
- commit payload（Take Session 意味論）が Monitor 状態に
  依らず gate-open 区間のみを segment/pass/take として
  構成することを検証する。

本 spec は Issue #117 を Close しない（実 device・実 GUI・
実 plugin の L2/L3 受入は別 gate）。

## Why Now

先行 slice（PR #214 crate 内契約、PR #373 engine 搭載）で
`decide`/`MonitorProbe`/`MonitorAdapter` の単体契約と engine
実経路への搭載は確定した。しかし現行の device-fault テスト
（`device_recording.rs` 等）はすべて `MonitorState::Off` で駆動
されており、「monitor を On/Auto にしていても device 異常・
frame gap で raw capture が誤停止・二重補正されない」こと、および
「monitor route 観測と実際に記録された gate 区間が session
レベルで一致する」ことは未検証だった。Issue #117 の受入条件が
要求する Fake Session/UI Scenario 相当の headless 証拠を
`synvelis-recording` の範囲で埋めるのが本 slice の目的。

## Users and Value

- 将来の統合層（`synvelis-app` の Arm/Monitoring UX・#114
  GUI Scenario）: monitor 状態が device 異常・session 分離・
  commit 意味論へ副作用を与えないことを crate 契約として
  再利用可能な形で得る。
- 検証者: Off/Auto/On × device fault × Punch gate の組合せで
  verified prefix・commit 分類・session 分離が同一であることを
  headless fixture で実測できる。

## Scope

### In Scope

- `crates/synvelis-recording/tests/` への Fake Session fixture
  追加（`DeviceRig` + `MonitorProbe` 合成の headless 駆動）。
- monitor route 区間と capture 済み gate 区間の一致検証、
  raw media/journal の monitor 非依存性の session レベル pin。
- device frame gap・device disconnect の fault 挙動が monitor
  状態に依らないことの検証。
- punch 中 device loss → fail-stop → 新規 session recovery の
  検証（旧 session の verified prefix と staging を保持）。
- 2 session 並行での fault 分離の検証。
- commit payload の segment/pass/take 構成が monitor 非依存で
  あることの検証。
- `docs/` の本 doc-chain と `evidence/sv117/` の証拠記録。

### Out of Scope

- `synvelis-player`/`synvelis-app`/`synvelis-native`/
  `synvelis-workspace`/`synvelis-project` の変更（他 Worker
  占有。必要な配線は PR 本文「未対応の作業」に記録）。
- 実 device・実 GUI・実 plugin の L2/L3 受入。
- in-session device resume（faulted session の継続録音）。
  本 slice は fail-stop + verified prefix + 新規 session
  recovery のみを対象とする。
- 実 GUI の 2Window/Focus 挙動（app 層・#114 の範囲）。
  本 slice は fake-session 分離の crate 契約のみを pin する。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI workflow の変更。

## Requirements

- Fake Session fixture は `DeviceRig`（fake device →
  `InputSession` → `StreamWriter` → `StagingArea`）と
  `MonitorProbe` を同一の block 単位駆動で合成し、probe の
  `TransportPhase` を caller が block 境界で供給する。
- monitor route（`MonitorRoute::Input`/`Playback`/`Muted`）の
  区間が capture 済み gate-open 区間と、定義された phase
  条件の下で一致することを検証する。`MonitorState::On` で
  route Input が gate 閉区域間にも及ぶ場合、capture が
  gate-open のみに留まること（Raw 独立性）を同時に pin する。
- monitor 状態を Off/Auto/On で切替えても、device frame gap・
  disconnect 時の fault bits・verified prefix 長・PCM 内容が
  同一であることを検証する。
- punch gate-open 区間の途中で device disconnect が起きた
  場合、fail-stop し verified prefix が session 0 から連続
  であること、`build_payload` が当該 stream を
  `capture-fault:` 理由で recoverable 分離することを検証
  する。その後の新規 session（回復した fake device 相当）
  が clean に完了し commit payload を構成できることを検証
  する。
- 2 つの独立 session（別 staging root・別 session id・別
  fake device）を並行駆動し、一方の fault が他方の capture・
  finalize・commit 分類に影響しないことを検証する。
- `build_payload` が生成する segment の session 範囲・pass
  構成が monitor 状態に依らず gate-open 区間と一致すること
  を検証する。
- すべての検証は headless・deterministic・bounded であり、
  RT callback 内の確保・blocking を導入しない。fake/headless
  の成功を実機・実 GUI の Pass と主張しない。

## Acceptance Criteria

- [ ] `tests/monitor_punch_session.rs` が Off/Auto/On × Arm ×
      Transport(Playing) × Punch gate
      × Mute/Solo の route 区間と capture gate 区間の一致・
      Raw File 独立性を session レベルで検証し pass する。
- [ ] 同 fixture が device frame gap・disconnect の fault
      挙動（fault bits・verified prefix・PCM）が monitor
      状態に依らず同一であることを検証し pass する。
- [ ] 同 fixture が punch 中 device loss → fail-stop →
      verified prefix 保持 → 新規 session recovery →
      commit 分類（recoverable 分離→新規 commit 成功）を
      検証し pass する。
- [ ] 同 fixture が 2 session 並行での fault 分離を検証し
      pass する。
- [ ] 同 fixture が commit payload の segment/pass/take が
      monitor 非依存で gate-open 区間を反映することを検証し
      pass する。
- [ ] `mise run fmt-check`/`check synvelis-recording`/
      `lint synvelis-recording`/`test synvelis-recording`
      がすべて exit 0。
- [ ] `evidence/sv117/` に Expected/Observed を区別した
      headless 証拠を記録し、fake Pass と実機/GUI NotRun を
      明示する。
- [ ] review-protocol で blocker が残っていない。

## Deferred Design Concerns

- 実 device monitor/latency 受入・実 GUI 受入行 | Issue #117
  の L2/L3 gate | 実機・GUI slice | fake 成功を実機 Pass と
  誤認する危険
- in-session device resume（faulted stream の session 内継続）
  | fail-stop が製品契約であり resume は設計変更を要する |
  resume 設計 slice | 暗黙の resume で verified prefix 保証が
  崩れる危険
- 実 GUI の 2Window/Focus・app Command 配線 | `synvelis-app`
  は別 Worker 占有 | app slice | session 分離契約と UI 挙動の
  乖離

## Sources and Relations

- Issue #117（Monitoring / Punch / Latency UX — Auto標準・
  Raw独立・Direct監視）の `synvelis-recording` 残スライス。
- `docs/specs/rcd-03-monitor-engine.md`（engine 搭載済みの
  monitor 契約）、`docs/specs/rcd-03-monitoring-punch-latency.md`
  （crate 内 decide/MonitorProbe 契約）、`docs/specs/
  rcd-03-monitor-persistence.md`（project 永続化）。
- 既存 `crates/synvelis-recording/tests/monitor_gate.rs`
  （route matrix・block 途中 punch 境界・Loop/PunchInLoop・
  raw 独立性の単体 pin）と `tests/device_recording.rs`
  （device fault・fail-stop・verified prefix、Monitor Off
  のみ）。
