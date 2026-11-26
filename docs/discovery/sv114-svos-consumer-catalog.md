---
id: "DISC-SV114SVOSCONSUMER00000001"
type: "discovery"
status: "completed"
title: "SV-OS consumer catalog 調査記録 (synvelis#114)"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/sv114-svos-consumer-catalog.md"
  depends-on:
    - "docs/specs/sv-gui-05-real-os-e2e.md"
    - "docs/test-specs/svt-real-os-e2e.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "evidence/sv114/sv-os-case-definitions.md"
    - "evidence/sv114/acceptance-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-app/tests/common/scenario.rs"
    - "crates/synvelis-app/tests/common/mounted.rs"
    - "crates/synvelis-app/tests/svt_product_e2e.rs"
  defers: []
---

# SV-OS consumer catalog 調査記録

## 調査範囲

synvelis#114 SV-GUI-05 の consumer catalog slice として、frozen
`evidence/sv114/sv-os-case-definitions.md`、既存 headless scenario、pin 済み
ruviform revision `20dc41f71c47674b3e51d1eb0921124b94f8bfaf` の
`ScenarioEntry`/`ScenarioMatrix`/`DriverBinding`/`RealOsGuiTest` を調べた。
ruviform 側は read-only reference であり、本 slice 自身は pin・`Cargo.lock`・ruviform 本体を変更しない。
最終検証前に取り込んだ main には sv#28 の pin 更新（lock rev `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`）が既に含まれるが、
これは upstream main の差分であり本 slice の net diff には `Cargo.lock` を出さない。catalog/real-scenario API の消費可否は
最終 main 取込後の compile/test で再確認する。

## 確認した前提

- frozen case definitions は SV-OS-01〜09 を定義するが、本 slice が executable にするのは
  SV-OS-01〜05 のみである。SV-OS-06〜09 は capability 分類の記録対象で、executable case を
  追加しない。
- 既存 `tests/common/scenario.rs` は driver-neutral semantic helper、製品 component key、
  receipt/projection の wire post、stale generation honesty check を既に持つ。
- `tests/common/mounted.rs` と `tests/svt_product_e2e.rs` は製品 service 込みの
  headless OS-L1 証拠であり、real OS/pixel/native dock 証拠ではない。
- pin 済み ruviform revision には catalog API と `RealOsGuiTest` が両方ある。
  `real-scenario` feature は `ruviform-ui-gpui/real-scenario` として消費できる。
- `tests/common/mod.rs` は lease 外なので、catalog は `scenario.rs` 内の
  `#[path = "sv_os_catalog.rs"] pub mod sv_os_catalog;` で module tree に載せる。

## 設計判断に効いた制約

1. catalog は function registry であり、新しい verdict authority や JSON DSL を作らない。
2. Fake、`GpuiSimulated`、Real OS、Native Plugin、CI evidence を混ぜない。
3. `gpui-test` lane は pixel readback を持たず、pixel capture を claim しない。
4. real lane は interactive desktop preflight に fail-closed し、利用不可なら全件 NotRun とする。
5. `mise.toml` は synvelis#101 active lease のため変更しない。real lane の入口は
   `mise exec -- cargo test -p synvelis-app --features real-os-scenario --test sv_os_real`
   の補助証拠 invocation とする。
6. `uitest` external-control seam と `real-scenario` catalog lane は別 feature・別証拠に保つ。

## 残る制約・ギャップ

- 本 slice の catalog body は semantic dispatch を実 window の `ViewHost` へ適用し、
  さらに独立した mounted `NativeShell`/`SynvelisApp` fixtureで製品 service oracleを検査する。
  それでも physical OS pointer/keyboard input、pixel、IME、native dock、real-window経由の
  service-backed create/open/close 全量を完了したとは主張しない。これらは AC6/SV-OS 全体では
  引き続き NotRun/Deferred である。
- `RealOsGuiTest` は session body 内にだけ存在するため、catalog context は
  session-scoped adapter handle を介する。adapter capability/report は実 adapter の値を使う。
- real lane を interactive Windows で実走できても、それだけでは Issue #114 を close しない。
  service-backed Project State/pixel/IME/DPI/monitor/native dock は後続 gate に残る。
