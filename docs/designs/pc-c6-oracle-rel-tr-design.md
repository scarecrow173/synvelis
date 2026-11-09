---
id: "DESIGN-mFknHHQIpULZsaBsqLEtPk"
type: "design"
status: "approved"
title: "PC-C6 oracle 設計 — fixture 宣言モデル・独立参照実装・Case 対応表"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/designs/pc-c2-control-relation-index-design.md"
    - "docs/designs/pc-c3-control-trace-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C6 oracle 設計 — fixture 宣言モデル・独立参照実装・Case 対応表

## Context

`synvelis-project` の merge 済み契約本体（`relation.rs` PR #204・
`trace.rs` PR #206）を、Issue #103 の固定 Case PC-C2-REL01〜07 /
PC-C3-TR01〜08 に対応する headless oracle fixture で照合する。
編集範囲は `crates/synvelis-project/tests/` の新規ファイルと `docs/`
のみ。`src/` は oracle が照合する対象であり変更しない。

## Scope

### In Scope

- `crates/synvelis-project/tests/pc_c6_oracle_rel.rs`（新規）:
  REL01〜07。
- `crates/synvelis-project/tests/pc_c6_oracle_tr.rs`（新規）:
  TR01〜08。
- `docs/` チェーン + impl record + 生成 index。

### Out of Scope

- spec の Out of Scope と同一（src/・他 crate・実機照合・
  player ledger・残り PC-C6 行）。

## Components and Boundaries

### REL 側（`pc_c6_oracle_rel.rs`）

```
FixtureModel（test 内・宣言的）
├── entities: BTreeSet<ControlEntityRef>   — 実在 entity の集合
├── edges: Vec<OracleEdge>                 — canonical direction で保持
│     kind / source / target / assignment_id / dangling 判定材料
└── 変異 API: add_target/add_lane/add_source/add_assignment/remove_*

oracle 導出（test 内独立実装）
├── oracle_edges(model) -> Vec<OracleEdge>
│     lane → AutomationDrives(lane→target)
│     assignment → ModulationDrives(source→assignment)
│                  ModulationDrives(assignment→target)
│                  DepthControlledBy(depth_source→assignment)（存在時）
├── oracle_state(edge, model) -> RelationState
│     dangling → Unresolved / disabled assignment 由来 → Disabled /
│     enabled 辺の cycle 参加 → Unresolved / 他 → Enabled
├── oracle_query(model, RelationQuery) -> RelationReply 相当
│     期待 coverage/revision/relations を独立 BFS で算出
└── assert_oracle_agrees(&RelationIndex, &FixtureModel, &RelationQuery)
      — coverage・project_revision・relations を全件比較
```

- FixtureModel は `Project::control` への直接構築と**同じ宣言**を保持
  し、oracle は `Project` を読まず FixtureModel のみから期待値を導出
  する（真の独立性: `RelationIndex::build` の走査結果を参照しない）。
- oracle の cycle 判定は「enabled（非 Disabled）辺グラフで target から
  source へ戻れるか」の到達判定で実装し、SCC ライブラリ的発想を
  使わない独立手順とする。大規模 fixture では cycle を含めないため
  この経路は小規模 fixture のみで使う。
- BFS は spec の決定的順序（構築順 adjacency・FIFO・Both は
  outgoing→incoming）を oracle 側で再実装し、`relations` を順序込み
  で全件比較する。`max_edges` 打ち切り・`max_depth`・visited
  node/edge・disabled 非 traverse も同じ規則で再現する。
- `RelationState::Unsupported` は現行 index が生成しない予約語彙 —
  oracle も生成せず、「全結果に Unsupported が現れない」ことを
  照合の一部として確認する。

### TR 側（`pc_c6_oracle_tr.rs`）

```
fixture helpers（trace_c3.rs と同型の合成値生成）
├── key()/ctx() — ControlTraceKey / FreshnessContext の基準値
├── receipt(status) / stamp(provenance) / trace 段階構築
└── oracle_freshness(&ControlTrace, &FreshnessContext)
      -> ObservationFreshness   — key↔ctx 次元照合→channel 評価→
         best-channel 集約を独立実装
    oracle_channel(...)         — plugin_reported / observed 個別分類
    oracle_coherence(&ControlTrace) -> bool — slot 内 generation と
         key の整合を独立検査（is_coherent の照合）
```

- oracle は `trace.rs` の `context_stale`/`channel_freshness`/
  `stamp_freshness`/`freshness`/`is_coherent` を呼ばず、spec/design
  に記述された分類規則を test 側で再導出する。
