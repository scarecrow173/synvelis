---
id: "ADR-034Xe4SL0Sj3r7V85VOaFO"
type: "adr"
status: "proposed"
title: "CB-00 Creative Branch — VariantSet/Scope/Lineage canonical model・単一 Controller CAS Adopt・Compare 3 層・Owner/DAG の決定 (synvelis#277)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/277"
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
    - "docs/specs/creative-branch.md"
    - "docs/specs/project-model.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/vcs-integration.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/278"
    - "https://github.com/scarecrow173/synvelis/issues/279"
    - "https://github.com/scarecrow173/synvelis/issues/280"
    - "https://github.com/scarecrow173/synvelis/issues/281"
    - "https://github.com/scarecrow173/synvelis/issues/282"
    - "https://github.com/scarecrow173/synvelis/issues/283"
    - "https://github.com/scarecrow173/synvelis/issues/284"
    - "https://github.com/scarecrow173/synvelis/issues/285"
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "https://github.com/scarecrow173/synvelis/issues/287"
    - "https://github.com/scarecrow173/synvelis/issues/288"
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/creative-branch-design.md"
    - "docs/specs/creative-branch.md"
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
    - "docs/adr/vs-d-commit-identity-manifest-single-mutation.md"
    - "docs/designs/vcs-integration-design.md"
    - "docs/designs/project-model-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/creative-branch.md"
    - "https://github.com/scarecrow173/synvelis/issues/277"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/176"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/289"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/562"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers:
      - "scarecrow173"
    consulted: []
    informed: []
---

# CB-00 Creative Branch — VariantSet/Scope/Lineage canonical model・単一 Controller CAS Adopt・Compare 3 層・Owner/DAG の決定 (synvelis#277)

## Context and Problem Statement

Synvelis の制作モデルは単一 Live `Project` + `ProjectController`
CAS + Undo/Save/Reload で構成され、Take/Comp（#12）や VCS
Semantic Diff（#7）が部分的に「別案の保持・比較」を担ってきた。
しかし Issue #277 が 2026-11-11 に合意した Creative Branch
契約は、Audio Clip・Take/Comp・MIDI・Instrument・FX
Processing Range・Track/Bus・Arrangement の **7 Domain 横断**
で「共通 Origin と明示 Scope を持つ独立編集・比較・採用可能な
制作候補」を要求する。物理的 Audio Graph 分岐だけでは
Clip/MIDI/Arrangement の候補を表現できず、Domain ごとの
ad-hoc な候補機構では Lineage・Compare・Adopt の契約が分裂する。

設計合意は Issue 本文で確定済みだが、実装・実 GUI・実 Plugin
受入は未実施であり、本 ADR と spec/design 文書の作成は Gate
PASS を意味しない。決定すべきは「候補を Project versioned
正本のどこに置き、どう編集・比較・採用し、Runtime 反映をどう
分離するか」である。

## Decision Drivers

- **非破壊**: Source/Raw Take/Asset/外部 VCS 状態を破壊しない。
- **単一 Writer**: `ProjectController` 以外に第二の project
  truth を置かない（PM-R-10）。
- **Undo/Save/Reload**: Fork/Discard/Adopt は Versioned Project
  操作として履歴・永続化・migration に整合する。
- **正直な証拠**: Missing Plugin・Stale・Unknown を empty/Equal
  にせず、Semantic/Signal/Perceptual を混同しない。
- **RT 安全**: 非採用候補を常時 RT 駆動せず、Adopt の Project
  CAS と RT Activation を非原子的な別段階にする。
- **独立性**: 通常 DAW Gate A・VCS・AI・Git を必須条件に
  しない。Ruviform へ DAW 語彙を持ち込まない。
- **下流分解可能性**: CB-01 型/Schema を先行正本として
  #279〜#288 が並行開始できる契約粒度。

## Considered Options

* **Option 1(採用)**: Project versioned persistent domain に
  `VariantSet`/`Branch`/`VariantScope`/`ComparisonSession` を
  追加し、`SnapshotIdentity`・typed stable ID・既存
  `ProjectController` CAS/`ProjectOp::Composite`/head CAS・
  versioned migration を再利用する canonical model。
  Adopt は単一 CAS/Composite + 1 Undo、Runtime Publish/
  Activation は別段階・別 Receipt。
* **Option 2**: 候補ごとに VCS branch/checkout を利用する —
  VCS は read-only/optional であり、Creative Branch は VCS Off
  でも動作必須。branch/checkout の Git 操作は VCS Provider への
  暗黙 Write に相当し、scope 付き制作候補の意味論を持たない。
  `vcs-integration` の責務分離契約に反するため却下。
