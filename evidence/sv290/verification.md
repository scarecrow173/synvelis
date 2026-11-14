# Verification — synvelis#290 RCP-01 headless スライス

Date: 2026-11-14. Canonical 検証は `mise run` 経由。
OS: Windows 11。Toolchain: mise 管理 Rust（lint 出力の
rustc 1.99.1）。

## 実行結果（実測）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0。初回は新規ファイルの整形差分で fail → `cargo fmt --all` 適用後 green） |
| `mise run check synvelis-project` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-render` | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-project` | Pass（exit 0、clippy `-D warnings`） |
| `mise run lint synvelis-render` | Pass（exit 0、clippy `-D warnings`） |
| `mise run test synvelis-project` | Pass（exit 0 — 全 test suite green） |
| `mise run test synvelis-render` | Pass（exit 0 — 全 suite green。`rcp01_recipe.rs` 11/11 を含む: l2_e2e 17, rcd_04 2, rcp01 11, sf03 2, tim04 5, x_e2e 5） |
| `mise run check <downstream>` | synvelis-project 直接依存クレート — 結果は PR 本文に記録 |
| docs index 再生成（doc-maintenance `index plan`→`index apply`） | `docs/impl/ir`・`docs/test-specs` written / blocked 0。`docs/impl/exp` は in-sync（`.jsonl` のみ管理・`.md` memo は index 対象外） |
| `doc_status.js lint --dir docs/test-specs` | 範囲内 Pass — `rcp-01-render-recipe.md` finding 0 件（dir 全体の既存 baseline finding は範囲外・未修復） |
| `audit_impl_record.js --dir docs/impl/ir` | 範囲内 Pass — `rcp-01-render-recipe.md` finding 0 件（baseline 残存） |

## 補助証拠（canonical = mise）

- `cargo fmt --all` 適用（mise 管理 toolchain 経由）。
- lint で検出・修正した点:
  - `render_input.rs`: `too_many_lines` ×2 —
    `all_semantic_objects` を domain 別 helper
    （arrangement/media/recording/comp/control/routing/misc）
    に、`canonical_entity_value` を special/keyed の 2 helper +
    dispatcher に分割（`#[allow]` 不使用）。
  - `dependency.rs`: `similar_names`（`pref`→`proc_ref`）、
    `if_not_else`（invert）、`must_use` 付与、
    `too_many_lines` — `resolve_recipe` を `schema_gates` /
    `input_gates` / `build_closure` に分割。
  - `recipe.rs`: `large_enum_variant` —
    `RecipeResolution::Resolved` の `recipe`/`provenance` を
    `Box` 化。
  - `invalidation.rs`: `doc_lazy_continuation` — doc list を
    nested bullet 化。
  - `rcp01_recipe.rs`: `similar_names` ×2、`needless_pass_by_value`
    （`apply` が `op` を消費する形へ）、`too_many_lines` ×3
    （rec01 を scenario helper 群へ分割、rec02 weak-asset
    fixture を helper 化、rec03 を `resolve_with` へ集約）、
    `match_wildcard_for_single_variants`、`assert_is_empty`。
- test fixture で判明・修正した点:
  - master bus は well-known ID namespace（`0x00000001…`）で
    `BTreeMap` 順の `next_back` では選べない — bus/strip は
    kind/owner で選択（fixture コメントに記録）。
  - sidechain source は同一 owner 不可（`RoutingModel::validate`
    の既存不変条件）— 第 2 strip を source に。
  - automation region の point は半開区間 `[start,end)` の
    end 上に置けない — `num: 3` へ。
  - `AddTempoEvent{tick:0}` は seeded tempo event と重複 —
    `tick: 3_840` へ。
  - non-finite 値は project `validate_references` が先に reject
    し得る — `resolve_recipe` 直接経路でも fail-closed を確認。

## NotRun

- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 — タスク指定に
  より対象外。
- 実 Plugin host・実 OS device/clock・実 codec・実 render 実行・
  実 RT — 本スライスは headless fixture のみ。Fake/static probe
  の PASS を実機 PASS に昇格しない（後続 Gate の責務）。
- `synvelis-workspace` Gateway projection — 別 Worker の lease
  のため DEFER（編集も検証も対象外）。
