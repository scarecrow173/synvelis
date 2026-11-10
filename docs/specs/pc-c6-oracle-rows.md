---
id: "SPEC-034VmUDawn7YGP2Vh36vnP"
type: "spec"
status: "approved"
title: "PC-C6 headless oracle — R01〜R08 / PL01〜PL08 / UI01〜UI07 / COMP01〜COMP05 fixture（Recording・Plugin・GUI/RT 横断 Gate）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/6"
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
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/specs/pc-c5-02-03-gesture-stale-inspector.md"
    - "docs/specs/pc-c6-oracle-rel-tr.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
    - "docs/test-specs/pc-c6-oracle-rows.md"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/ruviform/issues/219"
    - "https://github.com/scarecrow173/ruviform/issues/294"
  deferred-by: []
---

# PC-C6 headless oracle — R01〜R08 / PL01〜PL08 / UI01〜UI07 / COMP01〜COMP05 fixture

## What Should Be Built

Issue #103（PC-C6 Control Oracle）の固定 Test Matrix のうち、PR #212
（`f329f48`）で REL01〜07 / TR01〜08 を済ませた残りの headless oracle
行を実装する。oracle は fixture が宣言したモデルから期待値を独立に算出
し、main に merge 済みの契約本体の観測結果と照合する。

- **PC-C1-R01〜R08**: Automation Recording の record_start による
  Revision/Target/Lane/Clock Epoch/Scope/Loop Pass 固定、Composite CAS
  と stale、失敗時の点列非消費、成功 1 Undo、明示 Rebase/Retry、
  LoopWrap seal・pass 前進、Read/Touch/Latch/Write mode gating、
  Note/Clip/Tempo lane での元データ/RT 不変を `synvelis-workspace`
  の headless `ControlSession`/`ProjectSession` 経路で照合する。
- **PC-C4-PL01〜PL08**: Verified FrameBase / PluginInstance /
  ParamId / Capability Generation pin、QueueFull・GraphSwap・Reload、
  min process frame offset・RT noalloc、旧 Target 誤配送 0 を
  `synvelis-player` の `PluginBindingSession`/`ParamDispatch`/
  `GraphRuntime` 経路で照合する。実 Plugin Capability 未測定は
  Pass にしない。
- **PC-C5-UI01〜UI07**: Project/Parameter 唯一正本と Automation/
  Inspector/Signal Flow の同一 Case 照合、Begin/Preview/Commit/Cancel・
  2 Window stale/Focus/Graph Change、Preview 時 Project 不変・成功
  1 Undo を `FakeWindowHost` + headless `WorkspaceManager` で照合
  する。実 GUI とは別の Headless Oracle であることを明記する。
- **PC-C6-COMP01〜COMP05**: AI Off の通常 DAW Gate A、Gate B の
  構成/実観測/推定分離、Gate C 双方向探索、録音/RT/Export/Plugin、
  両 Consumer 精度/性能/CLI 非回帰を候補 SHA ごとに独立判定する
  gate。headless 実施可能な部分のみを本 slice で記録し、実 GUI・
  実 OS・実 plugin・実 consumer を要する判定は `NotRun` のまま残す。

## Why Now

Issue #103 は PC-C1〜C5 の Acceptance 証拠を統合する横断 Gate で
あり、REL/TR 行（PR #212）に続く残りの headless oracle 行が揃わない
と Gate A/B/C の独立判定が成立しない。契約本体（PC-C1 recording
transaction・PC-C4 plugin binding/dispatch・PC-C5 gesture/relation
view）は main に merge 済みで、残るのは「別 Oracle 判定」の fixture
のみ。

## Users and Value

- PC-C6 #103: 固定 Case の Expected/Observed を独立算出して記録する
  oracle 層。既存受入 test の再主張ではなく、fixture 宣言からの
  独立導出で照合する。
- 後続の COMP01〜05 Gate 判定・#114 実 GUI Scenario が参照する
  headless 証拠の一部。

## Scope

### In Scope

- 新規 test ファイルのみ（既存 test・`src/` 配下・manifest は一切
  変更しない）:
  - `crates/synvelis-player/tests/pc_c6_oracle_pl.rs`（PL01〜PL08）
  - `crates/synvelis-player/tests/pc_c6_oracle_pl_rt.rs`（PL の RT
    noalloc 行・`stats_alloc` は process-global allocator を観測する
    ため独立 test binary）
  - `crates/synvelis-workspace/tests/pc_c6_oracle_rec.rs`（R01〜R08）
  - `crates/synvelis-workspace/tests/pc_c6_oracle_ui.rs`（UI01〜UI07）
