---
id: "ADR-NUgXoDaahX19PI6VorpZTM"
type: "adr"
status: "accepted"
title: "RCP-00 — Render Recipe・依存閉包・有効性・Branch/Execution/Policy 統合契約の採用 (synvelis#289)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/289"
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
    - "docs/specs/render-checkpoint.md"
    - "docs/specs/media-render.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/project-model.md"
    - "docs/specs/execution-plan.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/l2-render-slice-boundaries.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/render-checkpoint.md"
    - "https://github.com/scarecrow173/synvelis/issues/289"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv289/render-checkpoint-contract-trace.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/290"
    - "https://github.com/scarecrow173/synvelis/issues/291"
    - "https://github.com/scarecrow173/synvelis/issues/292"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/295"
    - "https://github.com/scarecrow173/synvelis/issues/296"
    - "https://github.com/scarecrow173/synvelis/issues/297"
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# RCP-00 — Render Recipe・依存閉包・有効性・Branch/Execution/Policy 統合契約の採用 (synvelis#289)

## Status

Accepted（2026-11-11 合意を記録。review-protocol で blocker
なしを確認した docs slice として確定）。

## Context and Problem Statement

Synvelis には Creative Branch/Adopt、Plugin Missing、typed
Port/Edge Graph、Render Checkpoint、Render Policy、VCS 共有
という横断的関心が、別々の契約・実装として分散して存在
した。これらを統合する単一の整合契約がないまま #290〜
#297 の子 Issue が並行着手すると、recipe schema・依存
閉包・validity 判定・branch 参照・policy trigger の解釈が
Issue ごとにずれ、false-valid（古い/未検証/stale な
render を "Exact" として提示する）や重複実装（Ruviform
が所有する generic compiler/host/render/media の
Synvelis 側再実装）のリスクがあった。

Issue #289 は 2026-11-11 の合意に基づく **実装前仕様正本**
を要求する。Issue 作成・本 ADR の確定は実装完了・実
Plugin/RT/OS Gate PASS を意味しない。

## Decision Drivers

- Creative Branch/Adopt・Plugin Missing・Typed Graph・
  Render Checkpoint・Render Policy・VCS 共有を単一の
  整合契約へ統合する必要がある（Issue #289 本文）。
- 既存の ProjectController single writer・Ruviform generic
  Compiler/Plugin/Offline Render・AudioLens 独立 consumer
  品質を保護する必要がある。
- RecipeKey（処理要求）・ContentHash（byte 同一性）・
  Eligibility（再利用可否）の混同を防ぐ必要がある。
- #290〜#296 が既存契約＋Fake Adapter で並行着手可能と
  いう依存方針を DAG として固定する必要がある。
- false-valid を 0 にする fail-closed 判定（Unknown/stale/
  unverified を valid 扱いしない）が必要。
- `crates/synvelis-render/src/spec.rs` の既存
  `PluginStateRecipe`（plugin state 品質 evidence+gate）
  と新規 `RenderRecipe` の同名衝突を解消する必要がある。

## Considered Options

- **Option A（採用）**: Canonical `RenderRecipe` v1 schema
  + typed dependency closure + validity/eligibility 状態
  機械 + Branch/Adopt/late-attach 不変条件 + Render Policy
  + Execution Intent/subgraph substitution を単一 spec
  （`docs/specs/render-checkpoint.md`）で正本化し、
  design（`rcp-render-recipe-design.md`）と本 ADR で
  記録。Ruviform generic 層は再利用し、不足は #562 PoC
  で確認後に別 Issue で追加。
- **Option B**: 各子 Issue（#290〜#296）が個別に recipe/
  closure/policy を定義し、後で統合。→ Issue ごとの解釈
  ずれ・重複実装・false-valid リスクが高く棄却。
- **Option C**: Synvelis 独自の DSP Compiler/Plugin Host/
  Media Writer を新設して render checkpoint を内包。
  → Ruviform の既存 generic capability と重複し、
  Owner 境界（POLICIES.md §1/§6、media-render-design
  Owner 表、L2 ADR）に反するため棄却。
- **Option D**: 既存 `PluginStateRecipe` 型を RenderRecipe
  として拡張・流用。→ 別概念（plugin state 品質
  evidence+gate）であり、recipe の canonical schema・
  dependency closure・temporal invalidation を表現
  できないため棄却。命名も分離。

## Decision Outcome

Chosen option: "**Option A**", because 2026-11-11 合意が
要求する単一整合契約を、既存の ProjectController
single writer・Ruviform generic 層・AudioLens 独立性を
保護しつつ、false-valid 防止と並行着手可能な DAG を
同時に満たす唯一の構成であるため。

