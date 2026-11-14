---
id: "IMPL-034Xf6XTMuIwdmzniCRakm"
type: "impl"
status: "completed"
title: "RCP-04 Missing Plugin Subgraph Resolver・Execution Intent・Safe Plan Reconciliation headless slice (synvelis#293)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/293"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-player/src/rcp/"
      - type: "file"
        path: "crates/synvelis-player/tests/rcp_04.rs"
      - type: "file"
        path: "docs/test-specs/rcp-04-missing-plugin-resolver.md"
      - type: "file"
        path: "docs/impl/exp/rcp-04-missing-plugin-resolver.md"
      - type: "file"
        path: "evidence/sv293/rcp-04-fault-matrix.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
  implemented-by: []
  depends-on:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/rcp-00-render-recipe.md"
    - "docs/impl/exp/rcp-04-missing-plugin-resolver.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcp-render-recipe-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  references:
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
    - "docs/impl/exp/rcp-04-missing-plugin-resolver.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/rcp-04-missing-plugin-resolver.md"
    rejected: []
---

# RCP-04 Missing Plugin Subgraph Resolver・Execution Intent・Safe Plan Reconciliation headless slice (synvelis#293)

## Summary

`crates/synvelis-player` に `rcp` module を新設し、pinned
`ResolvedGraphSnapshot` + `ExecutionIntent` +
`PluginCapabilitySnapshot` + `RenderCandidateIndex` を入力に、
plugin 検証 → typed cutset 検証 → deterministic 選択 → plan
reconciliation の headless 決定契約を実装した。
`SelectionVerdict`（Exact / NonEquivalent / Blocked）と
`ReconciliationReceipt` を返し、`RoutingBuildKey` /
`RoutingFault` / `RoutingActivationState` 等の既存
`synvelis-project` 契約を再利用して stale publish 拒否・
last-good 保持・delivery gating を行う。26 件の deterministic
oracle test（`tests/rcp_04.rs`）が RCP-EX-01〜05 の
headless 検証可能分をカバーする。

## Context

- Issue: https://github.com/scarecrow173/synvelis/issues/293
  （RCP-04。RCP-00 canonical spec `docs/specs/
  render-checkpoint.md`・design `docs/designs/
  rcp-render-recipe-design.md`・ADR `docs/adr/
  rcp-00-render-recipe.md` に対する実装 slice）
- 検証意図: `docs/test-specs/rcp-04-missing-plugin-resolver.md`
- 実験記録: `docs/impl/exp/rcp-04-missing-plugin-resolver.md`
- 証拠台帳: `evidence/sv293/rcp-04-fault-matrix.md`
- lease: `crates/synvelis-player/` のみ。`synvelis-workspace`
  semantic→selection integration は別 Worker lease のため
  deferred residual（evidence に記録）。

## Implementation

### Changes

- `crates/synvelis-player/src/rcp/model.rs` — pinned snapshot
  （project/revision・structure digest・capability/engine/
  source-format generation・clock epoch・runtime instance・
  nodes/ports/edges/boundary requirements）、plugin capability
  snapshot、render candidate index、artifact output 等の
  immutable 入力 model。graph digest/fingerprint の決定的計算。
- `crates/synvelis-player/src/rcp/verify.rs` — plugin identity
  （version/binary digest drift）・availability（Missing/
  Denied）・state restore・advertised port layout・host
  capability を順序付きで検証し typed `PluginVerdict` を返す。
  `Live` 以外は実行不可。未知 state を live 扱いしない。
- `crates/synvelis-player/src/rcp/cutset.rs` — substitution
  region の境界検証: sink/unknown node、全 output boundary の
  artifact output 被覆、domain/format/channel/latency(PDC)
  整合、control 誤配送、boundary input の pinned 要件、
  sidechain 区別、required live input の feed 維持、optional
  `min==0` 許容、dangling/double-feed/multi-output 網羅、
  typed MIDI/Event output 要件。`CutsetRejection` は機械可読。
- `crates/synvelis-player/src/rcp/intent.rs` — `ExecutionIntent`
  （MainPlayback/EditingPreview/BranchAudition/OfflineRender/
  Export）の等価性境界・required live region・preview opt-in・
  artifact class 適格性 policy。
- `crates/synvelis-player/src/rcp/select.rs` — `ExecutionSelector`:
  verify → eligibility gate → deterministic greedy set-cover
  （correctness gate 通過後に covered 数→region サイズ→
  artifact id で安定化）→ skip/unresolved/degraded 分類 →
  required output feed 検査 → `ExecutionSelection` +
  `ReconciliationReceipt`（attempts・substitutions・skipped・
  unresolved・verdicts・digest）。
