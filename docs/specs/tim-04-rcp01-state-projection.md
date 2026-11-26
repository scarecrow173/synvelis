---
id: "SPEC-Tim04Rcp01StateProjection01"
type: "spec"
status: "approved"
title: "TIM-04 RCP-01 persisted plugin-state projection"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
---

# TIM-04 RCP-01 persisted plugin-state projection

## What Should Be Built

Issue #128 の TIM-04 slice として、`synvelis-project` の `PluginStateStore` / `PluginStateManifest` に保存された plugin state outcome を、`synvelis-render` の RCP-01 `PluginStateProbe` から `RecipeResolution`、さらに RCP-02 artifact `CacheLookup` の eligibility まで fail-closed に接続する。対象は保存済み state evidence の projection であり、実 plugin を使った Offline render 実行ではない。

projection は manifest の `Captured { quality, blob_digest, blob_len }` を processor ごとの evidence として保持する。`Coordinated` と `Quiescent` は exact-eligible、`BestEffort` は exact-ineligible である。`Unsupported` と `Failed` は blob を持たない outcome なので、空 digest・zero digest・fake digest を作らず、typed に exact-ineligible へ写像する。`Failed` は少なくとも `CaptureFailed`、`Timeout`、`PermissionDenied`、`Crashed`、`Corrupt`、`Unavailable`、`Missing` を区別できる。

## Fail-closed requirements

projection は store HEAD または explicit manifest を読む経路で、次を成功扱いしない。

- HEAD がない、HEAD/manifest/blob の読取りに失敗する。
- manifest bytes が読めない、JSON として不正、schema version が現行 `PLUGIN_STATE_SCHEMA_VERSION` と一致しない。
- manifest 名が content-derived digest と矛盾する。
- 同じ processor が manifest に重複する。
- `Captured` record が参照する blob がない、読めない、長さまたは SHA-256 digest が record と一致しない。

これらは projection error として扱い、空 evidence table を exact resolution と混同しない。valid な manifest に対象 processor の record がない場合だけ、RCP-01 側で missing evidence の typed `Unknown` となる。

## Candidate decision

本 slice は、外部 capability を待たず現行 main で完結する最小縦断として、persisted TIM-04 state → RCP-01 Recipe → RCP-02 Eligibility の projection を選ぶ。候補外の判断は次のとおりである。

- plugin-originated PermissionDenied capability は選ばない。現行 ruviform resolved rev `0479fa62e77e669a7bf84320d3d5418ffb949fc6` の `test_fault` fault bands は `none/abort/exit/hang/error/rt_alloc/bad_time/invalid` で permission band がなく、`HostError` に PermissionDenied variant がない。PR #676 でも VST3/CLAP capability verdict は `Unsupported` であり、実測 Pass 化は ruviform fixture/contract が hard blocker である。
- 実 plugin を組み込んだ Offline render 実行は選ばない。`synvelis-render/src/export.rs` は `OfflineRenderSupport::Verified` 以外を preflight で reject し、ruviform `GraphProcessor::offline_support` default は `Unknown`、bridge `PluginNode` に Verified override が確認できない。L1 の pass-through/substitution/lookahead を offline exact へ転用できないため、upstream capability が hard blocker である。
- typed runtime process oracle は選ばない。real ABI wrapper は `create_sendable` capability 不足で `EndpointUnavailable` と既測であり、ruviform 側 capability が hard blocker である。sv#127 transport application Worker が同じ検証面とファイルを占有していることも重複回避の理由である。
- 製品 registry/discovery は選ばない。scan、descriptor provenance、format 判定、per-plugin child route、runner 配布/解決の設計が必要であり、現行 runner 契約だけでは production composition を閉じられない。

## Out of scope / non-claims

- 実 plugin の Offline render 実行、実 ABI による Pass、typed runtime process oracle、製品 registry/discovery を主張しない。
- GUI/Native Editor、AU、市販 plugin、物理機材、AudioLens cross-gate、3OS final acceptance を主張しない。
- `BestEffort` や `Unsupported` から BitExact/Exact cache eligibility を主張しない。
- ruviform pin bump、ruviform 側修正、実 runner 変更、`synvelis-project` の保存契約変更、`synvelis-player` host adapter の複製を行わない。

## Canonical compatibility

exact-eligible recipe に入る `PluginStateEvidence { digest, quality }` の serialized shape、`RECIPE_SCHEMA_VERSION`、`CANONICALIZATION_VERSION`、recipe key layout は変更しない。`Unsupported` / `Failed` の detail は canonical recipe bytes ではなく resolution-time の typed unknown にとどめる。blob-less outcome に empty/zero/fake digest を入れない。

## Acceptance mapping

本 spec は Issue #128 の `TIM04-RCP-01` のうち persisted state projection を前進させる。`Coordinated`/`Quiescent`/`BestEffort`/`Unsupported` と外部 asset 不明を Recipe/Eligibility へ渡し、`BestEffort` から無根拠な BitExact を主張しない部分を対象とする。Save/Reload の full matrix、plugin crash/timeout/permission の実 ABI 受入、Undo/History/Copy Restore、実 CLAP/VST3 capability 測定、Windows 最終受入は残件である。
