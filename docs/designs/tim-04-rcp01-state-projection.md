---
id: "DESIGN-Tim04Rcp01StateProjection01"
type: "design"
status: "approved"
title: "TIM-04 RCP-01 persisted plugin-state projection design"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
---

# TIM-04 RCP-01 persisted plugin-state projection design

## 責務境界

`synvelis-project` は plugin state の capture、restore、manifest、blob、HEAD の保存契約を所有する。`synvelis-render` はその保存済み outcome を render recipe の input identity へ投影する。本設計では `synvelis-render` に persisted projection を置き、保存契約や host adapter を複製しない。

RCP-01 の resolver は live plugin や store を直接読まない。非 RT の control path が projection を構築し、owned evidence table を `PluginStateProbe` として resolver へ渡す。projection は `Send` な owned data であり、RT callback から state save/restore/FS IO を呼ばない。

## Data shape

probe outcome は次の三態である。

- `Captured(PluginStateEvidence { digest, quality })`: blob digest が検証済みの captured state。quality は manifest の値を変えずに渡す。
- `Unsupported`: plugin が state capture に非対応。digest は存在しない。
- `Failed { kind }`: capture または persisted input が typed failure。digest は存在しない。

processor の record が valid な manifest にない場合は probe が `None` を返し、既存どおり `PluginStateUnknown` となる。この三態により、旧来の `Option<PluginStateEvidence>` では blob-less outcome を表すために fake digest が必要だった問題を除く。

設計候補として、旧 `evidence` method を残しつつ richer outcome method を additive に足す形も検討した。その形はソース互換を保つ一方、同じ processor について二つの method が矛盾した答えを返せる。本 slice では crate 内の実装が `NoPluginStates` と `StaticPluginStates` だけなので、probe の authoritative outcome を一つの sum type に集約する。`PluginStateEvidence { digest, quality }` 自体の serialized shape は変えない。

## Projection validation

`PersistedPluginStateProjection` は store HEAD または explicit manifest 名から manifest bytes を読み、次を construction 時に検査する。

1. manifest 名が `ps-<digest16>.json` 形式であること。
2. manifest 名が bytes の content-derived name と一致すること。
3. JSON が `PluginStateManifest` として decode できること。
4. `schema_version == PLUGIN_STATE_SCHEMA_VERSION` であること。
5. processor id が重複しないこと。
6. `Captured` record の blob が readable であり、`blob_len` と SHA-256 digest が一致すること。

いずれかに失敗した場合は projection を構築しない。digest mismatch や absent blob を Quiescent/Coordinated evidence にしない。manifest bytes digest、manifest name、project id、project revision は owned metadata として保持し、evidence 記録と将来の呼び出し側検査に使えるようにする。

## Resolution behavior

`Captured` evidence は `quality.exact_eligible()` のときだけ Exact recipe の dependency closure に入る。`Coordinated` と `Quiescent` は closure に入り、`BestEffort` は `PluginStateDegraded` の typed `Unknown` となる。`Unsupported` と `Failed` も closure に入らず、同じく `PluginStateDegraded` となる。blob-less outcome では `quality` を outcome label（`Unsupported` / `Failed`）として使い、`Failed` では additive な `failure_kind` に `capture_failed`、`timeout` などの machine-readable kind を保持する。

新しい `UnknownCause` variant を足さないのは、`synvelis-workspace` などの直接下流が exhaustive match で `UnknownCause` を消費しているためである。既存 variant への optional field 追加にとどめることで、下流の match を壊さず、resolution-time detail だけを拡張する。`failure_kind` は exact recipe の canonical input ではない。

この unknown は resolution-time の failure detail であり、canonical recipe bytes には入らない。exact-eligible な recipe に含まれる `PluginStateEvidence` は従来どおり `digest` と `quality` だけを持つ。したがって `RECIPE_SCHEMA_VERSION` と `CANONICALIZATION_VERSION` は変更しない。recipe key layout、canonical byte ordering、exact evidence serialization の互換性は保たれる。

## CacheLookup interaction

RCP-02 `CacheLookup` は non-`Resolved` resolution を受け取った時点で `LookupOutcome::Unknown` を返し、candidate を評価しない。本 slice はその既存契約を、verified artifact candidate が存在する状態で縦断 test する。projection 由来の `Unknown` resolution が `EligibleExact` または `EligiblePreview` に昇格しないことを固定する。

## Error boundary

projection error は store read、manifest integrity、blob integrity の失敗を表す。missing evidence、save-time `Unsupported`、save-time `Failed` は projection error ではなく valid manifest 内の outcome なので、RCP-01 の typed `Unknown` となる。この分離により、persistence が壊れた場合と plugin state が exact-eligible でない場合を evidence で区別する。

## Deferred concerns

実 plugin Offline render、plugin-originated PermissionDenied capability、typed runtime process oracle、製品 registry/discovery は upstream capability または別 owner の設計が必要な残件である。本設計はそれらを fake-pass しない。
