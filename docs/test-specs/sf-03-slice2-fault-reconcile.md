---
id: "TSPEC-106SF03Vz2aBc3dEf4gHi5jKl6"
type: "test-spec"
status: "approved"
title: "SF-03 slice2 テスト仕様 — Fault 分類・Last-good/Retry・Reconcile"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
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
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  verified-by: []
  references: []
---

# SF-03 slice2 テスト仕様

対象: `crates/synvelis-player/tests/sf03_l2.rs`（新規）+
`sf03_l1.rs` の改名追従。
実行: `mise run test`（headless）。実 plugin host・GUI・device は
NotRun/Unsupported（後続 slice）。

## Feature: Fault 分類の mint 経路

| Rule | Example | 期待 |
| --- | --- | --- |
| revision 不一致は `StaleProjectSnapshot` | expected≠revision の prepare/publish | typed fault・last-good 不変 |
| fingerprint drift は `SpeculativeMismatch` | 同一 revision で内容の異なる project を publish に渡す | `Blocked{SpeculativeMismatch}`・publish count 0 |
| capability drift は `PluginCapabilityChanged` | `port_capability_generation` を変えた project で publish | `Blocked{PluginCapabilityChanged}` |
| instance/epoch 不一致は `DeviceEpochChanged` | 別 instance/epoch の request で prepare | `Failed{DeviceEpochChanged}` |
| frame budget 不足は `CompileResourceExceeded` | `max_frames < block_frames` の request | `Failed{CompileResourceExceeded}` |
| lowering budget 超過は `CompileResourceExceeded` | `max_nodes`/`max_connections` 極小の budget | `Failed{CompileResourceExceeded}` |
| 未解決 port 参照は `PortUnresolved` | port entity を欠く project clone で prepare | `Failed{PortUnresolved}` |
| channel 不一致は `UnsupportedFormatOrTimeMap` | strip port の channels を engine と変えた clone | `Failed{UnsupportedFormatOrTimeMap}` |
| 非 audio domain は `UnsupportedFormatOrTimeMap` | port domain を非 Audio に変えた clone | `Failed{UnsupportedFormatOrTimeMap}` |
| 構造不正は `InvalidRouting` | master 不在・processor port 欠落などの clone | `Failed{InvalidRouting}` |
| instance 未供給は `PluginUnavailable` | `ProcessorBinding::Unresolved` chain（sf03_l1 既存） | `Failed{PluginUnavailable}` |
| 順序違反 publish は `Superseded` | gen2 publish 後に gen1 prepared を publish | `Blocked{Superseded}`・gen2 live 維持 |

## Feature: Skipped 昇格と Offline gate

| Rule | Example | 期待 |
| --- | --- | --- |
| MissingSource は fail closed | source を解決できない clip を含む project で prepare | `Failed{MissingSource}`・fake silence なし |
| RateMismatch は `UnsupportedFormatOrTimeMap` | source rate ≠ engine rate の clip | `Failed{UnsupportedFormatOrTimeMap}` |
| Muted/SupersededByComp/CompSilence は許容 | muted clip を含む project で prepare+publish+run | 成功・無音は契約どおり |
| Offline + 未検証 processor は `OfflineCapabilityMismatch` | `BuildMode::Offline` で `offline_support() != Verified` の instance | `Failed{OfflineCapabilityMismatch}` |
| Offline + Verified processor は成功 | `Verified` を返す test processor | prepare/publish 成功 |

## Feature: Last-good 行列

| Rule | Example | 期待 |
| --- | --- | --- |
| Active 後の全失敗で last-good 保持 | gen1 Active → 各種失敗を注入 → `last_good_generation == Some(1)` | receipt・active_generation 不変 |
| 順序違反 publish で last-good 保持 | gen2 Active → gen1 prepared publish → `Superseded` | `last_good == Some(2)`・gen2 PCM 継続 |
| retry は fresh build で成功 | 失敗後に新 revision で prepare→publish→run | 新 generation が Active・last-good 前進 |
| Active receipt の `frames_observed` 保持 | Active 遷移後の receipt | `frames_observed > 0`（pending クリアで 0 に戻らない） |

## Feature: Reconcile

| Rule | Example | 期待 |
| --- | --- | --- |
| drift なしは空 report | publish+activate 後に同一 snapshot で reconcile | `mismatches` 空・`fault` None・receipt 不変 |
| revision drift | project に commit を追加して reconcile | `ProjectRevision` mismatch・`StaleProjectSnapshot` fault・旧 plan live |
| fingerprint drift | 同一 revision の別内容 project で reconcile | `RoutingFingerprint`・`SpeculativeMismatch` |
| capability drift | capability generation の異なる project | `PluginCapabilityChanged` |
| engine/source format drift | generation 引数を変えて reconcile | `UnsupportedFormatOrTimeMap` |
| pending 未活性化 | publish 後 run 前に reconcile | mismatch/fault なし（過渡的） |
| publish 済み plan 無し | 未 publish engine で reconcile | 空 report |
| stale counter 前進 | stale command を RT に残す経路で counter を進めて reconcile | `EventStale`（実現可能な範囲で） |
