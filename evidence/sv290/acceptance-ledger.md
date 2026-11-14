# Acceptance Ledger — synvelis#290 RCP-01 headless スライス

Date: 2026-11-14. Slice: RCP-01 Canonical Render Recipe +
typed dependency closure + temporal invalidation の
`synvelis-project`（Semantic ID/snapshot read adapter）と
`synvelis-render`（Recipe/Dependency Resolver）への headless
実装。Artifact Store/GC（RCP-02）・Render Job/Policy
（RCP-03）・実行経路選択（RCP-04）・workspace/Gateway
projection・実 Plugin/実 OS Gate は DEFER。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/290
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | feat/290-render-recipe HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 |
| Toolchain | mise 管理 Rust（lint 出力 rustc 1.99.1、`mise run` 経由） |
| Project schema | v14（変更なし — read adapter は既存 schema を消費） |
| Render-input schema | `RENDER_INPUT_SCHEMA_VERSION` = 1（新設） |
| Recipe schema | `RECIPE_SCHEMA_VERSION` = 1（新設） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-render/tests/
  rcp01_recipe.rs` が `mise run test synvelis-render` で green。
- **NotRun**: 実 OS/実 Plugin/実 codec/実 render 実行/実 RT —
  本スライスでは実行していない。
- **Deferred**: 後続 Issue の範囲として明示的に送った部分。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| RCP-REC-01 | Pass (headless) | `rec01_display_rename_and_insertion_order_do_not_move_key`（display rename・entity 追加順・fingerprint 順序が key 不変）、`rec01_acoustic_changes_move_key`（plugin parameter・FX reorder・sidechain・send/return・send tap・asset digest・tempo・automation・interpolation・renderer profile の各変更で key 変化）。別 branch 同一音響 graph の key 一致は `rec05_shared_edit_invalidates_all_branches_internal_edit_isolated` 内の branch fixture で確認（branch/set/revision/payload locator は hashed identity 外 — `RecipeBranchPin` は scope/shared-context digest のみ）。 |
| RCP-REC-02 | Pass (headless) | `rec02_plugin_state_unknown_and_degraded_fail_closed`（`NoPluginStates`→`PluginStateUnknown`、degraded→`PluginStateDegraded`、いずれも `Unknown`）、`rec02_unpinned_asset_unverified_source_live_clock`（weak digest+pin 無し→`UnpinnedExternalAsset`/`WeakAssetDigest`、`ExternalPin::Live`→`Unknown`、Imported source→`UnverifiedSource`、`ClockDependency::Live`→`Unsupported(LiveClock)`、`Unpinned`→`Unknown(UnpinnedClock)`）、`rec02_nonfinite_and_schema_skew_fail_closed`（NaN base→`InvalidValue`/`InputRejected` の fail-closed、旧 project schema→`UnsupportedSchema`）。いずれも `Resolved`/Exact CacheHit に昇格しない。 |
| RCP-REC-03 | Pass (headless) | `rec03_closure_covers_signal_and_unrelated_subgraph_isolated`（closure が Project/RoutingModel/Bus/Strip/ProcessorChain/Processor/SemanticPort/Track/Lane/Clip/MediaSource/Asset/ParameterTarget を含み、Audio/Event/Parameter/TimeMapping edge kind を確認。master 未接続の第 2 track subgraph は node 非含有・mute 変更で key 不変、closure 内 clip mute で key 変化）。 |
| RCP-REC-04 | Pass (headless) | `rec04_dirty_interval_expansion_and_fail_closed_full`（dirty [1000,2000) → lookahead+PDC 96 後方・tail 480 前方で [904,2480)、closure 外 object→interval 無し、未知 extent→`full`、temporal-domain→`full`）、`rec04_loop_seek_and_tail_expansion`（loop overlap→loop 全域、seek boundary 接触→終端まで拡張、merge 済み interval 列）。過少区間再利用なし（over-approximate のみ）。 |
| RCP-REC-05 | Pass (headless) | `rec05_shared_edit_invalidates_all_branches_internal_edit_isolated`（live shared context 編集で全 branch の key 変化、branch 内部 edit で他 branch 不変 — `shared_context_digest`/canonical scope fingerprint 経由）、`rec05_revision_only_change_does_not_invalidate`（revision のみの変化で key 不変）。 |
| RCP-REC-06 | Pass (headless) | `rec06_record_verify_and_collision_fail_closed`（正常 record の `verify`+`evaluate_validity`→`Match`/`Stale`、canonical bytes 改竄・key 衝突注入・schema version skew→`verify` err/`Unknown`、`Unknown`/`Unsupported` resolution は `Unknown` validity）。 |

## NotRun / Deferred 一覧

- 実 Plugin host・実 plugin state capture・実 OS device/clock・
  実 codec・実 render 実行・実 RT: NotRun（headless fixture +
  static probe のみ。`PluginStateProbe`/`ExternalAssetProbe`
  は注入境界であり、実 evidence 供給は後続 Gate）。
- Artifact Store/GC/remote upload（RCP-02）・Render
  Job/Policy（RCP-03）・実行経路選択（RCP-04）: Deferred。
- `crates/synvelis-workspace/` Gateway projection: Deferred
  （別 Worker の lease — 編集範囲外）。
- `PluginStateRecipe`（`synvelis-render/src/spec.rs`、plugin
  state 品質ゲート）との統合: 用語・責務は分離済み。品質
  ゲートの render 側接続は後続 slice。
- 旧 schema snapshot の render 解決: schema gate で
  `UnsupportedSchema` fail-closed（migration は対象外）。
- Source native fidelity の損失評価: canonical 抽出は
  display name/revision/location を除外するが音響内容は
  全量 digest — fidelity 退行は fixture の acoustic-change
  網羅で担保、実 media との bit 対応は NotRun。
