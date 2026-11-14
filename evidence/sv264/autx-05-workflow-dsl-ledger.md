# AUTX-05 Workflow DSL v1 — Case Ledger（synvelis#264 headless slice）

Date: 2026-11-14。Slice: `synvelis-automation` crate 新規実装
（YAML Workflow DSL v1・共通型付き Plan 検証/コンパイル・
静的予算）。**本 slice は headless の compile-time 契約のみ**
を対象とし、runtime 実行・実 Job/Recovery/OS/CLI・live
Gateway adapter・Script Builder・Macro 展開は実行していない
（#263/#265/#267/#273 の後続 slice — NotRun/Deferred）。
headless の Pass を runtime/実機の Pass に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/264
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `feat/264-workflow-dsl`（worktree `sv264-autx5`） |
| OS / CPU / FS | Windows 11 / Intel Core i9-9900K / NTFS（C:） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89 |
| Build | mbx managed target（worktree `target` リンク先。F: へ中間生成物を置かない） |
| Registry input | serialized Registry Snapshot fixture（`tests/common/mod.rs` 内 literal。live Gateway 非依存） |

## 判定語彙

- **Pass**: 当該 SHA で `mise run` task が exit 0 で完了し、
  対象 test/assertion が green だったことを実測した。
- **NotRun**: 本 slice で実行していない項目（後続 slice・
  MSRV lane・実機/実 Job 結合）。
- **Deferred**: 別 Issue が Owner の項目（#263/#265/#267/#273）。

