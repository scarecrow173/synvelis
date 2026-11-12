---
id: "IMPL-034VWypvBUG9TnfSAsy2zP"
type: "impl"
status: "completed"
title: "SF-03 slice9 実装記録 — host capability generation live firing"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/sf03_l9.rs"
      - type: "file"
        path: "docs/specs/sf-03-slice9-capability-firing.md"
      - type: "file"
        path: "docs/designs/sf-03-slice9-capability-firing-design.md"
      - type: "file"
        path: "docs/plans/sf-03-slice9-capability-firing-plan.md"
      - type: "file"
        path: "docs/tasks/sf-03-slice9-capability-firing-task.md"
      - type: "file"
        path: "docs/test-specs/sf-03-slice9-capability-firing.md"
      - type: "file"
        path: "docs/impl/ir/sf-03-slice9-capability-firing.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/host.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_engine.rs"
      - type: "file"
        path: "crates/synvelis-project/src/routing_build.rs"
      - type: "file"
        path: "crates/synvelis-project/src/routing_tx.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice9-capability-firing.md"
    - "docs/designs/sf-03-slice9-capability-firing-design.md"
    - "docs/tasks/sf-03-slice9-capability-firing-task.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-03-slice9-capability-firing-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-slice9-capability-firing-design.md"
    - "docs/specs/sf-03-slice9-capability-firing.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sf-03-slice9-capability-firing.md"
  verified-by: []
  references:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-03 slice9 実装記録 — host capability generation live firing

## Summary

Issue #106 SF-03 の残件「`PluginCapabilityChanged` が
`SpeculativeMismatch` に shadow されて live 発火しない」を、
host 側 capability generation の新設で解消した。
`ProcessorHost::capability_generation`（default `0`）を trait に
追加し、`PluginHostProcessors` が registration epoch +
composite `PluginHost` の `Scan` descriptor を FNV-1a で畳み
込んだ決定的 generation を返す。`RoutingEngine` は prepared
bundle ごとに host generation を pin し、publish 再検証・
ledger pin・`NoRebuild` equivalence・reconcile の全 gate に
fold した。`RoutingBuildKey` の構造と
`port_capability_generation`（project 側半分）の意味は不変 —
host 状態は project データではなく key に入れられないため、
key の外の engine pin として設計した。

## Context

slice2 の fault taxonomy は `PluginCapabilityChanged` を定義
したが、検出対象の capability 状態が project 側
`capability_revision` 和のみで、host 側登録
（`PluginHostProcessors` の boundary/catalog・factory/route
登録）はどの gate にも現れなかった。project fingerprint も
host を見ないため、host drift は検出経路を持たず fault は
defensive-only だった（map design §8.2 残件、slice2 design
L254-267）。本 slice は host 側 generation を engine の
capability pin に fold し、publish/reconcile での live 発火を
headless fixture で固定する。

## Implementation

### Changes

- `crates/synvelis-player/src/host.rs`:
  - `ProcessorHost::capability_generation(&self) -> u64`
    （default `0`）を trait に追加。`StaticProcessorHost` は
    provisioning 証拠を compiled plan に消費し登録 catalog を
    持たないため default のまま。
  - `PluginHostProcessors.registration_epoch: u64` を追加し、
    `register_boundary`/`register_params` が saturating add で
    bump — adapter 所有の登録は `Scan` から見えないため epoch
    で覆う。
  - `capability_generation` 実装: epoch bytes を FNV-1a で
    畳み込み、続けて `host.borrow_mut().execute(
    HostCommand::Scan)` の descriptor（id/name/vendor/version/
    features）を fold。composition root が共有
    `Rc<RefCell<PluginHost>>` handle 経由で直接
    `register_factory` する drift は adapter epoch を通らない
    ため `Scan` 軸が必須。
  - `fnv_fold` helper — project 側 `port_capability_generation`
    と同じ FNV-1a change-detector パターン。
