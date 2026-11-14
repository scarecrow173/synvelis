---
id: "EXP-rcp01-render-recipe"
title: "RCP-01 Canonical Render Recipe / typed closure / temporal invalidation 設計実験メモ (synvelis#290)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: RCP-01 render recipe headless スライス（synvelis#290）

## E1 branch authority 識別子を hashed identity から外す方式

**仮説**: RCP-REC-01 は「異なる branch で同一音響 graph は
同一 canonical recipe byte/key」を要求する。一方 CB-01 の
`BranchRenderPin` 相当の情報（variant set ID・branch ID・
branch content revision・payload digest・locator）は authority
provenance であり、音響同一性ではない。

- 案 A: pin 全体を recipe に含める → 同一音響内容でも branch
  ごとに key が分岐し RCP-REC-01 違反。
- 案 B（採用）: `RecipeBranchPin` を canonical
  `scope_digest` + `shared_context_digest` の 2 要素に限定し、
  authority 識別子は `ResolutionProvenance`（非 hash）へ分離。
  `scope_digest` は display name を含まない canonical
  fingerprint を別途計算する（CB-01 の `scope_digest` は
  `entity_value` 経由で name を含むため、render identity には
  流用しない — payload integrity 検査には従来 digest を使う）。

**観察**: `rec05_shared_edit_invalidates_all_branches_internal_edit_isolated`
で、live shared context の編集が `shared_context_digest`
経由で全 branch の key を動かし、branch 内部編集は scope
fingerprint 経由で当該 branch のみを動かすことを確認。

## E2 canonical entity 抽出と display name の扱い

**仮説**: `Project`/`RoutingModel` 等の aggregate をそのまま
digest すると subgraph 分離（RCP-REC-03 の無関係 subgraph
不変条件）が壊れる。また display `name` は UI rename で変わる
ため音響 identity に含めてはいけない。

- `canonical_entity_value` は entity ごとに canonical JSON
  value を生成し、`name` field を再帰的に除去する。
  `RoutingModel` は connection/port/strip/processor の
  semantic 内容のみを写像し、display name・作成時刻・
  revision・artifact location を除外する。
- 非有限 float（NaN/±inf）は canonicalization で reject
  （`0`/`null` への正規化はしない）。
- map/set は key ソート済み直列化で挿入順非依存。

**観察**: `rec01_display_rename_and_insertion_order_do_not_move_key`
で rename・追加順 shuffle が key を動かさないこと、
`rec02_nonfinite_and_schema_skew_fail_closed` で non-finite が
project validation を通過した経路（`resolve_recipe` 直接）
でも fail-closed になることを確認。

## E3 typed closure の走査起点と既知依存の列挙

**仮説**: render boundary port から routing を逆走査すれば、
audio/event/control/parameter/sidechain/external-media/
transport/time-mapping の typed edge で依存閉包を構成できる。

- `resolve_recipe` は `RenderBoundary`（master 出力 port 集合）
  を起点に `RoutingModel::validate` 済みの graph を BFS し、
  `DependencyNode`（semantic object + canonical digest）と
  `DependencyEdge`（typed kind）を構築する。
- processor は `PluginStateProbe`・`ExternalAssetProbe` の
  注入境界で品質ゲートを受ける。probe 未解決・degraded・
  unpinned・weak digest・unverified source・live clock は
  すべて `UnknownDependency`/`UnsupportedReason` として
  収集し `RecipeResolution::Unknown`/`Unsupported` に落とす
  （`Resolved` に混ぜない）。
- schema gate: `RENDER_INPUT_SCHEMA_VERSION` /
  `SCHEMA_VERSION` / `RECIPE_SCHEMA_VERSION` の不一致は
  traversal 前に `Unsupported`。

**観察**: `rec03_closure_covers_signal_and_unrelated_subgraph_isolated`
で closure が signal path・send/return・sidechain・automation
を含み、無関係 track の追加が key を動かさないことを確認。
`RoutingModel::validate` を `pub` に widening したのは
render crate が sanctioned read consumer として cycle/owner
不変条件を再検証するため（write 権限の付与ではない）。

## E4 temporal invalidation の保守的拡張

**仮説**: dirty interval の再利用判定は「不足なく拡張」
（over-approximate）が正しく、過少区間再利用は許されない。

- `classify_invalidation` は `ChangedObject`（semantic object
  + 任意 frame interval + temporal flag）を closure membership
  で filter し、該当するものだけ `InvalidationPlan` の
  interval 列へ変換する。
- 既知 interval は lookahead + PDC 分だけ後方、declared tail
  分だけ前方へ拡張。loop region と overlap すれば loop 全体を
  カバー。seek boundary を跨ぐ変更は render 終端まで拡張。
  temporal-domain 変更・未知 extent は render 全域へ fail。
- interval は render interval に clip し、merge（接するものも
  統合）して返す。

**観察**: `rec04_*` で lookahead/PDC/tail/loop/seek の各拡張と
未知 extent・temporal-domain の全域失効を確認。checked
`Tick`/`TickDuration` 演算のみ使用（unchecked 演算子不使用）。

## E5 RecipeRecord の fail-closed 検証

**仮説**: cache index/record は「verify してから信じる」。
canonical bytes からの key 再計算が一致しない record・
schema version 不一致の record は `Match` に昇格させない。

- `RecipeRecord::verify` は schema version を確認し、
  `canonical_bytes` から `RecipeKey` を再計算して照合する。
- `evaluate_validity` は verified record 同士の key 比較で
  `Match`/`Stale`、verify 失敗・`Unknown`/`Unsupported`
  resolution は `Unknown` を返す。

**観察**: `rec06_record_verify_and_collision_fail_closed` で
正常 record の `Match`/`Stale` と、digest 改竄・version skew
の `Unknown` を確認。
