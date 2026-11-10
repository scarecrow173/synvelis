---
id: "SPEC-034V2wnQPWFJdKz68KlwJG"
type: "spec"
status: "approved"
title: "SF-06 cross-gate Domain Oracle・Case ID・Fault Fixture・evidence ledger（headless slice）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
    - "https://github.com/scarecrow173/synvelis/issues/18"
    - "https://github.com/scarecrow173/ruviform/issues/193"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
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
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/adr/sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md"
    - "docs/designs/sf-06-cross-gate-oracle-domain-evidence.md"
    - "docs/plans/sf-06-cross-gate-oracle.md"
    - "docs/test-specs/sf-06-cross-gate-oracle.md"
    - "docs/tasks/sf-06-cross-gate-oracle-headless.md"
    - "docs/impl/ir/sf-06-cross-gate-headless-oracle.md"
    - "evidence/reports/sf-06-cross-gate-ledger.md"
  defers: []
  deferred-by: []
---

# SF-06 cross-gate Domain Oracle・Case ID・Fault Fixture・evidence ledger（headless slice）

## What Should Be Built

Issue #109（SF-06 Routing/Plan/Probe/GUI Cross Gate — Fault Matrix・
OS/RT/Device/Consumer 別受入）の **headless で成立する部分**。
SF-01〜SF-05 の個別 slice 受入を横断する独立検証基盤を作る。

- **Domain Oracle**: 実装内部を参照しない独立した期待値導出。
  `AudibilityOracle` は mute/solo の documented policy
  （SF-02 SF-TX-07/08）を persisted 生フィールドから再実装し、
  `RoutingModel::audible_strips` と全組合せで field-by-field 比較
  する。`routing_invariant_violations` は persisted 契約
  （master 一意・strip/port/chain/send の参照整合・方向合法・
  非循環・mute/solo 単一権威）を `validate_references` を介さず
  再検査する。
- **Case ID registry**: `SF-MODEL-01〜05`・`SF-VALID-01〜08`・
  `SF-ACT-01〜08`・`SF-PROBE-01〜08`・`SF-UX-01〜06`・
  `SF-COMPAT-01〜06` の安定 Case ID を ledger に登録し、各 Case
  の headless/実機/consumer の lane 別結果を `Pass`/`Fail`/
  `NotRun`/`Unsupported` で記録する。
- **Fault Fixture**: stale CAS・stale capability generation・
  feedback cycle・duplicate temp ID・illegal kind/map・参照中
  delete 等の拒否系 gesture が Live project・Revision・Undo を
  不変に保つことを検証する fault matrix。
- **Evidence ledger**: `caseId | Synvelis SHA | Ruviform resolved
  SHA | OS/CPU/FS/Device+Plugin | Graph/Project/Clock generations |
  fixture & command | expected vs observed | Pass/Fail/NotRun/
  Unsupported | resource/performance/fault | CI/PR link` の
  schema で `evidence/reports/sf-06-cross-gate-ledger.md` に記録。

### 提供するもの

1. `crates/synvelis-project/tests/sf06_oracle.rs`（新規）:
   - `AudibilityOracle` — mute×solo 全組合せ sweep（mix
     topology 7 strip = 2^14、chain topology 4 strip = 2^8、
     disabled-edge variant 含む）で `audible_strips` と独立導出を
     比較。
   - `routing_invariant_violations` — persisted 契約の独立再検査
     ＋ oracle 自体が破損を検出できることの自己検証
     （orphaned send・第二 master・duplicate edge・duplicate mute
     権威・dangling endpoint）。
   - fault matrix — 拒否 gesture ごとに serialized project +
     revision が byte 不変であること、および事後の成功 op の
     undo が fault 前状態へ正確に戻ることを検証。
   - v5→v6 migration oracle — 宣言 fixture から期待 legacy
     投影（track 毎 strip + master 直結 Main edge・processor
     捏造なし）を導出し、再 migration の決定性を検証。
2. `evidence/reports/sf-06-cross-gate-ledger.md` — Case ID registry
   と lane 別結果の ledger。headless lane のみ実測値を記録し、
   実 OS GUI・native plugin・device・consumer（Synvelis #219/
   #294、Ruviform consumer gate）の行は `NotRun` とする。
3. docs チェーン（本 spec・ADR・design・plan・test-spec・task・
   impl record）。

## Why Now

Issue #109 の 2026-11-09 着手依存監査で「#104 の既存 Routing
契約を利用し #106/#107/#108 と並行して Domain Oracle・Case ID・
Failure Fixture を作成できる（#108 完了を着手条件にしない・
テストは並行実装で全面 PASS は最終受入）」と明記済み。SF-01〜
SF-05 の個別受入は揃いつつあるが、cross-gate の独立 oracle と
証跡 ledger がないと「どの lane で何が未検証か」を正直に
報告できない。本 slice はその土台を headless 範囲で確立する。

## Users and Value

- Synvelis メンテナ・レビュア: SF-06 の各 Case がどの lane
  （headless / 実 OS GUI / device+plugin / consumer）で検証済みか
  を ledger 一箇所で追跡できる。