- `ScheduledReceipt::new`/`AppliedStageReceipt::new` の fail-closed
  は公開 API の振る舞いとして直接照合（oracle が status 規則を
  独立に持ち、構築可否を比較）。

### Case 対応表

| Case | fixture / 照合内容 |
| --- | --- |
| REL01 | 混合 fixture で Incoming/Outgoing/Both 全方向を oracle 照合 |
| REL02 | LFO/Macro/StepSeq source + Depth Driver 辺の oracle 照合 |
| REL03 | 同一 source→target 複数 assignment + disabled/include_disabled |
| REL04 | enabled cycle→Unresolved・disabled cycle・dangling・Unsupported 非生成 |
| REL05 | max_depth/max_edges・Truncated 決定的 prefix・max_edges=0 境界 |
| REL06 | Stale/Unavailable/Complete空・revision delta・削除反映 |
| REL07 | 数万 edge fixture の build/query `Instant` 実測 + oracle 一致 + 有界性 |
| TR01 | 7 stage 独立 slot・欠落 Unknown 維持・receipt fail-closed |
| TR02 | key↔ctx 7 次元それぞれの不一致 → 対応 StaleReason |
| TR03 | seek/loop/restart/pause 相当の世代差で旧証拠を Stale 化 |
| TR04 | Plugin Lost/Unknown・dispatch 終端・channel 独立性 |
| TR05 | lost/stale/estimated/future stamp の分類 |
| TR06 | Measured Silence vs Unobserved の別状態 |
| TR07 | probe 証拠共有の 2 trace が独立評価・非汚染 |
| TR08 | 固定サイズ投影・反復評価で状態非蓄積（retention 実計測は defer 記録） |

## Data and Control Flow

- fixture 構築: `Project::new` → `control` map 直接挿入（既存 test
  と同じ helper 流儀）→ `FixtureModel` に同一宣言を記録。
- oracle: FixtureModel → 期待 reply/freshness を導出（`Project` 非参照）。
- 照合: `RelationIndex::build(&p, rev)` / `ControlTrace` 構築値に対し
  `index.query(&q)` / `trace.freshness(&ctx)` 等を実行し oracle と比較。
- 計測: `Instant::now()` で build/query を挟み `eprintln!` で
  `{case} fixture: N edges/nodes, build {:?}, query {:?}` を出力
  （impl record に転記）。

## Non-Functional Constraints

- test のみ・`src/` 不変・Cargo 依存追加なし（dev-dependencies は
  既存の `stats_alloc` のみで本 slice では不要）。
- 決定性: fixture は固定値、oracle は純粋関数。`Instant` は計測のみで
  判定に使わない（閾値は保守的 sanity 上限のみ）。
- lint: workspace `[lints]`（clippy `-D warnings`）適合。
  `#[expect(clippy::too_many_lines)]` は既存 test 先例に倣い必要箇所のみ。

## Risks and Trade-offs

- **oracle 自体の誤実装リスク**: 契約を誤読して oracle を書くと
  実装側の正しさを検証できない → spec/design/test-spec の規則を
  一次情報とし、差異が出た場合は契約解釈を再確認する手順を
  test-spec に記す。
- **BFS 順序の再現**: oracle が順序込みで比較するため、順序規則の
  解釈差は fail として顕在化する — これは意図どおり（決定性は契約）。
- **巨大 fixture の test 時間**: 数万 edge でも build/query は
  計測上 1 秒未満〜数秒の見込み。CI を圧迫しない規模に留め、
  実測値を記録して Baseline とする。

## Deferred Design Concerns

- TR08 の bounded retention 実計測・no-RT alloc: TRACE-C3-05
  （synvelis-player slice、別 dispatch）へ defer。本 slice は
  「projection が履歴を持たない固定サイズ値である」ことの確認に
  留める。
- 2 Inspector 実 UI・共有 probe の実配線: #102/#114 側。
- 実機照合: #114 gate。

## Verification Notes

- canonical: `mise run fmt-check` / `check` / `lint` / `test`。
- test-spec `docs/test-specs/pc-c6-e2e-oracle-rel-tr.md` の各 Feature
  が fixture で担保されること。
- REL07 の実測値を impl record に記録（Synvelis commit・Ruviform
  SHA・OS/Backend/Fixture・Expected/Observed・Pass/Fail/NotRun・
  CI/PR URL の証拠欄を含む）。