* **Option 3**: Domain ごとに個別の候補機構（Comp 拡張・
  clip duplicate・plugin preset 複製等）を実装する —
  Lineage/Compare/Adopt/Undo の契約が Domain 数だけ分裂し、
  二重正本・無断参照の Oracle を一元化できない。却下。
* **Option 4**: Branch ごとに Project 全体を複製した別
  document/別 project として保持する — Asset/Source 共有・
  lineage・scope fingerprint・単一 Undo/CAS が成立せず、
  resource・RT・Save/Reload のコストが scope に比例しない。
  却下。
* **Option 5**: Semantic Merge/3-way Merge・自動 Blend を
  Adopt に含める — Issue が初期対象外と明示。Merge は別機能
  であり、単一候補の宣言 Scope 反映という Adopt 契約と
  混同すると失敗行列が曖昧になる。初期は採用しない。

## Decision Outcome

Chosen option: "Option 1", because Issue #277 の合意契約
（canonical 概念型・Scope/Adopt 表・状態競合契約・Owner/DAG
規則）を、既存の単一 Writer/CAS/Undo/Snapshot/diff/migration
契約の上に矛盾なく載せられ、CB-01〜CB-11 の並行分解を可能に
する唯一の構成であるため。

### Consequences

* Good, because VariantSet/Branch の真値が Project versioned
  persistent domain に置かれ、Save/Reload・migration・Undo が
  既存経路で成立する。
* Good, because `SnapshotIdentity`・`diff` engine・
  `plugin_state` 品質語彙・`routing_tx` preflight・SF-03 の
  activation/last-good 契約を再利用でき、新規の並行機構を
  増やさない。
* Good, because branch-local ID と Live typed stable ID の分離、
  Adopt 時の決定的 ID mapping、Ambiguous/Unknown Reject により
  無断誤参照・無断変更を Oracle で検出できる。
* Bad, because VariantSet/Branch の schema 版管理・branch-local
  ID 空間・scope fingerprint・domain adapter 群は CB-01〜CB-07
  の新規実装を必要とし、docs-only の本 slice では未検証のまま
  残る。
* Bad, because Compare の Signal/Perceptual 層は render/probe・
  RT・実 Plugin に依存し、headless oracle だけでは十分に
  検証できない（#285/#286/#288 と Ruviform #562 の gap 判定に
  委譲）。

### Confirmation

- `docs/designs/creative-branch-design.md` §4.3 の状態遷移表
  （T-1〜T-10）と §8 の Case ID/Oracle/失敗行列が Issue 受入
  条件 CB-AC-01〜08 を網羅することをレビューで確認する。
- CB-01（#278）以降の各 Issue が本 ADR と design 正本を
  `depends-on`/`derives-from` で参照し、実装時に契約へ
  適合することを対応 Gate で検証する。
- 違反が発見された場合は後続 Issue の fixture/oracle
  （CB-SM/CB-ID/CB-DOM/CB-PROC/CB-CMP/CB-ADP/CB-GATE）で
  Fail として記録し、本 ADR を supersede せず修正 Issue で
  是正する。

## Implementation Plan

* Affected paths: 本 slice は docs-only でありコード変更なし。
  将来の実装影響先は `crates/synvelis-project`（VariantSet/
  Branch/VariantScope/ComparisonSession の型・schema・op・
  migration ladder 追加）と、CB-02〜CB-07 の domain adapter、
  CB-08 の compare orchestration、CB-09/CB-10 の UI/Gateway
  層。正式な crate/型名は CB-01（#278）で確定する。
* Patterns to follow: `ProjectController::apply` の
  `Command{expected_revision}` CAS・validate→scratch apply→
  atomic swap・`ProjectOp::Composite` all-or-nothing・
  `ProjectStore::save_cas` head CAS・`SnapshotResolver` の
  read-only capture・`diff::compare_snapshots` の domain
  adapter 構造・`routing_tx::preflight`→`RoutingPlan` の
  検証→計画→commit 分離・`plugin_state` の
  `StateCaptureQuality`/`PluginStateFailureKind` 語彙・
  `migration` の順序付き registry・SF-03 の generation/
  last-good 契約・synvelis-project の id module
  （`define_id!`）の typed stable ID 形式。
* Patterns to avoid: ProjectController 以外の Writer、
  runtime graph ID/Plan Generation の Project semantic ID への
  昇格、ordinal のみの永続境界指定、自動 Rebase/Merge による
  Stale 解消、非採用候補の常時 RT 駆動、Plugin State の数値
  Parameter 化、Live ID の branch 内無条件再利用、VCS
  Provider への暗黙 Write、旧 WebView/SolidJS を正式 UI 契約
  とすること。