- `crates/synvelis-player/src/routing_engine.rs`:
  - `PreparedRouting.host_capability_generation: u64` —
    `prepare` が `checked_key` 通過直後・connect 開始前に
    capture（connect による instance 消費は drift ではなく
    build の一部）。
  - `Ledger.published_host_capability_generation:
    Option<u64>` — publish 成功時に prepared pin を保存。
  - `equivalence`: `live.semantic_eq(key)` に加え
    `published_host_capability_generation ==
    Some(host.capability_generation())` を要求 — host drift
    下で semantic 同一 revision が `Built` に倒れる。
  - `publish`: fingerprint 一致の次に project 側 sum と
    `prepared.host_capability_generation !=
    self.host.capability_generation()` の 2 半分を検査し、
    不一致を `PluginCapabilityChanged` で拒否
    （`SpeculativeMismatch` に shadow されない）。
  - `reconcile`: 同じ 2 半分比較で
    `RoutingReconcileMismatch::CapabilityGeneration` を push
    → 既存 precedence で `PluginCapabilityChanged` fault・
    `Blocked` receipt。live plan は継続。
  - module/fn doc を 2 半分 capability モデルに更新。
- `crates/synvelis-project/src/routing_build.rs`・
  `routing_tx.rs`: `capability_generation`/
  `port_capability_generation` が project 側半分のみを含み、
  host 側は `RoutingEngine` が別軸で pin することを doc 明記
  （構造・値・比較規則は不変）。
- `crates/synvelis-player/tests/sf03_l9.rs`（新規・5 件）:
  - `sf03_l9_01_host_generation_tracks_registrations`:
    baseline 安定 → `register_params` で変化 →
    `register_boundary` で再変化 → 共有 handle 経由
    `register_factory` で再変化 → 再び安定。
  - `sf03_l9_02_publish_rejects_factory_drift`: gen1
    Active 化 → gen2 prepare → factory drift → publish が
    `PluginCapabilityChanged` + `Blocked{last_good:1}` +
    `active_generation==1` → drift 後の再 prepare/publish が
    成功し新 generation が active。
  - `sf03_l9_03_reconcile_reports_host_capability_drift`:
    clean reconcile（mismatches 空）→ drift → reconcile が
    `[CapabilityGeneration]` + `PluginCapabilityChanged` +
    `Blocked` receipt、plan は live で run が音声を生成。
  - `sf03_l9_04_host_drift_defeats_equivalence`: semantic
    同一 revision（rename）で drift 有り engine は `Built`、
    drift 無し対照 engine は `Equivalent`。
  - `sf03_l9_05_host_swap_is_capability_drift`: prepare→
    publish 間の `set_processor_host` 差し替えで
    `PluginCapabilityChanged`。
- docs: spec/design/plan/task/test-spec/impl record 新設。
  `docs/*/README.md` index は dispatch 範囲外のため未更新。

### Decisions During Implementation

- **host generation は `RoutingBuildKey` に入れない**:
  `RoutingBuildKey` は `synvelis-workspace`/`synvelis-render`
  の test が literal 構築し `semantic_eq` で比較する project
  snapshot の純粋関数。host 状態は project データではなく
  persisted でもないため、key ではなく `PreparedRouting` +
  engine ledger の別軸 pin とした。field 追加は占有外 crate の
  test を壊すため不可でもあった。
- **capture は connect 前**: `prepare` が `checked_key` 直後に
  generation を取る。`connect` が instance を消費しても
  登録状態は変わらず、値は「この build が仮定する provisioning
  状態」を表す。publish 時の再評価との差分が drift。
- **`Scan` + epoch の 2 軸**: `register_factory` は adapter を
  介さず共有 `PluginHost` handle から直接呼ばれるため epoch
  では捕捉不可。逆に adapter 所有の boundary/catalog 登録は
  `Scan` の descriptor 表に現れない。両方を畳み込んで全登録
  軸を覆う。
- **`StaticProcessorHost` は default `0`**: provisioning 証拠が
  connect 時に compiled plan へ消費され登録 catalog を持た
  ない。既存 sf03_l2/l3 fixture は capability 経路不変で
  非回帰。