- #106/#107/#108 の並行実装者: 契約（#104 Routing・#106
  RoutingControlDispatch・#107 Probe epoch・RF#309 Align）を
  変えずに cross-gate 期待値を先に固定できる。
- 最終受入者: headless Pass を実機受入と誤認しない構造
  （NotRun/Unsupported の明示）を得る。

## Scope

### In Scope

- `crates/synvelis-project/tests/sf06_oracle.rs` の新規追加
  （test/fixture 領域のみ）。
- `evidence/reports/sf-06-cross-gate-ledger.md` の新規追加。
- 本 docs チェーン。
- 既存契約の**利用**: #104 Routing model/transaction、#106
  RoutingControlDispatch、#107 Probe epoch、Ruviform #309
  Frame/Clock/Epoch/Latency alignment。別 Clock/Probe 正本は
  作らない。

### Out of Scope

- #106/#107/#108 の実装本体の変更（並行実装中 — 契約は利用のみ）。
- SF-ACT/SF-PROBE/SF-UX/SF-COMPAT のうち runtime engine・GUI・
  device・plugin・consumer を要する Case の実施 → ledger 上
  `NotRun`（最終受入は別 gate）。
- 実 OS GUI（#114）・native plugin dock（#176）・consumer 実機
  （Synvelis #219/#294、AudioLens）の受入。
- Ruviform generic Graph/Probe/Clock/GUI primitive の変更。
  generic 変更が必要になった場合は両 consumer check・Synvelis
  #28・AudioLens no-impact 根拠を別途要求する。
- EX-001〜020（#18）・RF#193 G2/G7・RF#308/#309 の evidence の
  **再採取** — ledger から既存 evidence へ link するのみ。

## Requirements

- Domain Oracle は実装内部（private fn・内部状態）を参照せず、
  persisted 公開フィールドと documented policy のみから期待値を
  導出する。
- Oracle 自体が常に合格するトートロジーを避けるため、破損検出
  能力を自己検証する（corruption fixture で非空 violation を
  確認）。
- 拒否系 fault は「reject される」だけでなく「Live/Revision/
  Undo が不変」を検証する。
- headless Pass を実 OS GUI/device/plugin/consumer 受入へ昇格
  しない。未計測の lane は `NotRun`、環境非対応は
  `Unsupported` と記録する。
- close 済み generic Ruviform #184/#185 を本 Gate の拡大だけで
  未実装扱いしない。
- AI/VCS/Signal Flow 全機能完成を Gate A 前提にしない。

## Acceptance Criteria

- [ ] `cargo test -p synvelis-project --test sf06_oracle` が
  exit 0 で、audibility oracle sweep・invariant oracle・fault
  matrix・v5 migration oracle の 6 test が全て Pass。
- [ ] `evidence/reports/sf-06-cross-gate-ledger.md` が Issue #109
  の要求 schema 全 column を持ち、SF-MODEL/VALID/ACT/PROBE/UX/
  COMPAT の Case ID を網羅し、実機・consumer 行を `NotRun` で
  記録している。
- [ ] EX-001〜020・RF#193 G2/G7・RF#308/#309・Synvelis #114/#176/
  #219/#294 への link が ledger に存在する。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt` が
  exit 0。
- [ ] docs 索引が doc-maintenance の preview→apply で再生成済み。

## Deferred Design Concerns

- SF-ACT-01〜08 の publish/QueueFull/last-good/RT-Active receipt
  系 — runtime engine（#106 dispatch・session engine）の実装が
  並行中のため本 slice では headless project 層まで。最終受入は
  SF-03 slice4/slice5 の ACT evidence と統合する別 gate。
- SF-PROBE-01〜08 の capture/alignment/epoch 系 — #107 の probe
  epoch 実装確定後に headless fixture を拡張する。Ruviform #309
  Align 契約の再実装はしない。
- SF-UX-03〜06 の編集/Follow/Probe 共有 — #108 の UI 実装と
  #114 の実機 gate に依存し NotRun。
- SF-COMPAT-01〜06 — DAW Gate A/B/C・Recording/Offline/
  Automation・AudioLens f64・lockfile 互換は各担当 issue の
  evidence を link するのみで、本 slice では再計測しない。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/109>
- Case family の上流: Synvelis #104（Routing 契約）・#105
  （transaction）・#106（RoutingControlDispatch）・#107（Probe
  epoch）・#108（Signal Flow headless）・#18（EX-001〜020）・
  Ruviform #193（G2/G7）・#308（Probe capture/rebind）・
  #309（Frame/Clock/Epoch/Latency alignment）。
- [ADR](../adr/sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md) /
  [design](../designs/sf-06-cross-gate-oracle-domain-evidence.md) /
  [plan](../plans/sf-06-cross-gate-oracle.md) /
  [task](../tasks/sf-06-cross-gate-oracle-headless.md) /
  [test-spec](../test-specs/sf-06-cross-gate-oracle.md) /
  [impl record](../impl/ir/sf-06-cross-gate-headless-oracle.md) /
  [evidence ledger](../../evidence/reports/sf-06-cross-gate-ledger.md)
