---
id: "DESIGN-034VWsBTFsvYFbqeDihe3Z"
type: "design"
status: "approved"
title: "SF-03 slice9 — host capability generation live firing design"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/sf-03-slice9-capability-firing.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice9-capability-firing.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sf-03-slice3-plugin-host-connect-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice9-capability-firing.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  derived-by:
    - "docs/plans/sf-03-slice9-capability-firing-plan.md"
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  defers: []
  deferred-by: []
---

# SF-03 slice9 — host capability generation live firing design

## Overview

capability drift の検出は 2 つの半分からなる。

- **project 側**: `RoutingBuildKey.capability_generation` —
  `synvelis_project::port_capability_generation(project)` が
  persisted `capability_revision` の和を返す。build key の一部
  であり fingerprint と同じく project snapshot の純粋関数。
- **host 側**: `ProcessorHost` が provisioning する
  boundary/catalog/factory 登録状態。project snapshot にも
  fingerprint にも現れない runtime 所有の証拠。

本 slice は host 側半分を `ProcessorHost::capability_generation`
として公開し、`RoutingEngine` が prepared bundle ごとに pin
する別軸として prepare/publish/reconcile/equivalence の各 gate
に fold する。`RoutingBuildKey` の構造・`semantic_eq`・
`port_capability_generation` の意味は一切変更しない —
workspace/render 側が key を literal 構築・比較しており、host
状態は project データではないため key に入れられない。

## Detailed Design

### 1. `ProcessorHost::capability_generation`（`host.rs`）

```rust
fn capability_generation(&self) -> u64 {
    0 // default: provisioning 証拠は compiled plan に消費される
}
```

- default `0`: `StaticProcessorHost` のように登録 catalog を
  持たず、connect 時に provisioning 証拠が compiled plan へ
  消費される host は drift を持たない。
- connect による instance 消費・生成は drift と見なさない —
  それは build の staging/compile の一部であり、登録状態の
  変化ではない。
- 非 RT orchestration 経路からのみ呼ばれる（`prepare`/
  `publish`/`reconcile`/equivalence probe）。

### 2. `PluginHostProcessors` の generation（`host.rs`）

登録状態を 2 軸で畳み込む:

- **`registration_epoch: u64`** — `register_boundary` /
  `register_params` の各呼出で saturating add。adapter 所有の
  boundary declaration・param catalog 登録は composite
  `PluginHost` の `Scan` からは見えないため、epoch で覆う。
- **`Scan` descriptor 畳み込み** — `host.borrow_mut().execute(
  HostCommand::Scan)` が返す登録 descriptor 列（id/name/
  vendor/version/features）を FNV-1a で畳み込む。factory の
  追加・削除・descriptor 変更はこの軸で検出される。
  `register_factory` は adapter を経由しない（composition root
  が保持する共有 `Rc<RefCell<PluginHost>>` handle 経由で
  直接登録される）ため epoch では捕捉できず、`Scan` が必須。

`capability_generation()` = `fnv_fold(epoch bytes)` に続けて
各 descriptor の identity/features を fold した値。mutation が
無ければ同一値を返す決定的な change detector であり、
絶対値に意味は持たない（project 側 FNV と同じく drift 検出用）。

### 3. `RoutingEngine` への fold（`routing_engine.rs`）

- **`PreparedRouting.host_capability_generation: u64`** —
  `prepare` が `checked_key` 通過直後・connect 開始前に
  `self.host.capability_generation()` を capture し bundle に
  保持。prepare ごとに独立した pin なので、複数の outstanding
  prepare がそれぞれ「自分が build した時点の」generation を
  運ぶ。
- **publish 再検証** — `routing_fingerprint` 一致の次、
  sourcemap 整合の前に 2 半分を検査:
  `port_capability_generation(project) != key.capability_generation`
  または
  `prepared.host_capability_generation != self.host.capability_generation()`
  → `RoutingFault::PluginCapabilityChanged`（`Blocked` receipt、
  last-good 保持）。fingerprint が一致していても host drift を
  捕捉するため `SpeculativeMismatch` に shadow されない。
- **ledger pin** — publish 成功時に
  `ledger.published_host_capability_generation =
  Some(prepared.host_capability_generation)`。
  `published_generation` と常に対で存在する。
