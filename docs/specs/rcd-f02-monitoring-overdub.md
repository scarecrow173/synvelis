---
id: "SPEC-034XJxBZyIYAfzQFLeIAWA"
type: "spec"
status: "approved"
title: "RCD-F02 Advanced Monitoring / Overdub Layering / Pre-Punch Handles — 要求仕様（synvelis#120）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/120"
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
    - "docs/designs/rcd-03-app-arm-monitoring-ux.md"
    - "docs/designs/rcd-03-monitor-punch-design.md"
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/designs/recording-take-comping-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
    - "docs/adr/rcd-f02-monitoring-overdub.md"
    - "evidence/sv120/rcd-f02-monitoring-overdub-adr-decisions.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/120"
  derived-by:
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
    - "docs/adr/rcd-f02-monitoring-overdub.md"
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-f02-monitoring-overdub.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/120"
    - "https://github.com/scarecrow173/synvelis/issues/117"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/118"
    - "https://github.com/scarecrow173/synvelis/issues/12"
  defers: []
  deferred-by: []
---

# RCD-F02 Advanced Monitoring / Overdub Layering / Pre-Punch Handles — 要求仕様（synvelis#120）

## What Should Be Built

Issue #120（future/recording tracker）が要求する、Advanced Monitoring / Overdub Layering / Pre-Punch Handles の**設計合意**。実装ではなく、以下を canonical 文書チェーンと ADR として固定する docs-only slice:

- Monitor Off/Auto/On × Arm × Overdub × Punch × Playback の可聴 route UX Matrix
- Feedback Risk（二重監視・comb filtering・Hardware Direct 不可制御表示）の警告 semantics
- Capture Handle Retention（Punch 外 pre/post capture handle の consent-gated journal 記録・保存契約）
- Undo/Recovery（handle 素材の undo・device 切断・journal replay verified prefix との整合）
- Privacy（monitor 内容・handle 素材を consent なしに永続化しない境界）

成果物は `docs/adr/rcd-f02-monitoring-overdub.md` を本体とし、`spec → design → plan → task → test-spec → impl/ir + impl/exp` の文書チェーンと `evidence/sv120/` の決定記録・検証記録で構成する。

## Why Now

Issue #120 本文は「初期実装前に UX Matrix（Off/Auto/On×Arm×Overdub×Punch×Playback）、Feedback Risk、Capture Handle Retention、Undo/Recovery と Privacy を ADR で合意する」を必須化している。RCD-03（#117）で monitor/punch/latency の baseline 契約は確定済みだが、Overdub 同時混合・pre/post capture handle・consent/privacy の語彙は未定義であり、実装 slice（後続 dispatch）に先立ち第二正本を作らない形で合意が必要。

## Users and Value

- 実装 Worker: ADR/design の契約に従い `synvelis-recording`/`synvelis-player`/`synvelis-app` を変更できる。
- Coordinator/Owner: Issue #120 の受入条件に対する設計合意の記録をレビューできる。
- 将来の保守者: route/gate/handle/privacy の責務境界を第二正本なしに追跡できる。

## Scope

### In Scope

- UX Matrix・Feedback Risk・Capture Handle Retention・Undo/Recovery・Privacy の 5 領域の ADR 決定記録
- 既存契約（`monitor.rs` `decide`/`MonitorRoute`/`MonitorNotices`/`HardwareDirectState`、`gate.rs` `GateScheduler`/`evaluate_gate`/`GateSpec`、`journal.rs` append-only framed record、`synvelis-app` monitoring session 投影、RCD-02 verified prefix recovery、take/comping 非破壊保存）を baseline とした拡張境界の定義
- 後続実装 slice の想定ファイル・検証方針（Implementation Plan。実装自体は対象外）
- `evidence/sv120/` への決定要約・検証記録

### Out of Scope

- Rust コード・テスト・fixture の変更（実装 slice は ADR merge 後に直列 dispatch）
- 実 OS/実 Device/実 GUI/実 Plugin の受入（GUI-L2/EXT-L3、#114 系の別 Gate）
- `crates/**`、`Cargo.toml`/`Cargo.lock`、`mise.toml`、CI、`POLICIES.md`、`AGENTS.md` の変更
- 既存 broken relation の修復（pre-existing、範囲外）
- Issue #120 全体の close 宣言（`Refs #120`。Close は Coordinator/Owner 判断）

## Requirements

- **REQ-F02-01 UX Matrix**: Monitor Off/Auto/On × Arm × Overdub × Punch × Playback の可聴 route 行列を定義する。既存 `decide` 判定表を baseline とし、Overdub 次元の追加方法（route 値追加か並列表現か）を ADR で明示決定する。
- **REQ-F02-02 Feedback Risk**: `MonitorNotices::{double_monitoring,hardware_direct_unknown,input_unavailable}` を正本として再利用し、Overdub 同時混合・Loop pass・Track Direct Monitor での警告発火条件と表示義務を定義する。必要な新 notice は ADR の決定として明示し、app 側で独自名を発明しない。
- **REQ-F02-03 Capture Handle Retention**: Punch 外 pre/post capture handle は「明示有効化した区間だけ」journal 記録・保存する。User Consent 記録・保存容量・SourceRange/Clock 証拠の必須属性集合と retention 規則を定義し、無許可の隠し Media 生成を禁止する。
- **REQ-F02-04 Undo/Recovery**: handle 素材の undo・device 切断・recovery 時の journal/保存 semantics を RCD-02 の verified prefix 契約に整合させる。Undo と Recovery を別 semantics として定義する。
- **REQ-F02-05 Privacy**: monitor 内容（input PCM・handle 素材）は consent なしに永続化しない。保持期間・容量上限・削除/undo 経路を決定する。
- **REQ-F02-06 Honest-capability**: Hardware Direct Monitor は `DeviceControlled`/`UserReported`/`Unknown` の 3 分類のまま扱い、Synvelis DSP で制御を保証しない能力を「制御可能」と表示しない。`Unknown` を制御可能と表示せず、外部観測値を DAW 側で変更しない。
- **REQ-F02-07 Raw Recorder PCM 不変**: monitor route・Overdub・handle 判定は raw capture・capture writer を変更しない。handle 素材は consent 区間のみの追加 capture であり、take 素材の span/PCM を変えない。

