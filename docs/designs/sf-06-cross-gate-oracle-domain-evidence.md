---
id: "DESIGN-034V2wuzXfqcjIvGyXFhhz"
type: "design"
status: "approved"
title: "SF-06 cross-gate headless 設計 — oracle 独立性・fixture topology・ledger schema"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/adr/sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md"
  derived-by:
    - "docs/plans/sf-06-cross-gate-oracle.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-06-cross-gate-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-06 cross-gate headless 設計 — oracle 独立性・fixture topology・ledger schema

## Context

SF-01（`RoutingModel` persisted schema）・SF-02（routing
transaction + mute/solo audibility policy SF-TX-07/08）・SF-03
（lowering/SourceMap/activation）・SF-04（probe adapter/epoch）・
SF-05（Signal Flow headless view）が main に揃った。SF-06 は
これらを横断する cross-gate であり、headless slice は
`Project`/`RoutingModel`/`ControlModel` の persisted 層で完結
できる検証（model 不変条件・audibility・fault 拒否・migration）
に限定する。runtime engine（#106 dispatch）・probe epoch
（#107）・GUI（#108/#114）の実機 lane は本 slice の対象外。

## Scope

### In Scope

- `crates/synvelis-project/tests/sf06_oracle.rs`:
  独立 oracle 2 系統（audibility・persisted invariant）、
  fixture topology 2 系統（mix・chain）、fault matrix、
  v5→v6 migration oracle。
- `evidence/reports/sf-06-cross-gate-ledger.md`: Case ID
  registry + lane 別結果 ledger。

### Out of Scope

- runtime engine の publish/QueueFull/last-good/RT-Active
  receipt（SF-ACT — #106 実装確定後の別 gate）。
- probe capture/epoch/latency の runtime 計測（SF-PROBE —
  #107・RF#308/#309 の既存 evidence を link のみ）。
- GUI/visual/2-window/Dock（SF-UX — #108 実装・#114 実機 gate）。
- DAW Gate A/B/C・Recording/Offline/Automation・AudioLens・
  lockfile 互換（SF-COMPAT — 各担当 issue の evidence link）。

## Components and Boundaries

- `AudibilityOracle<'p>`（test 内）: `&Project` を保持し、
  persisted `connections`/`ports`/`strips`/`chains`/
  `processors`/`buses` と `ControlModel.targets` の生値だけから
  (1) port→strip 解決、(2) mute/solo 抽出（`base >= 0.5`）、
  (3) enabled edge 集合（muted source 除去）、(4) upstream/
  downstream BFS、(5) documented policy 合成を再実装する。
  `RoutingModel::audible_strips`/`enabled_strip_edges`/
  `strip_muted`/`strip_soloed`/`strip_of_port`/
  `strip_of_processor` は一切呼ばない。
- `routing_invariant_violations(&Project) -> Vec<String>`
  （test 内）: persisted 契約を独立に再検査 — map key==id、
  master 一意、bus↔strip 1:1 owner、strip の chain/port
  back-reference、Main port 1in/1out、processor の単一 chain
  帰属、port owner 解決・sidechain=input のみ、connection の
  output→input 方向・send↔config 厳密 1:1・非 send edge の
  (source,destination) 一意、gain_target 解決、enabled strip
  graph の非循環（独自 DFS）、strip 毎 Mute/Solo 権威の単一性。
- Fixture topology:
  - `build_mix_project`: 4 track strip + group bus strip +
    return bus strip + master = 7 strip。group←bass/vox、
    return←pre/post send、kick→bass processor sidechain、
    各 track→master の auto Main edge を保持。
  - `build_chain_project(disable_middle_main)`: a→b→c chain +
    auto strip→master edge（b の直接 edge を無効化する variant
    で alternate path のみ残す）。
  - 両者とも `engage_bool_targets` で全 strip の mute/solo
    Boolean 権威 target を実体化してから `Project` を clone
    して返す。
- `sweep_audibility`: persisted `ControlModel` を clone し、
  mute/solo bit を 0/1 へ直接 perturb して全 2^(2n) 組合せで
  SUT `audible_strips` と oracle `audible` を比較する。
  n≤8 の幅制限で mask を u64 に収める。
