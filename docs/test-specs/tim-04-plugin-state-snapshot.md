---
id: "TSPEC-034UC53kVrQjWrtds8Lrsn"
type: "test-spec"
status: "approved"
title: "TIM-04 plugin state snapshot consistency — test spec"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-04 plugin state snapshot consistency — test spec

## Purpose

Issue #128 の headless/mock 受入行を fixture test で固定する。
実 CLAP/VST3/AU・実 OS・GUI は対象外（`NotRun`/`Unsupported`
として impl record に記録）。

## Feature

Plugin State Snapshot の保存品質は capture 時に観測された
事実（processing・stable_boundary・port achieved）から分類
され、receipt・manifest・blob に記録される。File Save 成功・
Plugin State Exact・RT Active は別 receipt であり、混同
しない。

## Rules

- R1: `StateCaptureQuality` は `Coordinated`/`Quiescent`/
  `BestEffort`/`Unsupported` の 4 値。`exact_eligible()` は
  `Coordinated`/`Quiescent` のみ true。
- R2: save は全 processor を `ProcessorId` 順に capture し、
  `Unresolved` binding・live instance 不在を
  `Failed(Unavailable)`、plugin 非対応を `Unsupported`、
  その他を `Failed(kind)` として blob なしで記録する。
- R3: `Strict` policy は `BestEffort`/`Unsupported`/`Failed`
  を含む save を `QualityRejected` で拒否し、blob・manifest・
  HEAD のいずれも publish しない（store 未接触）。
- R4: `Standard` policy は全 outcome を記録して保存を許容
  するが、receipt/manifest/summary は品質を正直に記録し、
  BestEffort を exact と偽らない。
- R5: blob は content-addressed（`ContentDigest` sha256）、
  manifest は versioned JSON + content-addressed 名
  （`ps-<digest16>.json`）+ HEAD pointer。
- R6: manifest publish が `Indeterminate` の場合、HEAD 名と
  manifest bytes・blob digest を readback で照合し、一致す
  れば `reconciled: true`、不一致は typed error。blob put の
  `Indeterminate` も read_blob+digest 照合で reconcile。
- R7: restore は manifest schema/digest 検証 → record 毎に
  blob read + sha256 照合（不一致は `Failed(Corrupt)`）→
  port restore。manifest に無い live processor は
  `missing_states` に列挙し skip しない。
- R8: `PluginStateReceipt`/`PluginStateRestoreReceipt` は
  `SavedState`（project HEAD）や RT activation receipt と
  別物で、`project_revision`/`project_head` で対応付く。
- R9: player adapter の capture/restore は非 RT のみ。
  `PluginHost` の `SaveState`/`RestoreState` を消費し、
  lifecycle 前後観測で `processing`/`stable_boundary` を
  求める。`stable && non-processing` なら実測品質を記録
  （mode=BestEffort でも Quiescent/Coordinated を過小申告
  しない）。
- R10: render `PluginStateRecipe` は `policy`+`evidence`+
  `manifest_digest` を運び、`Strict` で
  `!evidence.exact_eligible()` を `PreflightError` で拒否。
  `ExportReceipt` は recipe evidence をそのまま記録する。

## Examples

- E1: mock port が `Coordinated` blob を返す Standard save →
  receipt `Captured{Coordinated}`、manifest publish、HEAD が
  manifest 名を指し、blob digest が readback で一致。
- E2: mock port が `Unsupported` を返す Standard save →
  record `Unsupported`、blob なし、receipt に品質が残る。
  Strict save → `QualityRejected`、store head/blob 不変。
- E3: mock port が `Failed(Crashed)`/`Failed(Timeout)`/
  `Failed(PermissionDenied)` → record `Failed(kind)` で typed。
- E4: restore で blob bytes を改竄 → sha256 不一致で
  `Failed(Corrupt)`、port.restore は呼ばれない。
- E5: manifest publish `Indeterminate` + HEAD readback 一致
  → `reconciled: true` receipt。HEAD が別 manifest → typed
  error、成功断定しない。
- E6: store copy（InMemory clone / Directory 再読込）後の
  restore → record/quality/digest が維持される（Undo/History/
  Copy Restore 相当）。
- E7: player adapter で activated plugin の SaveState →
  非 Processing で stable なら `Coordinated`（mode=Coordinated）
  または `Quiescent`、Processing 中なら `BestEffort`。
- E8: render `Strict` + `exact_eligible()==false` の evidence
  → preflight 拒否。`Standard` → receipt に evidence 記録。

## Guarantees

- Save Completed ≠ Plugin State Exact ≠ RT Active が構造的に
  分離される。
- 空 blob・recovery blob・旧 blob を無検証で現行 state と
  みなさない（digest/identity 照合を必須とする）。
- RT callback から state save/FS IO を呼ばない。
- 実 plugin/OS/GUI は `NotRun`/`Unsupported` で偽装しない。

## Non-goals

- 実 CLAP/VST3/AU・OS capability 実測、実 OS FS atomicity、
  L1 out-of-process state、GUI、RT 経路、VCS snapshot への
  manifest 統合、`synvelis-app`/`workspace` 等の変更。

## Risk

- 品質分類を誤ると「保存できたのに exact でない」事故に
  なる。port achieved + observation の二重記録と Strict
  fail-closed で緩和する。