- **equivalence probe** — `live.semantic_eq(key)` に加えて
  `ledger.published_host_capability_generation ==
  Some(self.host.capability_generation())` を要求。host drift
  下では semantic 同一 revision でも `Built` に倒れ、stale な
  capability 証拠の plan に shortcut しない。
- **reconcile** — `port_capability_generation(project) !=
  key.capability_generation` または
  `ledger.published_host_capability_generation !=
  Some(self.host.capability_generation())` で
  `RoutingReconcileMismatch::CapabilityGeneration` を push。
  既存 precedence で `PluginCapabilityChanged` fault・`Blocked`
  receipt に写像され、live plan は継続する。

### 4. key 契約の文書化（`routing_build.rs`/`routing_tx.rs`）

`RoutingBuildKey.capability_generation` と
`port_capability_generation` の doc に「project 側半分のみを
含み、host 側 generation は `RoutingEngine` が別軸で pin する」
ことを明記。構造・値・比較規則は不変。

## Data and Control Flow

```
prepare(request, project, revision, deps)
  └─ checked_key → RoutingBuildKey（project 側のみ）
  └─ host.capability_generation() → PreparedRouting pin
  └─ equivalence(key): semantic_eq && host_current → Equivalent|Built
publish(prepared, project, revision)
  └─ revision/CAS → fingerprint → capability 2半分 → sourcemap → …
  └─ 成功: ledger.published_host_capability_generation = pin
reconcile(project, revision, …)
  └─ CapabilityGeneration mismatch → PluginCapabilityChanged + Blocked
```

## Non-Functional Constraints

- 決定性: generation は登録状態の純粋関数。`Scan` の descriptor
  列順は host の内部 table に従うが、固定状態では安定
  （fixture で連続 2 回の同一性を assert）。
- fail closed: drift は常に `PluginCapabilityChanged` /
  `Blocked` で表面化し、黙って stale plan を publish/継続
  しない。
- 非 RT: `capability_generation` は prepare/publish/reconcile
  の orchestration 経路のみ。RT audio path に触れない。
- `ProcessorHost` は `!Send` 単一 owner を維持 — generation
  は `&self` で `RefCell` 借用のみ。

## Risks and Trade-offs

- **`Scan` が route 登録を含まない場合**: audio descriptor table
  のみ畳み込む。route/factory 以外の登録軸が `Scan` に現れない
  場合は `registration_epoch` 側で adapter 経由の変更を覆う。
  composition root が adapter を介さず host を直接 mutate する
  経路は `host()` handle を意図的に公開している test/observer
  用途に限定され、production 経路では登録は構築時に完了する。
- **generation の衝突（hash 一致で drift 見逃し）**: FNV-1a は
  change detector であり暗号学的保証はない。project 側
  `capability_generation` と同じ既存パターンを踏襲し、
  32/64bit 衝突確率は運用上無視できる水準。
- **host swap の false positive**: `set_processor_host` で
  別 host に差し替えると generation が変わり publish が
  `PluginCapabilityChanged` になる — これは意図した検出
  （provisioning 証拠が変わった）であり、同一内容の host を
  再構築した場合も安全側に倒れる。

## Deferred Design Concerns

- `ActivationNotObserved`: engine epoch/instance は構築時 pin
  （`RoutingEngine::new`）で遷移 API が無く、published plan の
  RT activation を正当に withhold する経路が存在しない。
  fault-injection seam の新設は本 slice では行わず NotRun。
- `RetireOverflow`: Ruviform retire channel の overflow が前提。
  headless 決定的再現には upstream seam が必要 — ruviform 側
  Issue 候補として記録。
- reconcile `DeviceEpoch`: epoch/instance 遷移 API が無いため
  mismatch を構成できない。device epoch 遷移設計は別 slice。
- 実 VST3 での factory unload/scan drift: 外部 gate（実機/GUI
  lane）に委譲。

## Verification Notes

- [x] `cargo test -p synvelis-player --test sf03_l9` — 5/5 pass
      （登録軸 tracking・publish live firing・reconcile
      mismatch・equivalence 敗退・host swap）。
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
      `test-msrv`/`check-app-native`/`lint-app-native`/`build`
      — 結果は IR/PR に実測記録。
