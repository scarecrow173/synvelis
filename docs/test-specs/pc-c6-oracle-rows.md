---
id: "TSPEC-034VmVmcW4lYrji082NUve"
type: "test-spec"
status: "approved"
title: "PC-C6 oracle E2E — R01〜R08 / PL01〜PL08 / UI01〜UI07 / COMP01〜COMP05 独立照合 fixture"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
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
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/designs/pc-c6-oracle-rows-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/designs/pc-c6-oracle-rows-design.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c6-oracle-rows.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
  defers: []
  deferred-by: []
---

# PC-C6 oracle 検証仕様

`crates/synvelis-workspace/tests/pc_c6_oracle_rec.rs`・
`pc_c6_oracle_ui.rs` と `crates/synvelis-player/tests/
pc_c6_oracle_pl.rs`・`pc_c6_oracle_pl_rt.rs` が担保する契約。
全て headless synthetic fixture / fake plugin / `FakeWindowHost` —
実 plugin/OS/RT device/GUI 観測は対象外。期待値は oracle が fixture
宣言から独立に導出し、契約本体の観測と照合する。oracle と実装の
差異が出た場合は、実装を oracle に合わせて変更せず、契約解釈を
spec/design/test-spec に立ち返って確認する。

## Feature: R01〜R08 — Recording Safety 独立照合

- Rule(R01): `record_start` が固定する context — fixed_revision・
  target 集合・target→lane 対応・clock epoch・scope・loop pass・
  seal 状態 — が fixture 宣言値と field 単位で一致する。
- Rule(R02): transaction 期間中の外部 revision 前進は Composite
  CAS/stale として commit を拒否し、記録点列を消費せず lane の
  既存 points/columns を保持する。
- Rule(R03): 構造的失敗（write lane 除去等）は typed rejection
  され、失敗経路でも lane の既存データを破壊しない。
- Rule(R04): commit は applied + was_sealed を含む 1 Undo 境界で
  exactly-once。`record_abort` は点列を適用せず transaction を
  破棄する。
- Rule(R05): 外部変更後の commit は自動再束縛せず明示 rebase を
  要求し、rebase → 再 preview → commit が宣言どおり動作する。
- Rule(R06): loop end 通過は `SealReason::LoopWrap` で seal し
  playback pass を前進・位置を loop 先頭へ戻す。seal 後の write は
  拒否され、sealed transaction の commit は記録点を適用する。
- Rule(R07): `AutomationMode` Read/Touch/Latch/Write と
  `WriteArm` Disarmed/Armed の gating 行列が宣言どおり —
  record_start は record enabled・rolling transport・armed
  write-mode target・bound write lane を全て要求する。
- Rule(R08): transaction の固定 scope は無関係な project op
  （対象外 entity 追加）後も維持される。Note/Clip/Tempo lane
  種別での元データ/RT 不変は headless 観測範囲で照合し、実 RT
  device での不変証明は NotRun。

## Feature: PL01〜PL08 — Plugin Binding/Dispatch 独立照合

- Rule(PL01): attach 時の `frame_base` が verify/pin され、binding
  は plugin identity・`StablePluginInstanceId`・node・capability
  generation・graph generation・param id・clock context を pin
  する。pin 値は fixture 宣言と一致する。
- Rule(PL02): shared parameter・stale record は typed
  `DispatchError` で reject され、delivery log に誤配送が残らない。
- Rule(PL03): plan staleness は orchestration boundary で検査され、
  stale plan 由来の dispatch は fail closed。
- Rule(PL04): dispatch queue は有界で、満杯は
  `DispatchError::QueueFull` として型付き観測される。batch dispatch
  は atomic — 内部 1 件の stale binding で batch 全体が staging
  なしに reject される。
- Rule(PL05): delivery は min process frame offset を遵守する。
  `stats_alloc` 計測（独立 test binary・warm-up/window 分離）で
  warm hot path の allocation が 0 であること。実 plugin/実 RT
  device での計測は NotRun。