## Acceptance Criteria

- [ ] **AC-F02-01**: `docs/adr/rcd-f02-monitoring-overdub.md` が MADR 形式で 5 領域（UX Matrix・Feedback Risk・Capture Handle Retention・Undo/Recovery・Privacy）の決定を記録し、`docs/adr/README.md` index に登録される。
- [ ] **AC-F02-02**: UX Matrix が既存 `decide` 判定表を baseline として転記し、Overdub 次元の route 表現拡張方法が ADR の決定として明示されている（暗黙の拡張なし）。
- [ ] **AC-F02-03**: Feedback Risk が既存 notice 名を正本として再利用し、新 notice が必要な場合は ADR で決定として記録されている。
- [ ] **AC-F02-04**: Capture Handle の必須属性（User Consent・保存容量・SourceRange/Clock 証拠）と retention 規則が ADR/design に定義され、「明示有効化区間のみ journal 記録・保存・無許可の隠し Media なし」が契約化されている。
- [ ] **AC-F02-05**: Undo/Recovery が RCD-02 verified prefix 契約に整合し、Undo（決定的・project/session command・stable-ID 境界尊重）と Recovery（verified journal/media prefix・未検証/torn data を受理済み capture として露出しない）が別 semantics として定義されている。
- [ ] **AC-F02-06**: Privacy が「monitor 内容は consent なしに永続化しない」「punch 外素材は明示区間のみ journal/保存」を決定文として含み、保持期間・容量上限・削除/undo 経路を定義している。
- [ ] **AC-F02-07**: Hardware Direct の honest-capability（3 分類・制御非保証・`Unknown` を制御可能と表示しない）が ADR/design に明記されている。
- [ ] **AC-F02-08**: 8 文書 doc chain（spec/design/plan/task/test-spec/adr/impl-ir/impl-exp）が canonical front matter・relations・status で揃い、触れた全 dir の管理 index が `in-sync`。
- [ ] **AC-F02-09**: `evidence/sv120/` に ADR 決定記録（5 領域の決定要約 + 採用/却下選択肢）と実行検証記録（コマンド・exit code・差分要約、docs-only・cargo 未実行の正直な記録）がある。
- [ ] **AC-F02-10**: `mise run fmt-check`・prek・doc-status 監査・impl audit が実行され、pre-existing failure と slice 起因 failure が区別されている。review-protocol で blocker 0。

## Deferred Design Concerns

- 実装 slice（`synvelis-recording` Overdub/Explicit Pre-Punch Handle/Take Layer Policy、`synvelis-player` Monitor Cue、`synvelis-app` Consent/Warning UX・Fixture）| ADR merge 後・直列 dispatch | Issue #120 merge 後 | 設計のみでは実動作を保証できない
- 実 OS E2E（Loop/Take/Comp/Device 切断/Undo/Recovery・RT noalloc）| GUI-L2/EXT-L3 の別 Gate | #114 系受入 slice | headless 証拠を実機 Pass に換算できない
- RT graph への monitor node 搭載 | RCD-03 で Deferred 済み・本 slice でも対象外 | RT graph slice | monitor engine は現状 RT callback 外
- Project-level consent 永続化（「常に許可」の保存）| session-scoped grant で要件を満たすため YAGNI | 実利用で要望が出た場合 | consent を project に残すと privacy 境界が複雑化する

## Sources and Relations

- Issue #120（future/recording RCD-F02 tracker）— 受入条件・後続スコープの正本。
- `crates/synvelis-recording/src/monitor.rs` — `decide` 判定表・`MonitorRoute`・`HardwareDirectState`・`MonitorNotices` の既存契約（read-only input）。
- `crates/synvelis-recording/src/{gate,plan,session,journal}.rs` — gate 区間・session 遷移・journal verified prefix の既存契約。
- `crates/synvelis-app/src/monitoring.rs` — Arm/Monitoring UX op・session-scope 状態投影。
- `crates/synvelis-player/src/{monitor,monitor_engine}.rs` — MonitorAdapter gain ramp・MonitorEngine 合成の既存契約。
- `docs/designs/rcd-03-*`・`docs/designs/rcd-02-*`・`docs/{specs,designs}/recording-take-comping*` — 確定済み baseline 契約。
- `evidence/sv117/` — RCD-03 monitor session の受入証拠先例。
- Issue 依存: RCD-01/03/04・`#12`・SF-02 `#105`・SV-GUI-05 `#114`。統合・受入関連: `scarecrow173/ruviform#627`/`#628`/`#629`/`#631`・synvelis `#117`/`#118`。