- Fault matrix: `state_snapshot`（live revision + serialized
  project）を before/after で比較し、reject が Live/
  Revision/Undo を不変に保つことを検証。cycle/dup-temp は
  error 文字列も確認。末尾で成功 op + undo が pre-fault
  内容へ正確に戻ること（revision は apply+undo の +2）を確認。
- Migration oracle: 宣言 fixture（track 数・master 一意・
  strip 数 = track+1・Main edge 数 = track・processor/send
  空）から期待投影を導出し、v5 envelope load の結果と比較。
  再 load の決定性（`c2.live() == c3.live()`）と save が新
  snapshot head を発行することも検証。
- Evidence ledger: `evidence/reports/sf-06-cross-gate-ledger.md`
  に Issue #109 の要求 schema
  （`caseId | Synvelis SHA | Ruviform resolved SHA | OS/CPU/FS/
  Device+Plugin | Graph/Project/Clock generations | fixture &
  command | expected vs observed | Pass/Fail/NotRun/
  Unsupported | resource/performance/fault | CI/PR link`）で
  Case ID registry と lane 別結果を記録する。

## Data and Control Flow

- Fixture 構築: `ProjectController` + `CounterIdGenerator` で
  `ProjectOp` を逐次 apply（`AddTrack`→`AddTrackStrip`→
  `AddBus`→`AddConnection`/`AddSend`/`InsertProcessor`→
  `SetConnectionEnabled`→`SetStripMute`/`SetStripSolo`）。
- Oracle 比較: persisted `Project` を clone → `ControlModel`
  の Boolean base を sweep → SUT/oracle の `BTreeSet<StripId>`
  を `assert_eq!`。
- Fault 検証: `RoutingEditIntent`/`ProjectOp` を `apply` →
  `Err` を確認 → `state_snapshot` が before と一致することを
  確認 → 末尾の undo で内容一致・revision +2 を確認。
- Ledger: test 実測値（SHA・OS・command・結果）を markdown
  table に転記。実機 lane は `NotRun` 固定。

## Non-Functional Constraints

- headless 完結: GUI・audio device・plugin host・network に
  依存しない（`synvelis-project` の test target のみ）。
- 決定性: `BTreeMap`/`BTreeSet` ベースで反復順は canonical、
  sweep は mask 昇順で再現可能。
- sweep 規模: 2^14 = 16384 組合せ（mix）+ 2^8 = 256（chain
  ×2 variant）で実測 ~1s — CI 負荷は無視できる。
- oracle 独立性: SUT の private 実装・内部キャッシュに依存
  しない（公開 persisted フィールドのみ）。

## Risks and Trade-offs

- Oracle が SUT と同じ誤読をする危険 → documented policy
  （SF-02 SF-TX-07/08 の spec 文）を唯一の根拠にし、
  `validate_references`/`audible_strips` 等の SUT 関数を
  oracle から呼ばない。corruption fixture で oracle 自体の
  検出能力を検証する。
- Sweep が大きすぎると CI が遅い → strip 数 ≤8 で u64 mask
  に収め、fixture は 7/4 strip に限定。
- headless Pass の誤昇格 → ledger で lane を列分離し、
  実機/consumer 行を `NotRun` で明示。spec/ADR に昇格禁止を
  記録。
- #106/#107/#108 の並行変更で契約が動く → 本 slice は
  persisted 層（#104 契約）のみを対象とし、runtime 契約の
  変更を前提にしない。契約変更時は本 test の再評価が必要。

## Deferred Design Concerns

- SF-ACT 系（publish receipt・QueueFull・last-good・RT-Active）
  の headless fixture — #106 `RoutingControlDispatch` の実装
  確定後に `synvelis-player`/app 層で拡張。trigger: #106 merge。
- SF-PROBE 系の epoch/latency/alignment fixture — #107 probe
  epoch と RF#309 Align の結合点確定後。trigger: #107 merge。
- SF-UX 系の visual-only/Follow/2Window fixture — #108 headless
  hierarchy の API 確定後。trigger: #108 merge。
- SF-COMPAT 系 — 各担当 issue の evidence が揃った時点で link
  のみ追加。trigger: #18 EX 行の再確認・#114/#176/#219/#294 の
  gate 完了。

## Verification Notes

- [x] `cargo test -p synvelis-project --test sf06_oracle`
  exit 0（6 test 全 Pass）。
- [x] Oracle が破損を検出することを `sf_model_valid_oracle_
  detects_corruption` で確認。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`。