### Consequences

* Good, because #290〜#296 が既存契約＋Fake Adapter で
  並行着手でき、Issue ごとの schema 解釈ずれを防げる。
* Good, because RecipeKey/ContentHash/Eligibility の分離に
  より「同じ recipe だから同じ音」「hash が同じだから
  再利用可」の誤判定を構造的に防げる。
* Good, because Ruviform generic 層を再利用し、DAW 固有
  語彙を Ruviform に持ち込まず、AudioLens の独立性を
  保てる。
* Good, because fail-closed（Unknown/stale/unverified を
  valid 扱いしない）により missing/stale render を "Exact"
  として提示する事故を防げる。
* Bad, because canonical recipe schema・dependency
  closure・validity 状態機械・policy trigger の全てを
  単一正本で固定するため、初期の spec が大きくなり、
  子 Issue 間の境界調整コストが残る。
* Bad, because Ruviform generic capability（typed graph
  port/node API・stateful offline render・verified media）
  の不足が #562 PoC で確認された場合、Synvelis 側だけで
  は解決できず Ruviform Issue との調整が必要になる。

### Confirmation

- `docs/specs/render-checkpoint.md` が Issue #289 §0〜§9
  の全要件を codify し、RCP-SPEC‑01〜08 へ写像されている
  こと。
- `docs/designs/rcp-render-recipe-design.md` が layered
  architecture・persistent/derived 分離・semantic graph/
  execution plan 分離・resolver/dependency index・
  artifact manifest/CAS・validity/eligibility・policy/job
  receipt・branch/VCS/late-attach・plugin-missing/last-good
  を説明し、棄却した代替案（特に Ruviform generic 層の
  重複実装）を記録していること。
- `evidence/sv289/render-checkpoint-contract-trace.md` が
  Issue 各節 ↔ spec セクション ↔ 既存契約/コードの対応を
  記録していること。
- docs index が `doc-maintenance` で再生成され、lint/audit
  が範囲内で finding なしであること。
- 実装・fixture 実走・実機 gate は子 Issue 側であり、本
  ADR/spec/design の確定をもって Pass とみなさないこと。

## Implementation Plan

これは docs/ 設計契約 slice であり、コード実装は #290〜
#297 が担う。エージェントが後続作業で参照すべき影響範囲:

* Affected paths（本 slice で作成）:
  - `docs/specs/render-checkpoint.md`（canonical spec）
  - `docs/designs/rcp-render-recipe-design.md`（design）
  - `docs/adr/rcp-00-render-recipe.md`（本 ADR）
  - `evidence/sv289/render-checkpoint-contract-trace.md`
  - `evidence/sv289/render-checkpoint-verification.md`
  - `docs/specs/README.md`・`docs/designs/README.md`・
    `docs/adr/README.md`（doc-maintenance で再生成）
* Affected paths（後続実装が触る予定の領域、現時点では
  read-only 参照）:
  - `crates/synvelis-render/src/{spec,job,preflight,
    prepare,verify,snapshot,lib}.rs`（既存 offline render/
    verified publish。`PluginStateRecipe` は別概念）
  - `crates/synvelis-library/src/{digest,cache,registry,
    local,git/,webdav/}`（verified blob/digest・catalog
    cache・provider registry）
  - `crates/synvelis-project`（ProjectController CAS/Undo）
* Patterns to follow:
  - spec §Non-negotiable Invariants の 12 項を全ての
    render checkpoint 実装で維持する。
  - fail-closed: `Unknown`/stale/unverified/missing を
    valid/Exact 扱いしない。index/plan missing は再評価。
  - receipt 分離: render job receipt と save/vcs receipt
    を混同しない。plan generation receipt
    （Configured/Prepared/Published/Active/Observed）と
    render job state を混同しない。
  - ProjectController single writer: canonical 変更は
    CAS+Undo のみ。派生物は project を mutate しない。
  - Ruviform 境界: generic compiler/host/render/media は
    Ruviform。Synvelis は product policy/recipe/closure/
    validity/branch/VCS/GUI を所有。
* Patterns to avoid:
  - Synvelis 独自 DSP Compiler/Plugin Host/Media Writer/
    RT Runtime/Git Provider/Raw Recording Writer の新設。
  - RecipeKey/ContentHash/Eligibility の混同。
  - `PluginStateRecipe`（spec.rs）を `RenderRecipe` と
    して流用・改名・包含。
  - preview/compare/audition の結果を Main/Recording
    Writer に反映、または exact と表示。
  - OnCommit で VCS Write 未実装を成功扱い。
  - "latest wins" の暗黙 artifact 選択。
  - RT thread での FS/allocation/blocking/graph search。
  - AudioLens への DAW render policy のそのまま適用。
