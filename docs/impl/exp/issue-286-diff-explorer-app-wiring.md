---
id: "EXP-286DiffExplorerAppWiring08"
type: "experiment"
status: "completed"
title: "Issue #286 impl EXP — Difference Explorer app wiring 実験記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/286"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/issue-286-diff-explorer-app-wiring.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-286-diff-explorer-app-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/cb-09-diff-explorer.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-286-diff-explorer-app-wiring.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #286 impl EXP — Difference Explorer app wiring 実験記録

## 実験サマリ

app wiring 実装中に発生した設計上の分岐を実測で確定した。
主な結論: (1) adapter 登録の authoritative な一箇所は
`VariantPorts::standard` であり facade 側の手動 install は
不要、(2) view-local action は authority context を要求
しない別経路が必要、(3) adopt は compare target を mint
しない、(4) project 切替時の session クリアが衝突回避に
必須。

## 採否記録（exp.jsonl と対応）

| EXP | 仮説 | 結果 | 要点 |
| --- | --- | --- | --- |
| EXP-1 | facade 側で `ArrangementAdapter` を `VariantDiffEngine::new` に手動 install する必要がある | rejected | upstream `VariantPorts::standard`（synvelis#539/#542, commit 2c41dae7）が CB-07 adapter を authoritative に登録。facade 側 install は stale 前提の冗長コードとして除去（3d9a67bb） |
| EXP-2 | view-local action も `variant_diff_input` の persisted target/context を要求できる | rejected | fresh view（target 無し）で selection/layer visibility が fail-closed になる設計 gap を app test で実測。`apply_variant_diff_view_local` を workspace に追加し authority 非依存経路へ分離 |
| EXP-3 | adopt commit 成功後に compare target を自動 mint してよい | rejected | target は明示 pin のみが権威を持つべきで、adopt 結果の ambient 昇格は stale-context 検証を迂回する。view は untargeted/unavailable を正直に維持（test で固定） |
| EXP-4 | project 切替後も per-view session/engine を保持してよい | rejected | identical bundle copy の別 project で branch id が衝突し前 project の session/cache を誤再利用する経路を回帰 test で再現。切替時に engine/session/target/publish observation を全クリアする方式を採用 |
| EXP-5 | op 面は `syn.variant-diff.visual`/`view`/`action` の 3 本で十分 | adopted | visual=projection read、view=lifecycle、action=dispatch の責務分離で service registry に収まり、app test 11 件 + mount smoke で契約を固定 |
| EXP-6 | signal provider 出力を `AuxLayerEvidence` へ provenance 保持で写像できる | adopted | CB-08 provider seam を facade 内 adapter で写像し `cb09_engine_compare_maps_signal_provider_provenance` で provenance 保持を実測。unavailable/unknown/unsupported は zero-diff に変換しない |

## 実測

- workspace `cb09_diff_explorer`: 40/40 Pass（compare・
  provider provenance・CAS adopt・typed reject・
  authority/context・adapter 解決を含む）。
- app `cb09_variant_diff`: 11/11 Pass（op dispatch・view
  lifecycle・mount smoke・project 切替 cleanup・
  stale/implicit target reject・preview 非 commit）。
- `mise run check-app-native` / `lint-app-native` /
  `test-app-gpui` は最終 head で exit 0
  （`evidence/sv286/app-wiring-verification.md`）。
- aggregate `mise run fmt-check` は本 worktree で
  Windows `os error 206`（環境起因）により exit 1。
  全 15 member の個別 `cargo fmt -p -- --check` は pass、
  main repo では aggregate も exit 0 を確認済み。

## 関連リンク

- `synvelis#286`（本 Issue）/ CB-07 `synvelis#284` /
  CB-08 `synvelis#285` / adapter 登録 `synvelis#539`・
  PR #542
- IR: `docs/impl/ir/issue-286-diff-explorer-app-wiring.md`
- 証跡: `evidence/sv286/app-wiring-verification.md`

## 関連

- 前段 EXP: `docs/impl/exp/cb-09-diff-explorer.md`
- 残課題: 実 GPUI window/plugin dock/RT/PCM/perceptual
  acceptance は synvelis#288・CB-11 gate の範囲