## 実行した正規検証

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-automation` | Pass（exit 0） |
| `mise run lint synvelis-automation` | Pass（exit 0、clippy warning 0） |
| `mise run test synvelis-automation` | Pass（82 test: compiler 43・determinism 12・yaml_subset 15・decode 1・lib 内包。doc-test 0） |
| `mise run check-headless-deps` | Pass（11 headless crates、GUI 依存なし） |
| `mise run check-full` | Pass（workspace `members` 変更の整合、exit 0） |
| `*-msrv` 系 | NotRun（ユーザー指定により未実行） |

補助証拠（canonical = 上記 mise task）: `cargo clippy
-p synvelis-automation --all-targets` warning 0、
`cargo test -p synvelis-automation` 82 passed。

## 受入条件 → case 対応表

### AC-1 YAML Parser の Unknown/Tag/Alias/implicit 型/入力予算違反を Golden で検出

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| YAML-G01 anchor/alias/tag reject | `&`/`*`/`!` で fail-closed error | 各構文で parse error | Pass | `tests/yaml_subset.rs::rejects_anchor_alias_tag` |
| YAML-G02 flow collection reject | `{`/`[` で reject | parse error | Pass | `rejects_flow_collections` |
| YAML-G03 block scalar/merge reject | `|`/`>`/`<<` で reject | parse error | Pass | `rejects_block_scalars_and_merge` |
| YAML-G04 implicit 型制限 | `yes`/`on`/`0x..`/`1_000` 等の非 canonical・非対応綴りを reject、真の bool/null/int/float のみ受理 | 受理/reject が仕様どおり | Pass | `implicit_types_are_restricted`・`rejects_noncanonical_numbers` |
| YAML-G05 multi-doc/directive reject | 2 個目の `---`・`%YAML` directive で reject | parse error | Pass | `rejects_multidoc_and_directives` |
| YAML-G06 duplicate key/indent reject | 同一 map の重複 key・不正 indent で reject | parse error | Pass | `rejects_duplicate_keys_unknown_indent` |
| YAML-G07 予算違反 reject | >1MiB・depth>64・node>200k・scalar>64KiB で reject | parse error | Pass | `rejects_oversized_and_deep` |
| YAML-G08 block subset 受理 | block map/seq・quoted scalar・comment・inline seq/map 形を受理 | 期待 tree | Pass | `accepts_block_subset`・`quoted_scalars`・`inline_seq_map_forms` |
| YAML-G09 review regression（UTF-8/comment/tab/`...`） | single-quoted 非 ASCII の文字化けなし・inline comment が key/seq item/value で一貫 strip・tab indentation reject・`...` end marker 受理 | 期待 tree/error | Pass | `single_quoted_preserves_utf8`・`comments_strip_everywhere`・`rejects_tab_indentation`・`end_of_document_marker` |

### AC-2 YAML と外部 Script Builder が生成した同じ意味の Definition を同じ PlanCompiler で検証

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| SRC-01 source 非依存 compiler | JSON tagged-expr 経路の decode → 同一 `PlanCompiler` で compile 可能 | 同一 compile 経路・同一 plan 形状 | Pass（本 slice 範囲: compiler が source 非依存であることの固定） | `tests/compiler.rs::plan_compiler_is_source_agnostic`・`tests/decode.rs::json_decode_accepts_tagged_expr_form` |
| SRC-02 Builder 生成 Definition との同一検証 | Luau/Rhai Builder（#273）生成物での実証 | — | Deferred（Builder は #273） | — |

### AC-3 Definition Version/Plan Digest/Capability Version/Schema/ID の決定的 Golden と安全なエラー

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| GOLD-01 definition digest golden | canonical definition の SHA-256 = `1c1a411fb54370886dcdc2307dfcf4110e134178167159898b2f26156e004bae` | 一致 | Pass | `tests/determinism.rs::basic_workflow_golden_digest` |
| GOLD-02 plan digest golden | canonical plan の SHA-256 = `6779664f07bc06672487d6f9603e078ba25af3c214dae22d340ecead16b1492d` | 一致 | Pass | 同上 |
| GOLD-03 step order/effect golden | async/branch/loop の `step_order`・effect bound が決定的 | 期待順序・union | Pass | `async_workflow_golden_step_order`・`branch_workflow_golden_step_order_and_effects`・`loop_workflow_golden_budgets` |
| GOLD-04 再 compile 決定性 | 同一入力の反復 compile で digest 不変 | 一致 | Pass | `same_inputs_same_plan_across_runs`・`compile_is_deterministic` |
| GOLD-05 入力値非依存 plan | input 実値の違いが plan 構造に影響しない | plan digest/構造不変 | Pass | `plan_is_input_value_independent` |
| ERR-01 unsupported schema version | `schema_version != 1` で reject | `UnsupportedSchemaVersion` | Pass | `unsupported_schema_version_rejected`・`definition_schema_version_constant_is_one` |
| ERR-02 unknown field | 全 level の未知 field で reject | decode error | Pass | `unknown_fields_rejected_at_every_level` |
| ERR-03 encode/decode roundtrip | definition/plan の JSON roundtrip が canonical | 一致 | Pass | `encode_decode_roundtrip`・`canonical_json_is_key_order_independent`・`canonical_json_sorts_keys_and_is_compact`・`digest_is_lowercase_sha256_hex`・`definition_digest_stable_across_key_order`・`definition_digest_changes_with_definition`・`definition_roundtrip_via_json` |
| ERR-04 capability exact version | `id@major.minor` ref・exact `(id,version)` 解決 | 期待 ref 文字列 | Pass | `capability_refs_are_exact_versions`・`unsupported_capability_version_rejected` |
| ERR-05 ID/identifier 検証 | 不正 identifier・duplicate step id・duplicate input/output 名で reject | definition error | Pass | `invalid_identifiers_rejected`・`duplicate_step_ids_rejected` |
| ERR-06 plan schema version | `ExecutablePlan.schema_version == 1` | 一致 | Pass | `plan_schema_version_is_one` |

### AC-4 逐次/分岐/有界 ForEach/AtomicMacro/Async Job/Wait が型付き AST・Step Output Schema で結合、欠損 Output/将来生成 ID/Target 違い/循環を Compile Reject

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| STEP-01 基本逐次 compile | query→invoke→emit-result が compile 成功 | plan 生成・digest 有 | Pass | `basic_workflow_compiles` |
| STEP-02 async+wait compile | async-submit→await-job(job typed) が compile 成功 | plan 生成 | Pass | `async_workflow_compiles` |
| STEP-03 未知 step 参照 | `$steps.unknown` で reject | `UnknownStepId` | Pass | `unknown_step_reference_rejected` |
| STEP-04 前方参照/循環 | 未来 step・自己参照で reject | `ForwardStepReference` | Pass | `forward_step_reference_rejected` |
| STEP-05 branch scope | branch 内 step は外から参照不可 | reject | Pass | `branch_local_step_not_visible_outside` |
| STEP-06 step output field | `$steps.<id>.<field>` が宣言 output schema と照合 | 型一致で受理・不一致で reject | Pass | `step_output_field_projection_checked`・`output_type_mismatch_rejected` |
| STEP-07 control step output | If/ForEach 等の control step を `$steps` 参照で reject | reject | Pass | `control_step_output_rejected` |
| STEP-08 未知/未宣言 input | 未宣言 input 参照・必須 input 欠落・余分 input で reject | `UnknownInput`/definition error | Pass | `unknown_input_rejected`・`missing_required_input_rejected`・`unexpected_input_rejected`・`input_schema_violation_rejected` |
| STEP-09 await job 型 | 非 Job 型を await-job に渡すと reject | `JobRefTypeMismatch` | Pass | `await_job_requires_job_typed_value` |
| STEP-10 AtomicMacro | 未定義 macro 参照は fail-closed reject（#263 までは常に reject） | `UndefinedMacro` | Pass（本 slice の契約: 参照予約+fail-closed） | `undefined_macro_rejected` |

### AC-5 Limit 超過・Availability Planned/Retired・Unknown Capability/Effect Bounds を Fail-closed、Branch Effect を過小申告しない

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BUD-01 step/depth/iteration 過小申告 | worst-case > 宣言 limit で `BudgetUnderestimated` | reject | Pass | `step_budget_underestimated_rejected`・`depth_budget_underestimated_rejected`・`iteration_budget_underestimated_rejected` |
| BUD-02 ceiling 超過 | 宣言 limit > compiler ceiling で `BudgetExceeded` | reject | Pass | `declared_budget_above_ceiling_rejected` |
| BUD-03 loop 乗算 | bounded loop の body cost が iteration 数で乗算 | 期待 cost | Pass | `loop_workflow_scales_budgets`・`loop_workflow_golden_budgets` |
| BUD-04 effect union | branch 両側の effect/data-access が plan bound に union | 過小申告なし | Pass | `branch_workflow_unions_effects`・`branch_workflow_golden_step_order_and_effects` |
| BUD-05 effect ceiling | 宣言 effect/data-access bound 超過で reject | reject | Pass | `effect_ceiling_rejected` |
| AVAIL-01 availability | `planned`/`retired`/`disabled` capability で reject | reject | Pass | `planned_retired_disabled_rejected` |
| AVAIL-02 unknown capability | registry に無い capability で reject | `UnknownCapability` 系 | Pass | `unknown_capability_rejected` |
| DOM-01 domain/execution mismatch | operation domain・execution contract 不一致で reject | reject | Pass | `domain_mismatch_rejected`・`execution_mismatch_rejected` |
| SCH-01 args/output schema | capability input args の schema violation・emit output 型不一致で reject | `SchemaViolation`/`OutputTypeMismatch` | Pass | `args_schema_violation_rejected`・`output_type_mismatch_rejected` |
| TGT-01 target 契約 | target 必須で未指定・非対応 target・kind 不一致・暗黙 target 形で reject | reject | Pass | `missing_target_rejected`・`unexpected_target_rejected`・`target_kind_mismatch_rejected`・`implicit_target_forms_rejected` |

### AC-6 Definition に principal/grant/approval/OS credentials・自由な Shell/Eval が書けず、静的検証だけで実行権限を得られない

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| SEC-01 authority field 非表現 | `principal`/`grant`/`approval`/`credential`/`shell`/`eval` 等は variant/field として存在せず、書けば unknown field reject | decode reject・plan に authority field なし | Pass | `forbidden_authority_fields_have_no_representations`・`plan_has_no_authority_fields` |
| SEC-02 式の表現力上限 | function call/env/eval/script variant が AST に存在しない | `$` 参照 grammar 外は reject | Pass | `json_decode_rejects_bare_scalars_as_exprs`・frontend `lower_reference` reject 系（`tests/compiler.rs` 参照系 case に包含） |

### AC-7 Headless で同 Definition+Input+Registry Snapshot → 同 Plan Digest/Step Order/Warnings、Dry Run は非 Project Job・外部副作用まで成功保証しない

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| DET-01 deterministic triple | 同一 triple で同一 digest/step order/warnings | 一致（GOLD-04 と同一実測） | Pass | `same_inputs_same_plan_across_runs` |
| DET-02 opaque job warning | output schema 未解決の await-job は `OpaqueJobResult` warning（成功保証しない） | warning 生成 | Pass | `async_workflow_compiles`（warning 経路を含む fixture） |
| DET-03 Dry Run/実 Job/OS/CLI | runtime 実行・実 Job・外部副作用の成功保証 | — | NotRun（#265/#267 別 Gate） | — |

## emit-result / condition 系（AC-4 の補完）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| EMIT-01 undeclared output | 宣言外 output の emit で reject | reject | Pass | `undeclared_output_rejected` |
| EMIT-02 duplicate emit | 同一 output の重複 emit で reject | reject | Pass | `duplicate_output_emit_rejected` |
| EMIT-03 missing emit | declared output の emit 欠落で reject | reject | Pass | `missing_output_emit_rejected` |
| EMIT-04 nested emit | branch/loop 内の emit-result で reject | `EmitResultNotTopLevel` | Pass | `emit_inside_branch_rejected` |
| COND-01 condition 型 | 非 bool 条件・順序比較の非数値 operand・空 and/or で reject | reject | Pass | `incomparable_condition_operands_rejected`・`ordering_condition_requires_numeric`・`empty_and_or_conditions_rejected` |
| COND-02 items 型 | for-each の items が非 array で reject | reject | Pass | `non_array_items_rejected` |
| COND-03 item scope | loop 外の `$item` 参照で reject | `ItemOutOfScope` | Pass | `item_reference_outside_loop_rejected` |

## 集計

- Pass: 82 test（`mise run test synvelis-automation` 実測）＋
  fmt-check/check/lint/check-headless-deps/check-full の
  exit 0。
- NotRun: `*-msrv` 系（ユーザー指定）、Dry Run・実
  Job/Recovery/OS/CLI 結合（#265/#267）。
- Deferred: Luau/Rhai Builder 同一検証（#273）、AtomicMacro
  定義/展開（#263）、live Gateway adapter（統合 slice）。
- Fail: なし。