* Dependencies or configuration:
  - なし（docs slice）。後続実装は `Cargo.toml`/workspace
    構成と `mise.toml` タスクに従う。

## Verification

* [x] `docs/specs/render-checkpoint.md` が Issue #289
  §0〜§9 を codify し RCP-SPEC‑01〜08 へ写像（本 slice で
  作成・contract-trace で確認）
* [x] `docs/designs/rcp-render-recipe-design.md` が layered
  architecture・分離・resolver/index・manifest/CAS・
  validity・policy/receipt・branch/VCS/late-attach・
  plugin-missing/last-good と棄却案を記録（本 slice）
* [x] `evidence/sv289/` に contract-trace と検証記録
  （実行コマンド・結果・NotRun/Unsupported 区別）
* [x] docs index を `doc-maintenance` で再生成し lint/audit
  が範囲内 finding なし
* [x] `mise run fmt-check` 実行（docs slice の軽量検証）
* [ ] 実装 fixture・test-spec・実機 gate: **NotRun**
  （#290〜#297 の子 Issue が担う。本 ADR の確定を Pass
  とみなさない）
* [ ] 実 Plugin/RT/OS/PCM/GPUI/AudioLens 受入: **NotRun**
  （#296 RCP-GATE・別 lane）

## Pros and Cons of the Options

### Option A（採用）: 単一 canonical spec + design + ADR

* Good, because Issue #289 の全要件を一つの正本に集約し、
  子 Issue の解釈ずれを防げる。
* Good, because 既存契約（media-render/project-persistence/
  execution-plan/project-model/vcs/tim/sf-03/playback-
  graph）との関係を `depends-on`/`refines`/`related` で
  明示できる。
* Good, because fail-closed・receipt 分離・single writer・
  Ruviform 境界を不変条件として固定できる。
* Bad, because spec が大きく、子 Issue への分解時に境界
  の再確認が必要。

### Option B: 子 Issue 個別定義→後で統合

* Good, because 各 Issue が必要最小限の契約だけ書ける。
* Bad, because recipe/closure/validity/policy/branch の
  解釈が Issue ごとにずれ、統合時に矛盾が発生する。
* Bad, because false-valid・重複実装のリスクが高い。

### Option C: Synvelis 独自 compiler/host/media writer

* Good, because render checkpoint を Synvelis 内で完結
  できるように見える。
* Bad, because Ruviform の generic capability と重複し、
  Owner 境界・保守・精度・RT に反する。
* Bad, because AudioLens 等他 consumer への還元ができない。

### Option D: 既存 `PluginStateRecipe` 流用

* Good, because 既存型を再利用できるように見える。
* Bad, because plugin state 品質 evidence+gate という
  別概念であり、recipe canonical schema・dependency
  closure・temporal invalidation を表現できない。
* Bad, because 同名衝突で混乱を生む。

## More Information

- Issue: https://github.com/scarecrow173/synvelis/issues/289
- Canonical spec: `docs/specs/render-checkpoint.md`
- Design: `docs/designs/rcp-render-recipe-design.md`
- Contract trace: `evidence/sv289/
  render-checkpoint-contract-trace.md`
- Verification: `evidence/sv289/
  render-checkpoint-verification.md`
- 関連 ADR: `docs/adr/l2-render-slice-boundaries.md`
  （sequential offline render・staged publish・独立
  readback の既存境界）
- 関連 spec: `media-render.md`（MR-RCP-01）・
  `project-persistence.md`（PROJ-RCP-01）・
  `execution-plan.md`（RCP-00〜07/F01 ノード）・
  `project-model.md`（PM-R-08）・`vcs-01`/`vcs-integration`・
  `tim-04`・`tim03`・`playback-graph`・`sf-03`・
  `command-gateway`・`musical-time`
- 子 Issue: #290(RCP-01)・#291(RCP-02)・#292(RCP-03)・
  #293(RCP-04)・#294(RCP-05)・#295(RCP-06)・#296(RCP-07)・
  #297(RCP-F01)
- Ruviform 関連: #562（generic alternate-execution gap
  PoC）・#196（plugin state/bus/host）・#198（stateful
  offline render）・#294（independent consumer gate）
- AudioLens 関連: #127（non-regression follow-up）
- Revisit conditions: Ruviform #562 PoC で generic typed
  graph/port API・stateful offline render・verified media
  の不足が確認された場合、または #296 acceptance で
  recipe/closure/validity の実機 gate が Fail/NotRun の
  場合に本 ADR と spec を見直す。