- **host swap も drift**: `set_processor_host` で別 host に
  差し替えると generation が変わり publish が
  `PluginCapabilityChanged` — provisioning 証拠の変化として
  意図した検出（安全側）。

## Related Experiments

なし。

## Validation

- `cargo test -p synvelis-player --test sf03_l9`:
  5/5 pass（補助証拠・実測）。
- `mise run fmt-check`: exit 0。
- `mise run check`: exit 0。
- `mise run lint` 以降の canonical lanes: PR 本文の検証節に
  実測値を記録（実行結果は worker 報告を参照）。

## Fault / Gate 判定表（matrix）

| 対象 | 判定 | 根拠 |
| --- | --- | --- |
| `PluginCapabilityChanged` publish live firing | **Pass** | `sf03_l9_02`/`sf03_l9_05` — factory drift・host swap で実発火、`SpeculativeMismatch` に shadow されない |
| reconcile `CapabilityGeneration` | **Pass** | `sf03_l9_03` — `[CapabilityGeneration]` mismatch + `PluginCapabilityChanged` fault + `Blocked` receipt、plan live 継続 |
| `ActivationNotObserved` | **NotRun** | engine epoch/instance は `RoutingEngine::new` で構築時 pin、遷移 API 無し。published plan の RT activation を正当に withhold する経路が存在せず、fault-injection seam の新設は scope 外 |
| `RetireOverflow` | **NotRun** | Ruviform retire channel の overflow が前提。headless 決定的再現には upstream fault-injection seam が必要 — ruviform 側 Issue 候補 |
| reconcile `DeviceEpoch` | **NotRun** | epoch/instance 遷移 API が無く mismatch を構成できない。device epoch 遷移設計は別 slice |
| 実 VST3 での host drift | **NotRun** | 外部 gate（実 plugin host の factory unload/scan 変化）。実機/GUI lane に委譲 |
| 実機・GUI・RT 実測 | **NotRun** | 外部 gate — headless 範囲外 |
| SF-ACT-07 preview policy | **NotRun** | 未決定・別 Issue 候補（slice8 と同じ扱い） |

## NotRun（正直な記録）

- `ActivationNotObserved`: 上記 matrix の通り発火経路なし。
  defensive fault のまま残る（仕様上許容 — 発火義務は正当な
  経路が存在する場合に限る）。
- `RetireOverflow`/`DeviceEpoch`/`EventStale` の live firing:
  headless 再現不可または発火経路なし。
- 実 VST3・実機 device・GUI・RT latency 実測: 外部 gate。
- `synvelis-app`/`synvelis-workspace` session 経路での
  end-to-end 発火: `EnginePort` は `RoutingEngine` をそのまま
  委譲するため host drift は session 経路でも同じ gate を通る
  が、app/workspace 側の占有範囲のため本 slice では
  `RoutingEngine` 直下の headless 証拠に留める。
- Issue #106 の Close: Coordinator が行う（本 slice は Open の
  まま）。

## Risks

- `Scan` descriptor 列の順序が host 内部 table 依存 — 固定
  状態では安定（fixture が連続 2 回の同一性を assert）だが、
  ruviform 側で順序が非決定的になると false positive があり
  得る。現行 pinned rev では問題なし。
- FNV-1a は change detector であり衝突の暗号学的保証なし —
  project 側 `capability_generation` と同じ既存パターン。
- host swap は同一内容の再構築でも `PluginCapabilityChanged`
  になる — 安全側の fail-closed であり意図した挙動。

## Follow-ups

- `ActivationNotObserved`/`RetireOverflow`/`DeviceEpoch` の
  defensive-only fault は、正当な発火経路（RT activation
  withhold 観測・upstream retire seam・epoch 遷移 API）が
  設計された時点で別 slice/Issue として再評価。
- 実 VST3 host での factory unload/scan drift 検証は外部
  gate lane。
- `docs/*/README.md` managed index の再生成は本 dispatch の
  範囲外 — doc-maintenance で別途実施。