- Rule(PL06): capability generation・graph generation の drift と
  plugin reload（新 instance mint）は旧 binding を fail closed
  にする — 旧 binding での dispatch は typed rejection。
- Rule(PL07): wrong plugin instance・node・generation・param id・
  capability・frame base はそれぞれ typed rejection を返し、旧
  Target への誤配送は delivery log 上 0 件。
- Rule(PL08): 実 VST3/CLAP plugin capability・実 plugin GUI・
  実 RT device 性能は本 fixture では未測定 — Pass にせず ledger
  で NotRun として記録する（boundary marker test が headless
  範囲を明示）。

## Feature: UI01〜UI07 — Control Surface 独立照合

- Rule(UI01): relation view（`relation_rows`/`relation_rows_with`）
  が fixture 宣言 edge 集合（AutomationDrives lane→target・
  ModulationDrives source→assignment・assignment→target）と
  coverage/revision/rows 全件で一致し、Project が唯一正本である
  こと（view 側に複製なし）。
- Rule(UI02): `RelationNav` が Parameter→Assignment→Source→
  Parameter の双方向探索で path を保持し、`pop` が直前 anchor を
  復元する。
- Rule(UI03): `expected_revision` pin の旧 revision query は
  `RelationCoverage::Stale` + 空 rows を返し、構築済み view は
  project 変更後 `is_stale` で検出される — 旧 receipt を現行
  真値にしない。
- Rule(UI04): window A で begin した control-edge gesture は view
  の window B 移動後に commit すると `ControlEdgeOutcome::Stale`
  で drop（retarget しない・project/revision 不変）。cancel は
  project・revision・undo 深さを変えない。
- Rule(UI05): Begin/Preview/Commit/Cancel lifecycle — preview は
  in-memory のみで project/revision 不変、commit は assignment
  1 件追加 + `Applied{revision}`、undo 1 回で除去（成功 1 Undo）。
- Rule(UI06): headless surface に probe/trace 状態が漏れないこと
  を marker として確認（実 probe 証拠は TR gate の責務）。
- Rule(UI07): `FakeWindowHost` が headless window/dock 意味論
  （owner 解決）のみを証明することを marker として確認。実
  GPUI/OS focus/IME/plugin GUI は NotRun。

## Feature: COMP01〜COMP05 — Gate 判定記録

- Rule(COMP): 各 COMP Case の headless 実施可能部分のみを証拠
  ledger に Pass/Fail/NotRun/Unsupported で区別記録する。
  Gate A（AI Off 通常 DAW）・Gate B（構成/実観測/推定分離）・
  Gate C（双方向探索）の headless 証拠は本 slice の R/PL/UI/REL/TR
  oracle Pass を構成要素として参照するが、実 GUI/OS/plugin/
  両 Consumer を要する判定は `NotRun` のまま残し、Fake/Headless
  を実機成功に繰り上げない。

## Guarantees

- 全判定は決定論的・read-only（fixture 構築を除く）・非 RT。
  `Project`/session state を oracle が直接変更しない。
- 欠落・失敗・stale・sealed・disabled を成功・Live・観測済みと
  誤認しない。
- headless fixture の Pass を実 VST3/GPUI/OS/実 RT の成功に
  繰り上げない。

## Non-goals

- PC-C2-REL01〜07・PC-C3-TR01〜08（PR #212 で実装済み・回帰のみ）。
- `crates/synvelis-app/` の製品 GUI シナリオ・実 GUI/OS/plugin/
  RT device/両 Consumer 実機照合（#114 gate）。
- TRACE-C3-05 dispatch ledger 結合・Ruviform #219/#294 consumer
  照合（別 gate/dispatch）。

## Risk

- oracle 実装自体の誤り — spec/design の規則を一次情報とし、差異時は
  契約解釈を再確認する。
- workspace `control_with` API の観測面が PC-C1 設計とずれる場合 —
  公開面のみを使い内部に立ち入らない。
