# AUTX-04 Atomic Macro v1 — Case Ledger（synvelis#263 headless slice）

Date: 2026-11-14。Slice: `synvelis-automation` への versioned
Atomic Macro Recipe モデル + `MacroCompiler`/`MacroRegistry`
+ `Step::AtomicMacro` plan 展開、および `synvelis-gateway`
batch `bindings` typed reject fallback。**本 slice は
headless の compile-time 契約のみ**を対象とし、create-
then-reference の end-to-end commit（GW-04 scratch rewrite
→ `Composite::validate` 逐次化 → apply）は `synvelis-project`
リース（sv#290）依存の Deferred である。headless の Pass を
runtime/実機の Pass に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/263
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `scarecrow173/sv263-autx4`（worktree `sv263-autx4`） |
| OS / CPU / FS | Windows 11 / Intel Core i9-9900K / NTFS（C:） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89 |
| Build | mbx managed target（worktree `target` リンク先） |
| Registry input | serialized Registry Snapshot fixture（`tests/macros.rs`・`tests/common` 内 literal。live Gateway 非依存） |

## 判定語彙

- **Pass**: 当該 SHA で検証が exit 0 で完了し、対象
  test/assertion が green だったことを実測した。
- **NotRun**: 本 slice で実行していない項目（後続 slice・
  MSRV lane・実機/実 Job 結合）。
- **Deferred**: 別 Issue/別リースが Owner の項目
  （sv#290 の `Composite::validate` seam、#265/#266/#267/
  #272/#273）。

## 実行した正規検証

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-automation` | Pass（exit 0） |
| `mise run check synvelis-gateway` | Pass（exit 0） |
| `mise run lint synvelis-automation` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-gateway` | Pass（exit 0、clippy warning 0） |
| `mise run test synvelis-automation` | Pass（99 test: lib 1・compiler 43・decode 11・determinism 12・macros 17・yaml_subset 15。doc-test 0） |
| `mise run test synvelis-gateway` | Pass（26 test binaries・0 failure。`prepare_engine` 新規 3 test 含む） |

補助証拠（canonical = 上記 mise task）: 上記 cargo test は
mise task と同一 target の直接実行であり、結果は一致。

## 受入条件 → case 対応表

### AC-1 versioned recipe definition・decode integrity

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| DEF-G01 schema_version/macro_id/version/shape | version 1 のみ受理、未知 field・欠落 field reject | decode error | Pass | `tests/macros.rs` decode/shape reject 系 |
| DEF-G02 authority 非保持 | principal/grant/approval/secret field は表現不能 | variant/field 非存在 | Pass | `AtomicMacroDefinition` 型に該当 field なし |
| DEF-G03 digest 決定性 | 同一 definition → 同一 canonical JSON + SHA-256 digest、key 順非依存 | 決定的 digest | Pass | deterministic compile test |

### AC-2 static compile fail-closed

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| CMP-G01 capability exact version | `(id, version)` 完全一致、latest 解決なし | exact match のみ | Pass | registry/compiler tests |
| CMP-G02 domain/execution | 非 `ProjectMutation` domain・async capability reject | typed reject | Pass | `reject_*` domain/execution tests |
| CMP-G03 target contract | implicit target・kind/shape 不一致 reject | typed reject | Pass | target contract tests |
| CMP-G04 member/ceiling | >64 member・policy ceiling 超過 reject | typed reject | Pass | policy/size tests |
| CMP-G05 nested batch | batch capability member reject | typed reject | Pass | nested batch test |
| CMP-G06 reserved ID | 予約レンジ literal/param 値 reject | `ReservedIdLiteral` | Pass | reserved id tests |

### AC-3 output binding・決定的 placeholder

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BND-G01 flat ops + bindings | create→参照 recipe が flat `ops[]`・named binding・member index 参照を生成 | wire 形状一致 | Pass | `compile_emits_flat_ops_and_bindings` |
| BND-G02 placeholder | `$output.<name>` が `OUTPUT_BINDING_ID_BASE` レンジの `kind-<32hex>` placeholder | 予約レンジ確認 | Pass | 同上 |
| BND-G03 実 ID 非推測 | compile 出力に実 project ID を含まない | placeholder のみ | Pass | 同上 |
| BND-G04 batch_args | `ops` 常時 array・`bindings` 非空時のみ | wire shape test | Pass | `batch_args()` test |

### AC-4 workflow plan 展開

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| PLN-G01 registry 解決 | `UndefinedMacro`/`UnknownMacro`/`UnsupportedMacroVersion` 区別 | 区別された error | Pass | plan macro tests |
| PLN-G02 bounds 会計 | macro member の step/effect/data-access/capability が plan bounds に反映 | 会計一致 | Pass | plan expansion test |
| PLN-G03 macro-only expr | workflow 内 `$param`/`$output` は `MacroOnlyExpr` reject | typed reject | Pass | `plan_rejects_macro_only_exprs_and_bad_args` |
| PLN-G04 macro target | macro-level target は reject | typed reject | Pass | 同上 |

### AC-5 gateway bindings fallback（Deferred seam の fail-closed）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| GW-G01 bindings schema | `bindings` optional array max 64 受理、非配列は SchemaViolation | schema enforced | Pass | `batch_bindings_shape_is_schema_enforced` |
| GW-G02 非空 bindings reject | `PreparationRejected` typed reject・reason に Workflow 分解誘導 | typed reject | Pass | `batch_output_bindings_reject_typed_unsupported` |
| GW-G03 bindings なし非破壊 | `bindings` 欠落・空配列は従来通り lower | lowering 成功 | Pass | `batch_without_bindings_still_lowers` |

### Deferred（NotRun）

- create-then-reference end-to-end: GW-04 scratch rewrite・
  preview/commit ID CAS・`Composite::validate` 逐次化 —
  `synvelis-project` リース（sv#290）完了後の後続 slice。
- 実 Gateway request 経路・live registry・GUI・OS 実機結合
  （#265/#266/#267）、Action Script lowering（#272）、
  Workflow Script owner（#273）。