- `synvelis-workspace` への新規 test 追加は Coordinator 承認済みの
  範囲拡張（Orca message `msg_7802648feb93`）。
- oracle は fixture 宣言（目標集合・lane 結合・mode/arm・plugin
  identity/generation・gesture 端点・relation edge 宣言）から期待値
  を test 側で独立導出し、契約本体の観測と照合する。
- `docs/` 文書チェーン（spec/design/plan/task/test-spec/impl
  record）と生成 index の再生成。

### Out of Scope

- `crates/*/src/`・既存 test・`Cargo.toml`/`Cargo.lock`/`mise.toml`/
  CI 設定の変更。
- `crates/synvelis-app/`（別 Worker が占有。製品 GUI シナリオ対応表
  ・Evidence は本 slice 対象外）。
- `crates/synvelis-recording/`・`crates/synvelis-gateway/`・
  `crates/synvelis-library/` 等の他 crate。
- PC-C2-REL01〜07・PC-C3-TR01〜08 — PR #212 で実装済み（回帰確認のみ）。
- 実 GPUI/実 plugin/実 OS/実 VST3/実 CLAP/両 Consumer 実機照合 —
  **NotRun**（#114 gate）。
- #28 Ruviform 追従・RF#219/#294 consumer 照合 — **NotRun**（別 gate）。

## Requirements

### PC-C6-R1: 独立 Oracle 判定

- 期待値は oracle が fixture 宣言から独立に算出する。既存受入 test
  の期待値をコピー・再主張しない。Recording 側は transaction の
  fixed context（target/lane/revision/epoch/scope/pass/seal）を
  宣言モデルと field 単位で照合し、Plugin 側は binding pin と
  typed rejection・delivery log の 0 誤配送を照合し、UI 側は
  宣言 edge 集合・gesture 結果・undo 深さを照合する。
- Fake/Headless の結果を実 VST3/GPUI/OS/実 RT の成功に繰り上げない。
  未実行 Case・未搭載機能は NotRun/Unsupported として正直に記録
  する。

### PC-C6-R2: R01〜R08 網羅（synvelis-workspace headless）

- R01: record_start が revision/target/lane/clock epoch/scope/
  loop pass を固定すること。
- R02: 外部変更との Composite CAS/stale — 失敗時に点列を消費せず
  lane の既存 points/columns を保持すること。
- R03: 構造的失敗（lane 除去等）で rejection されても既存データが
  保持されること。
- R04: commit は 1 Undo 境界・exactly-once。abort は適用なし。
- R05: 明示 Rebase/Retry — 自動再束縛なし、rebase 後の再 preview
  →commit が動作すること。
- R06: LoopWrap で seal + pass 前進、seal 後の write 拒否、sealed
  transaction の commit は記録点を適用すること。
- R07: Read/Touch/Latch/Write mode gating を oracle 照合。
- R08: 固定 scope が project drift（対象外 op 追加）後も維持される
  こと。Note/Clip/Tempo lane の元データ/RT 不変は headless 範囲で
  照合し、実 RT 不変は NotRun 記録。

### PC-C6-R3: PL01〜PL08 網羅（synvelis-player headless + RT noalloc）

- PL01: attach-time `frame_base` verify/pin と plugin identity・
  instance・node・capability generation・graph generation・param id
  の pin。
- PL02: shared parameter・stale record の typed rejection。
- PL03: plan staleness を orchestration boundary で検査。
- PL04: queue capacity 有界・`DispatchError::QueueFull` の型付き
  観測・batch dispatch の atomicity（1 stale で全体 reject・
  staging なし）。
- PL05: min process frame offset を守った delivery・`stats_alloc`
  による warm hot path の allocation-free 実測（別 test binary）。
  実 plugin/実 RT device での計測は NotRun。
- PL06: capability/graph generation drift・reload で旧 binding が
  fail closed。
- PL07: wrong plugin instance/node/generation/param/capability/
  frame base の typed rejection と旧 Target 誤配送 0（delivery log
  で照合）。
- PL08: headless boundary marker — 実 VST3/CLAP capability 未測定を
  Pass にしないことの明示。

