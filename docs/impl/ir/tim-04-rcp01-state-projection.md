---
id: "IMPL-Tim04Rcp01StateProjection01"
type: "impl"
status: "approved"
title: "TIM-04 RCP-01 persisted plugin-state projection implementation record"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
---

# TIM-04 RCP-01 persisted plugin-state projection implementation record

## 実装概要

`crates/synvelis-render/src/plugin_state.rs` に `PersistedPluginStateProjection` を追加した。projection は `synvelis-project::plugin_state::PluginStateStore` の HEAD または explicit manifest から manifest bytes を読み、manifest 名 digest、JSON decode、schema version、duplicate processor、`Captured` blob の存在・長さ・SHA-256 digestを construction 時に検査する。検査済み outcome は owned `BTreeMap` として保持し、RCP-01 `PluginStateProbe` へ非 RT control path から渡す。

`dependency.rs` では probe outcome を `Captured` / `Unsupported` / `Failed` の sum type へ拡張した。`Captured` だけが `PluginStateEvidence { digest, quality }` を持ち、`Coordinated` / `Quiescent` だけが exact-eligible closure に入る。`BestEffort`、`Unsupported`、`Failed` は exact-ineligible な typed `Unknown` になる。`Failed` kind は `PluginStateDegraded.failure_kind` に保持し、新しい `UnknownCause` variant は追加しない。これは `synvelis-workspace` の exhaustive match を壊さないための互換境界である。

## Test / evidence

`crates/synvelis-render/tests/tim04_rcp01_state_projection.rs` は次を headless に固定する。

- `Coordinated` / `Quiescent` captured state が Resolved になり、digest/quality を変更せず closure に入る。
- `BestEffort`、`Unsupported`、全 `Failed` kind、missing evidence が typed `Unknown` になる。
- HEAD absence、manifest/store read failure、malformed JSON、schema mismatch、duplicate processor、manifest digest mismatch、missing/corrupt/unreadable blob が projection error になる。
- verified artifact candidate が存在しても non-`Resolved` resolution は `LookupOutcome::Unknown` であり、`EligibleExact` / `EligiblePreview` に昇格しない。
- `PluginStateEvidence` serialized shape、`RECIPE_SCHEMA_VERSION`、`CANONICALIZATION_VERSION` が変わらない。

machine-readable rows は `evidence/sv128/tim04_rcp01_state_projection.jsonl`、日本語 summary は `evidence/sv128/tim04-rcp01-projection.md` にある。

## 残件 / non-claims

実 plugin Offline render、plugin-originated PermissionDenied capability、typed runtime process oracle、製品 registry/discovery、GUI/Native Editor、AU/市販 plugin、物理機材、AudioLens cross-gate、3OS final acceptance は未実施である。docs index は Coordinator が統合時に整備する。