* Dependencies or configuration: `SCHEMA_VERSION`（現行 13）
  の ladder への CB-01 追加。workspace ルートの
  Cargo.toml/Cargo.lock は本 Issue では変更しない。
  Ruviform 側は Generic capability の
  gap が再現された場合のみ upstream Issue（#562 系）で定義し、
  既存 ruviform#308/#309/#294 を再実装しない。

## Verification

* [ ] `docs/specs/creative-branch.md` の CB-R-01〜08 が Issue
  #277 合意契約を網羅し、矛盾がないこと（contract-trace で照合）。
* [ ] `docs/designs/creative-branch-design.md` の型写像・状態
  遷移表・domain adapter 表・Case ID/失敗行列が受入条件
  CB-AC-01〜08 を満たす記述になっていること。
* [ ] 既存コード参照（`controller.rs`/`snapshot/types.rs`/
  `diff/`/`comp.rs`/`midi_comp.rs`/`plugin_state.rs`/
  `plugin_instance.rs`/`routing*.rs`/`migration.rs`/
  `store.rs`）との整合を目視照合済みであること。
* [ ] §9.2/§9.3 の Owner/DAG 監査で既存 Issue との重複・循環
  がないこと（`evidence/sv277/owner-dag-audit.md`）。
* [ ] `mise run fmt-check` exit 0、doc-status/doc-maintenance
  の index/lint で新規文書由来の finding がないこと
  （`evidence/sv277/verification.md`）。
* [ ] 実装・実 GUI・実 Plugin・PCM/RT・OS 受入は本 slice 対象外
  であり、CB-11（#288）以降の独立 Gate で NotRun/Pass を
  評価すること。

## Pros and Cons of the Options

### Option 1（採用）: Project versioned domain の canonical model

* Good, because 既存の単一 Writer/CAS/Undo/Save/migration/
  Snapshot/diff 契約をそのまま再利用でき、契約の分裂を防ぐ。
* Good, because scope 付き候補・lineage・fingerprint・ID mapping
  を一つの authoritative model に集約できる。
* Bad, because schema 版管理と domain adapter の実装コストが
  CB-01〜CB-07 に集中する。

### Option 2: VCS branch/checkout の利用

* Good, because 既存 VCS UI/履歴を再利用できるように見える。
* Bad, because VCS は optional/read-only であり、Creative
  Branch は VCS Off で動作必須。Git 操作は Provider への暗黙
  Write で scope 意味論を持たない。却下。

### Option 3: Domain ごとの個別候補機構

* Good, because 各 Domain の既存モデルに最小変更で載る。
* Bad, because Lineage/Compare/Adopt/Undo が Domain 数だけ
  分裂し、二重正本・無断参照の Oracle を一元化できない。却下。

### Option 4: Project 全体複製

* Good, because 各候補が独立 document として単純。
* Bad, because Asset 共有・lineage・scope fingerprint・単一
  Undo/CAS が成立せず、コストが scope に比例しない。却下。

### Option 5: Semantic Merge/自動 Blend の採用

* Good, because 複数候補の統合を自動化できる。
* Bad, because Issue が初期対象外と明示し、単一候補 Adopt
  契約と失敗行列を曖昧にする。初期は採用しない。

## More Information

- 正本 Issue: <https://github.com/scarecrow173/synvelis/issues/277>
  （2026-11-11 合意済み製品契約・概念型・Scope/Adopt 表・
  状態競合契約・Owner/DAG 規則・RCP/2026-11-11 合同契約）。
- 製品契約 spec: `docs/specs/creative-branch.md`。
- 統合設計: `docs/designs/creative-branch-design.md`。
- 下流実装 Issue: synvelis#278〜#288（CB-01〜CB-11）、
  RCP-05 #294、Ruviform#562（Generic Alternate Evaluation
  Context PoC）。関連既存 Issue: synvelis#10/#12/#17/#7/
  #104/#105/#106/#121/#122/#128/#114/#175/#176/#23、
  ruviform#308/#309/#294。
- 関連 ADR: `docs/adr/sf-03-semantic-lowering-source-map-
  activation-boundary.md`（activation/last-good）、
  `docs/adr/pc-c1-commit-session-cas-orchestration-prepared-
  commit.md`（CAS authority/prepare-finish）、
  `docs/adr/vs-d-commit-identity-manifest-single-mutation.md`
  （commit identity/manifest）、および
  `docs/specs/vs-a-project-recording-foundation-stable-id-
  command-save-reload.md`（session/take stable ID）。