### PC-C6-R4: UI01〜UI07 網羅（synvelis-workspace headless）

- UI01: Project/Parameter 唯一正本 — Inspector relation view が
  宣言 edge 集合と一致し、view 側に正本の複製を持たないこと。
- UI02: RelationNav の双方向探索（Parameter→Assignment→Source→
  戻り）が path を失わないこと。
- UI03: stale index honesty — 旧 revision pin は `Stale` coverage +
  空 rows、view は `is_stale` で検出されること。
- UI04: 2 Window stale — window A で begin した gesture が view の
  window B 移動後に `Stale` で drop し retarget しないこと。
  cancel は undo boundary を残さないこと。
- UI05: Begin/Preview/Commit/Cancel lifecycle — preview は project
  不変、commit は 1 assignment + 1 Undo 境界。
- UI06: probe ownership boundary marker — headless surface に probe
  trace 状態が漏れないこと（実 probe 証拠は TR gate 側）。
- UI07: real GUI boundary marker — `FakeWindowHost` の headless
  window/dock 意味論のみを証明し、GPUI/OS focus/IME/plugin GUI は
  NotRun。

### PC-C6-R5: COMP01〜COMP05 gate 記録

- 固定 Case ごとに Synvelis commit・Cargo.lock Ruviform SHA・
  OS/Backend/Plugin/Fixture・Expected/Observed・Pass/Fail/
  Unsupported/NotRun・Recovery/Perf baseline・CI/PR URL を impl
  record / PR に記録する。
- headless 実施可能な部分（AI Off 通常 DAW 経路の headless 証拠・
  Gate B の構成/観測/推定の headless 分離・Gate C 双方向探索の
  headless relation 証拠）のみを本 slice の証拠として記録し、実
  GUI/OS/plugin/consumer を要する判定は `NotRun` として残す。
  Fake/Headless を実 VST3/GPUI/OS 成功に繰り上げない。

## Acceptance Criteria

- [ ] PC-C1-R01〜R08 が `pc_c6_oracle_rec.rs` で独立導出の期待値と
  照合され全 pass（headless 範囲。実 RT/実機行は NotRun 記録）。
- [ ] PC-C4-PL01〜PL08 が `pc_c6_oracle_pl.rs` +
  `pc_c6_oracle_pl_rt.rs` で照合され全 pass（実 plugin capability
  未測定は NotRun）。
- [ ] PC-C5-UI01〜UI07 が `pc_c6_oracle_ui.rs` で照合され全 pass
  （実 GUI は NotRun）。
- [ ] PC-C2-REL/TR（PR #212）は回帰として既存 test が pass。
- [ ] COMP01〜05 の headless 実施可能部分が証拠 ledger に
  Pass/Fail/NotRun/Unsupported を区別して記録される。
- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` / `mise run test-rt` が全て exit 0。
- [ ] doc chain 完走、docs/impl に impl record、生成 index 再生成済み。
- [ ] 未実行・対象外 Case が NotRun/Deferred として記録され、
  headless fixture を実機成功に繰り上げていない。

## Deferred Design Concerns

- 実 GUI・実 OS・実 plugin（VST3/CLAP）・実 RT device・両 Consumer
  実機照合 — #114 gate。
- `crates/synvelis-app/` の製品 GUI シナリオ対応表・Evidence —
  別 Worker/別 slice。
- TRACE-C3-05 の player dispatch ledger 結合・bounded retention —
  別 dispatch。
- #28 Ruviform 追従・RF#219/#294 consumer 照合 — 別 gate。
- COMP01〜05 の実機判定部分 — 全行統合・実機 gate 後。

## Sources and Relations

- Issue #103（本 spec の直接の source）: 固定 Test Matrix の
  PC-C1-R01〜08・PC-C4-PL01〜08・PC-C5-UI01〜07・PC-C6-COMP01〜05
  行と証拠記録要件。
- Issue #6（Design Owner）: Automation / Modulation の意味論。
- PC-C1 spec（recording transaction）・PC-C4 specs（plugin
  binding/dispatch）・PC-C5 specs（relation inspector/gesture）に
  depends-on — oracle が照合する契約本体。
- `docs/specs/pc-c6-oracle-rel-tr.md` + `docs/adr/
  pc-c6-oracle-rel-tr-headless-fixture.md`: 同一独立 oracle 方式の
  先例（PR #212）。