- `crates/synvelis-player/src/rcp/reconcile.rs` —
  `PlanReconciler`: edit 分類（NoRebuild/ControlOnly/Rebuild）、
  単調 generation の plan prepare、publish 前の
  project/branch revision・capability generation・graph
  fingerprint・engine/source generation・clock epoch・runtime
  instance 再検証、stale/superseded 拒否、Published と RT 観測
  Active の分離、QueueFull/Retire/restore failure/epoch drift
  での last-good 保持と current mismatch 報告、旧 generation
  への param/automation delivery gating。
- `crates/synvelis-player/src/rcp/mod.rs` — public API 再 export。
- `crates/synvelis-player/src/lib.rs` — `pub mod rcp;` 追加。
- `crates/synvelis-player/tests/rcp_04.rs` — 26 件の
  deterministic oracle test（RCP-EX-01〜05）。fixture は node
  内で一意の port id（input `0,1,…` / output `10,11,…`）を使用。

### Decisions During Implementation

- 既存 activation 正本の再利用: plan lifecycle は新機構を作らず
  `synvelis-project` の `RoutingBuildKey`/`RoutingFault`/
  `RoutingActivationState`/`RoutingReconcileMismatch` に揃えた
  （#106 系契約との二重正本を回避）。
- Preview semantics: 明示 opt-in の preview intent は未知/欠落
  plugin を `NonEquivalent` + skipped node として許容するが、
  `skip_keeps_outputs_fed` と required live region 保護で
  fail-closed を維持。MainPlayback/OfflineRender/Export では
  skip/degrade を一切適用しない（`docs/impl/exp/` 参照）。
- Cutset 会計: boundary output は domain/format/latency 全検査
  通過時のみ covered に記録し、reject 済み出力による偽の
  feed 会計を防止。optional 入力（`min==0`）は dangling 強制
  対象外だが required output boundary の被覆要件は維持。
- Layout 比較: Ruviform `Direction`/`SignalDomain` が
  `#[non_exhaustive]`+`Ord` 非実装のため、ordinal+name 正規化
  `LayoutKey` で決定的比較（wildcard arm で将来 variant に
  対応）。
- Render invalidation と plan rebuild の分離: edit 分類は
  fingerprint/capability/epoch 等の evidence 優先で決定し、
  render artifact の stale 判定とは独立（test-spec Rule・
  `ex04_*` で固定）。
- pin が証明できない変更は fail-closed: `EditKind::SourceContent`
  は snapshot pin が media bytes を含まないため `Rebuild` 固定。
  `observe_active` は generation と clock epoch の両方一致を
  要求し、stale-epoch の RT 観測を `Active` にしない。
  `reconcile` は capability snapshot generation と snapshot pin
  の一致も検査する（review-protocol adversarial pass の指摘を
  反映。詳細は `evidence/sv293/rcp-04-fault-matrix.md`）。

## Related Experiments

### Adopted

- `docs/impl/exp/rcp-04-missing-plugin-resolver.md` — cutset
  covered 会計・optional 入力 dangling・preview Unknown state・
  layout 比較の 4 試行。いずれも oracle test で裏付けて採用。

### Rejected

- preview + Unknown plugin state を `Blocked` に倒す案:
  明示 opt-in の編集プレビュー用途を過度に制限するため棄却
  （NonEquivalent+skip の fail-closed 設計を採用）。

## Validation

`evidence/sv293/rcp-04-fault-matrix.md` に全件記録。要約:

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass (exit 0) |
| `mise run check synvelis-player` | Pass (exit 0) |
| `mise run lint synvelis-player` | Pass (exit 0・warning 0) |
| `mise run test synvelis-player` | Pass — `rcp_04.rs` 26/26 + 既存 suite 全 pass |
| `mise run check synvelis-render` | Pass（下流 build integrity） |
| `mise run check synvelis-app` | Pass（下流 build integrity） |

OS: Windows (win32)、toolchain: mise 管理 Rust 1.99、
Ruviform `fd1d4d5f`（変更なし）。補助証拠として直接
`cargo clippy`/`cargo test` も同結果を確認（canonical は
mise タスク）。

## Risks

- cutset 規則は spec の境界条件を網羅的に oracle 化したが、
  実 plugin の多様な port 構成（変則 multiplicity・exotic
  domain）での網羅は実機 Gate（#296）依存。
- `ExecutionSelection` → workspace plan 構築の配線は deferred
  のため、現状は decision contract のみで end-to-end の plan
  activation には未接続。
- `RenderCandidateIndex` の eligibility 分類は #290-#292 の
  正本実装を仮定した入力 contract であり、供給側との整合は
  統合時に再検証が必要。

## Follow-ups

- `crates/synvelis-workspace` semantic→selection integration
  （別 Worker lease・deferred residual。evidence/sv293/
  rcp-04-fault-matrix.md に必要範囲を記録）。
- RCP-EX-06 実機 Gate: PCM・実 Plugin・実 OS・実 RT
  （synvelis#296 RCP-GATE）。headless Pass を実機 Pass に
  昇格しない。
- Render eligibility evaluator 正本（#290/#291/#292）との
  contract 整合確認。
