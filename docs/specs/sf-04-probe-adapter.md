---
id: "SPEC-HAm4lXgM35E1VEXdXe80jZ"
type: "spec"
status: "approved"
title: "SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約（headless project/player 層）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
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
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice6-probe-compare.md"
    - "docs/specs/sf-04-slice8-probe-faults-declare.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
    - "docs/specs/sf-04-slice6-probe-compare.md"
    - "docs/specs/sf-04-slice8-probe-faults-declare.md"
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
  defers: []
  deferred-by: []
---

# SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約

## What Should Be Built

Issue #107（SF-04 Semantic Probe Adapter）の headless 受入面を、
`synvelis-project` / `synvelis-player` の範囲で SF-PROBE-01〜08 の
各ケースへ写像し、既存 slice 実装（slice1..10、全て main 済み）の
被覆を監査した上で、未被覆の headless 証拠だけを補う集約タスク。

- `crates/synvelis-project`: `resolve_probe_point`（Semantic Point →
  実 Node/Port/Slot、fail-closed `Unavailable`/`Unsupported`）と
  semantic contract 型は slice1 で実装済み — 変更しない。
- `crates/synvelis-player`: `ProbeSession`/`ProbeTraceBridge`/
  `probe_compare`/clock-epoch 宣言は slice1/2/3/6/8/9 で実装済み —
  変更しない。
- 本タスクの差分は `crates/synvelis-player/tests/sf04_probe_acceptance.rs`
  の新規 headless テスト 2 件のみ:
  - bound probe の対象 entity 消失 → republish で `Unresolved`
    へ正直に unbind され、stale slot を観測しないこと、再 attach
    （新 id）が新 generation で fresh bind すること（SF-PROBE-01/03、
    Issue 副次条件「port disappearance/re-attach」「old slot reuse
    で誤観測 0」）。
  - 同一 `ProbeSubscriptionKey` の重複 request が冪等で broker
    probe/capture を重複させないこと、および player 層の dedup が
    key 単位である境界の明示（SF-PROBE-06 の player 層側。window/
    inspector lease は slice5 = synvelis-workspace の契約）。
- `evidence/sv107/` に被覆表・検証結果・外部 Gate の NotRun/
  Unsupported 判定を記録する。

## Why Now

Issue #107 の SF-04 slice 群は全て merge 済みで、残るのは
SF-PROBE-01〜08 の受入写像と外部 Gate（実機・実 plugin・GUI #108・
Domain Oracle #109）の正直な記録。headless 層の未被覆分岐
（entity 消失時の unbind・per-key dedup 境界）を先に固定し、
被覆表で「何が headless で証明済みか」を一意にする。

## Users and Value

- #108（Signal Flow UI）・#109（Domain Oracle）の実装者が、
  probe 状態語彙（`ProbeResolveState`/`ProbeObservation`/
  `AcousticObservation`）と被覆表をそのまま消費できる。
- 受入判定者が、Fake/Fixture の成功を実機 Pass と混同しないための
  NotRun/Unsupported 記録を `evidence/sv107/` で参照できる。

## Scope

### In Scope

- `crates/synvelis-player/tests/sf04_probe_acceptance.rs`（新規）。
- `docs/` 文書チェーン（spec/design/plan/task/test-spec/impl ir/exp）。
- `evidence/sv107/` の新規証拠ファイル。
- 触った docs dir の管理 index 再生成。

### Out of Scope

- `synvelis-project`/`synvelis-player` の src 変更（実装は slice
  群で完了済み。監査で contract 違反が見つかった場合のみ最小修正
  を行い、見つからなければ変更しない）。
- `synvelis-app`/`synvelis-workspace`/`synvelis-native`/
  `synvelis-recording`（別 Worker 占有）。SF-PROBE-06 の session 層
  lease は slice5 で実装済み・本タスクは被覆表で参照するのみ。
- 画面表示・Accessibility・Diagnostics UI（#108）、Domain Oracle
  （#109）、実機/実 plugin/実 device gate、Ruviform 側の変更。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/`.github/workflows/`。

## Requirements

- SF-PROBE-01: Track/Bus/Processor/Main(master bus)/Send/Sidechain
  の Semantic Point → 実 Node/Port/Slot 解決。physical tap なし・
  非 Audio Port は `Unavailable`/`Unsupported`、偽 PCM なし。
- SF-PROBE-02: Probe Off/Monitor/Capture が Processor Bypass・
  Connection・DSP/PCM/perf 基準・Raw Recording/Source Blob を変更
  しない。
- SF-PROBE-03: Requested→Queued→RT Installed の区別、旧 generation
  metrics/stale status/late ack の拒否、Plugin Port 再構成/
  SourceMap 失効で `Unknown`/`Unbound`。
- SF-PROBE-04: Graph swap 中 PCM Capture の区間分断・再自動
  Capture なし、#308 QueueFull/ClearAll＋Bind 途中失敗の Fault
  Injection。#308 は CLOSED のため契約レベル証拠は有効、実機 Gate
  は NotRun。
- SF-PROBE-05: Capture max_frames、ChunkPool/out channel/
  telemetry 満杯、LostBlocks/LostRecords/Unobserved、Silence と
  Missing の区別。
- SF-PROBE-06: 同一 Semantic Point の複数 Window/Inspector で
  Broker Capture 重複なし（session 層 lease = slice5）、player 層
  は per-key 冪等。Window Close/Move/Session stop/Capture Abort の
  ownership。
- SF-PROBE-07: 複数 Probe の Clock Domain/Epoch/Latency/Format/
  Gaps 比較、Seek/Loop/Device Restart の Unknown/Incomparable。
  #309 は CLOSED、実測 Gate は NotRun。
- SF-PROBE-08: Diagnostics first observed drop vs proven root
  cause、Plugin unavailable/Probe Off/PCM silence/No Data の状態
  分離。headless 層は状態語彙の分離まで、原因候補の解釈と UI
  分離・Accessibility は #108/#109。
- Unknown/Unobserved を Silence 値に置換しない。RT callback で
  PCM Copy を無制限化しない。

## Acceptance Criteria

- [ ] `sf04_probe_acceptance.rs` の 2 テストが entity 消失 unbind・
  再 attach fresh bind・per-key 冪等 dedup・session 境界を検証し、
  `mise run test synvelis-player` で green。
- [ ] `evidence/sv107/` に SF-PROBE-01〜08 の被覆表（既存テスト
  への写像・新規テスト・NotRun/Unsupported 判定）がある。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`（synvelis-project・
  synvelis-player）が green。
- [ ] 文書チェーン（spec/design/plan/task/test-spec/ir/exp）が
  作成され、触った docs dir の index が再生成されている。

## Deferred Design Concerns

- 実機/実 plugin/実 device での Probe 動作・RT 計測 | #109・
  EXT-L3 gate | 実環境が必要 | headless Fixture の成功を実機
  Pass にしない。
- Probe 状態の UI 分離・Accessibility・Diagnostics 原因候補表示
  | #108/#109 | UI/Domain Oracle slice | headless は状態語彙の
  分離までが責務。
- session 層の subscriber lease/capture owner | slice5
  （synvelis-workspace、merge 済み） | 既に実装済み | 本タスクは
  player 層側の per-key dedup 境界のみを固定する。
