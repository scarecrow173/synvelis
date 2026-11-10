---
id: "DESIGN-034VmVbkRvKggN17EiZnQ9"
type: "design"
status: "approved"
title: "PC-C6 oracle 設計 — Recording/Plugin/UI/RT fixture 宣言モデル・独立 oracle・Case 対応表"
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
  implements:
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
    - "docs/designs/pc-c5-01-relation-inspector-design.md"
    - "docs/designs/pc-c5-02-03-gesture-stale-inspector-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C6 oracle 設計 — Recording/Plugin/UI/RT fixture 宣言モデル・独立 oracle・Case 対応表

## Context

`synvelis-workspace` / `synvelis-player` の merge 済み契約本体を、
Issue #103 の固定 Case PC-C1-R01〜08・PC-C4-PL01〜08・
PC-C5-UI01〜07・PC-C6-COMP01〜05 に対応する headless oracle
fixture で照合する。編集範囲は新規 test ファイルと `docs/` のみ
（`synvelis-workspace` への新規 test 追加は Coordinator 承認済み・
`msg_7802648feb93`）。`src/`・既存 test・manifest は変更しない。
REL/TR 行は PR #212 で完了済みのため回帰確認のみ。

## Scope

### In Scope

- `crates/synvelis-workspace/tests/pc_c6_oracle_rec.rs`（新規）:
  R01〜R08。
- `crates/synvelis-workspace/tests/pc_c6_oracle_ui.rs`（新規）:
  UI01〜UI07。
- `crates/synvelis-player/tests/pc_c6_oracle_pl.rs`（新規）:
  PL01〜PL08。
- `crates/synvelis-player/tests/pc_c6_oracle_pl_rt.rs`（新規）:
  PL05 RT noalloc（`stats_alloc` の process-global allocator 計測の
  ため独立 test binary）。
- `docs/` チェーン + impl record + 生成 index。

### Out of Scope

- spec の Out of Scope と同一（src/・他 crate・app・実機照合・
  REL/TR 再実装・COMP 実機判定）。

## Components and Boundaries

### Recording 側（`pc_c6_oracle_rec.rs`）

```
OracleModel（test 内・宣言的）
├── target/lane/mode/arm/record_enabled の宣言値
├── fixed context 期待値: fixed_revision・fixed_lanes・
│   clock_epoch・scope・pass・seal
└── fixture helpers（pc_c1.rs 流儀: CounterIdGenerator・
    FakeWindowHost・ProjectSession::control_with）

照合
├── tx(&ws): control().recording() の transaction field を
│   OracleModel と field 単位で比較（fixed_revision/lane map/
│   epoch/scope/pass/sealed）
├── commit/abort/rebase/preview の outcome と undo 深さを観測
└── lane の points/columns 保持を project 直接読みで照合
```

- oracle は `RecordingTransaction` の内部を呼ばず、宣言した
  fixture 操作列から期待する fixed context・seal 理由・適用結果を
  導出する。`control_with` 経由の公開 control API のみを駆動する。
- pause は gate を一時閉鎖するが seal しない、stop/seek/loop 変更/
  project 変更/disarm/mode 変更は seal/close — の分類を case 毎に
  宣言し、観測した `SealReason`/write 拒否と照合する。

### Plugin 側（`pc_c6_oracle_pl.rs` / `pc_c6_oracle_pl_rt.rs`）

```
Rig（pc_c4_binding.rs 流儀の fixture stack）
├── FakeHostedPlugin + attach-time AttachClockContext（frame_base pin）
├── graph/node 構築 + PluginBindingSession + ParamDispatch
└── delivery log（誤配送 0 の証拠）

照合
├── binding pin: plugin identity/instance/node/capability gen/
│   graph gen/param id/frame_base を宣言値と照合
├── typed rejection: DispatchError variant を case 毎に期待
├── atomicity: batch 内 1 stale で全体 reject・staging なし
├── QueueFull: capacity 超過が型付きで観測されること
└── rt binary: warm-up 後 hot path（transport/plan schedule/
    partition/delivery/loop-discontinuity）の stats_alloc = 0
```

- `stats_alloc` は process-global allocator を観測するため
  `pc_c6_oracle_pl_rt.rs` を独立 test binary とし、setup/warm-up
  は計測 window 外に置く（`tim02_midi_rt.rs` 先例）。
- 実 VST3/CLAP plugin・実 RT device の計測は本 fixture の対象外
  （PL08 boundary marker + ledger NotRun）。

### UI 側（`pc_c6_oracle_ui.rs`）

```
OracleFixture（宣言的）
├── ProjectOp 列で target/lane/source/assignment を構築
├── declared_edges(): fixture op から導出した canonical edge 集合
│   （AutomationDrives lane→target・ModulationDrives source→
│   assignment・ModulationDrives assignment→target）
└── 照合対象: relation_rows/relation_rows_with の RelationView
    （coverage・project_revision・rows 全件）、RelationNav stack、
    control_edge_begin/preview/commit/cancel の ControlEdgeOutcome、
    session undo 深さ
```

- oracle は `RelationIndex`/`ControlsUi`/view internals を呼ばず、
  宣言 edge 集合・gesture 規則・undo 規則から期待値を導出する。
- `RelationViewOptions`（max_depth/max_edges/include_disabled/
  expected_revision）は case 毎に明示し、default depth=1 の
  有界性を oracle 側でも同じ規則で再現する。
- `FakeWindowHost` は headless のみ — 実 GPUI window/OS focus/IME/
  plugin GUI は UI07 boundary marker + ledger NotRun。

### Case 対応表

| Case | fixture / 照合内容 |
| --- | --- |
| R01 | record_start の fixed context 全 field を宣言値と照合 |
| R02 | 外部 revision 前進で commit CAS 失敗・点列非消費・lane 保持 |
| R03 | lane 除去等の構造的失敗で rejection・既存データ保持 |
| R04 | commit 1 Undo exactly-once・abort 無適用 |
| R05 | 明示 rebase → 再 preview → commit（自動再束縛なし） |
| R06 | LoopWrap seal・pass 前進・seal 後 write 拒否・sealed commit |
| R07 | Read/Touch/Latch/Write gating 行列 |
| R08 | 固定 scope の project drift 耐性・Note/Clip/Tempo lane |
| PL01 | frame_base/identity/instance/node/gen/param pin |
| PL02 | shared param・stale record の typed rejection |
| PL03 | plan staleness boundary 検査 |
| PL04 | QueueFull 型付き観測・batch atomic reject |
| PL05 | min process frame offset・stats_alloc=0（別 binary） |
| PL06 | capability/graph gen drift・reload fail closed |
| PL07 | wrong target 系 typed rejection・誤配送 0 |
| PL08 | headless boundary marker（実 capability NotRun） |
| UI01 | 宣言 edge 集合 = relation view rows（唯一正本） |
| UI02 | RelationNav 双方向 round-trip |
| UI03 | pinned stale revision → Stale + 空 rows |
| UI04 | 2 window 移動で gesture Stale drop・cancel 無 undo |
| UI05 | Begin/Preview/Commit/Cancel + 成功 1 Undo |
| UI06 | probe ownership boundary marker |
| UI07 | headless window host boundary marker |
| COMP01〜05 | headless 実施可能部分の証拠 ledger 記録・実機行 NotRun |

## Data and Control Flow

- fixture 構築: workspace 側は `Project::new` → `ProjectSession` →
  `control_with` 駆動、player 側は `pc_c4_binding.rs` の Rig 流儀。
- oracle: 宣言モデル → 期待値導出（`Project`/binding/dispatch/view
  internals は読まない — 観測対象としてのみ触れる）。
- 照合: transaction field・`DispatchError` variant・delivery log・
  `RelationView`/`ControlEdgeOutcome`/undo 深さを比較。
- 計測: `stats_alloc` のみ（PL05）。`Instant` 閾値判定は行わない。

## Non-Functional Constraints

- test のみ・`src/` 不変・Cargo 依存追加なし（`stats_alloc` は
  既存 dev-dependency）。
- 決定性: fixture は固定値、oracle は純粋導出。RT 計測は alloc
  count のみで timing 閾値は設けない（baseline 未確定）。
- lint: workspace `[lints]`（clippy `-D warnings`）適合。
- workspace test 追加は承認済み範囲（`msg_7802648feb93`）に限定。

## Risks and Trade-offs

- **oracle 自体の誤実装リスク**: 契約を誤読して oracle を書くと
  実装側の正しさを検証できない → spec/design/test-spec の規則を
  一次情報とし、差異時は契約解釈を再確認する。
- **workspace API の version skew**: `control_with`/`ControlSession`
  の公開面が PC-C1 設計とずれる場合は観測面を最小に留め、内部に
  立ち入らない。
- **`stats_alloc` の process-global 性**: 他 test との干渉を避ける
  ため独立 binary + warm-up/window 分離（tim02 先例どおり）。
- **UI gesture API の epoch 意味論**: window 移動が gesture を
  `Stale` にする規則は workspace 側契約 — 観測が契約と違う場合は
  実装を oracle に合わせず契約解釈を再確認する。

## Deferred Design Concerns

- 実 GUI/OS/plugin/RT device/consumer 照合: #114 gate。
- `synvelis-app` 側の GUI scenario 対応表: 別 Worker。
- TRACE-C3-05 dispatch ledger 結合: 別 dispatch。
- COMP 実機判定: 全行統合後の gate。

## Verification Notes

- canonical: `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv`。
- test-spec `docs/test-specs/pc-c6-oracle-rows.md` の各 Feature が
  fixture で担保されること。
- PL05 の alloc 計測値・COMP の Pass/NotRun 分類を impl record に
  記録（Synvelis commit・Ruviform SHA・OS/Backend/Fixture・
  Expected/Observed・Pass/Fail/NotRun・CI/PR URL）。
