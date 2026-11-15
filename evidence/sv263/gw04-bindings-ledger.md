# AUTX-04-F02 GW-04 bindings scratch-ID/rewrite seam — Case Ledger（synvelis#263 gateway slice）

Date: 2026-11-15。Slice: `synvelis-gateway` の prepare engine に
AUTX-04 `bindings` の scratch-ID/rewrite seam を実装
（`crates/synvelis-gateway/src/prepare_engine.rs`）し、
headless integration test（`crates/synvelis-gateway/tests/
gw04_bindings.rs`）で契約を固定した。**本 slice は headless
の prepare→proposal→apply→undo 経路のみ**を対象とし、実
OS/実 Plugin/実 RT 経路・`*-msrv` レーンは NotRun である。
headless の Pass を runtime/実機の Pass に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/263
（OPEN・`Refs` のみ・Close しない — Close 判定は Coordinator）。

先行 slice: AUTX-04 本体（`evidence/sv263/
autx-04-atomic-macro-ledger.md` — compile 側契約と typed
fallback）、F01（`evidence/sv432/composite-forward-ref-
ledger.md` — `Composite::validate` 逐次 dry-run、commit
`737b0f2`）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `scarecrow173/sv263-gw04-bindings`（worktree `sv263-gw04-bindings`。base = origin/main `737b0f2`） |
| OS / CPU / FS | Windows 11 / NTFS（G: worktree） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89（`*-msrv` レーンは対象外） |
| Build | mbx managed target（worktree `target` リンク先。`CARGO_TARGET_DIR` 未設定） |
| Project schema | 変更なし — prepare/lowering 意味論のみ |

## 判定語彙

- **Pass**: 当該 SHA で検証が exit 0 で完了し、対象
  test/assertion が green だったことを実測した。
- **NotRun**: 本 slice で実行していない項目（後続 slice・
  MSRV lane・実機/実 OS 結合）。
- **Deferred**: 別 Issue/別リースが Owner の項目
  （#264 AUTX-05 workflow DSL、#265 AUTX-06 coordinator、
  automation compile→gateway prepare の結合 e2e）。

## 実行した正規検証

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0。`cargo fmt --all -- --check`） |
| `mise run check synvelis-gateway` | Pass（exit 0。`cargo check --locked --all-targets`） |
| `mise run lint synvelis-gateway` | Pass（exit 0。clippy `-D warnings`・`--all-targets`。初回 `type_complexity`/`needless_pass_by_value`/`manual_let_else` を `Lowered` 型 alias・slice 参照化・let-else で解消後に再実行） |
| `mise run test synvelis-gateway` | Pass（`cargo test --locked`。lib 13・`gw04_bindings` 12・既存 26 test binaries 全件 0 failure） |
| `mise run test-rt` | Pass（`cargo test --workspace --locked rt_`。workspace 全 crate の rt_ contract test） |
| `prek run --all-files`（commit 時 hook と同等） | Pass（commit 時に実施。結果は commit 履歴参照） |

補助証拠（canonical = 上記 mise task）: `cargo test -p
synvelis-gateway --test gw04_bindings` の直接実行でも同結果
（12/12 Pass）。

## 受入条件 → case 対応表

Test spec: `docs/test-specs/autx-04-f02-gw04-bindings.md`
（AUTX4F2-TS-R01–R09）。実装記録:
`docs/impl/ir/autx-04-f02-gw04-bindings.md`。試行記録:
`docs/impl/exp/autx-04-f02-gw04-bindings.md`。

### AUTX4F2-TS-R01 declaration shape

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BND-F02-G01 非 object/未知 field/欠落 field | typed reject | `SchemaViolation`/`UnknownResourceKind` | Pass | `rejects_malformed_binding_declarations`（unit: `batch_bindings_reject_malformed_declarations`） |
| BND-F02-G02 member 範囲外・非整数・負・小数 | typed reject | 同上 | Pass | 同上（member out of range/negative/fractional case） |
| BND-F02-G03 未知 kind・重複 `(member,kind)` | typed reject | 同上 | Pass | 同上（unknown kind・duplicate case） |

### AUTX4F2-TS-R02 placeholder identity

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BND-F02-G04 未宣言 slot の placeholder | typed reject | `SchemaViolation` | Pass | `rejects_undeclared_and_kind_mismatched_placeholders`（undeclared slot case） |
| BND-F02-G05 kind-tag と宣言 kind の不一致 | typed reject | `SchemaViolation` | Pass | 同上（kind mismatch case） |
| BND-F02-G06 object key の placeholder | typed reject | `SchemaViolation` | Pass | `rejects_placeholder_object_keys_and_nested_batches` |

### AUTX4F2-TS-R03 resolution order

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BND-F02-G07 forward/self/backward 参照 | lowering で typed reject | `SchemaViolation` | Pass | `rejects_backward_self_and_premature_references`（3 case）・unit `batch_bindings_reject_premature_and_undeclared_placeholders` |
| BND-F02-G09 nested batch member | typed reject | `SchemaViolation` | Pass | `rejects_placeholder_object_keys_and_nested_batches`（nested batch case） |

### AUTX4F2-TS-R04 exactly-one creation

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BND-F02-G08 producer apply 失敗 → 未解決 placeholder | `PreparationRejected`（"could not be resolved"） | reason 含有を assert | Pass | `rejects_unresolvable_producer_output` |
| BND-F02-G10 created 0（kind 不一致の producer） | `PreparationRejected`（"created 0"） | reason 含有を assert | Pass | `rejects_wrong_created_arity`（第 1 case）・unit `batch_bindings_reject_producer_wrong_kind_arity` |
| BND-F02-G11 created 2（detach-source が source+notes を mint） | `PreparationRejected`（"created 2"） | reason 含有を assert | Pass | `rejects_wrong_created_arity`（第 2 case） |

### AUTX4F2-TS-R05 deterministic id series

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BND-F02-G12 create→参照の決定的 ID | watermark シードで track=1000/lane=1001/clip=1002 | candidate で実測 | Pass | `agent_create_then_reference_commits_as_one_undo_unit`・`chained_bindings_resolve_across_three_members`・unit `batch_bindings_resolve_create_then_reference` |

### AUTX4F2-TS-R06 scratch-fed provisional oracle

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BND-F02-G13 agent create-then-reference | scratch oracle で `Known` 解決 → prepare→apply→1 undo | deterministic id で commit 成功・undo で両 resource 消失 | Pass | `agent_create_then_reference_commits_as_one_undo_unit` |
| BND-F02-G14 create axis なし | `Denied` | `GatewayError::Denied`・proposal 0 件 | Pass | `agent_without_create_axis_still_denies` |
| BND-F02-G15 `include_future` なし | `Denied` | `GatewayError::Denied` | Pass | `agent_scope_without_future_membership_denies` |
| BND-F02-G16 human caller | grant なしで seam が動作 | Composite retained・candidate に track 1000 | Pass | `human_caller_bindings_batch_prepares` |

### AUTX4F2-TS-R07 domain failure ordering

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BND-F02-G19 member domain 失敗は lowering で raise しない | emitted `Composite` の `op.validate` が provisional gate 後で再現 | `lower_bound_batch` は `is_ok()` で失敗を呑み `ops.push` のみ（prepare_engine.rs trial-clone 節）。placeholder 非依存の domain 失敗を直接打つ専用 test は未整備 — review P3 として記録 | Pass（設計・コード経路で確認。専用 test は Follow-up 候補） | `rejects_unresolvable_producer_output`（placeholder 系 reject は lowering で発生する対照系）・`prepare.rs` 既存 batch domain reject 系 |

### AUTX4F2-TS-R08 single transaction contract

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BND-F02-G17 bindings 非空 batch | flatten `Composite`・1 proposal/1 commit/1 undo | `ProjectOp::Composite{ops:2}`・`accept_apply`→`ProjectController::apply` 1 回・`undo` で両消失 | Pass | `agent_create_then_reference_commits_as_one_undo_unit` |

### AUTX4F2-TS-R09 legacy lowering

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BND-F02-G18 bindings 空/非所持 | legacy lowering・scratch なし | Composite retained・実 ID（track 3）のみ参照 | Pass | `empty_or_absent_bindings_keep_legacy_lowering`・unit `batch_without_bindings_still_lowers`・既存 `tests/prepare.rs` batch 系 18 件全 Pass |

## NotRun / Deferred

- 実 OS・実 Plugin・実 RT・MCP live 経路: NotRun（headless slice）。
- `*-msrv`（Rust 1.89）lane: NotRun（CI toolchain 1.99.0 のみ実施）。
- automation `MacroCompiler` 出力をそのまま gateway prepare へ
  流す compile→prepare 結合 e2e: Deferred（#264/#265 系の
  後続 slice。wire 契約は `macros.rs`/`schema.rs` と本 ledger の
  placeholder 形式で固定済み）。
- `draw_raw_id` 予約窓を跨ぐ batch: F01 から継続の既知制限
  （fail-closed 既定。exp E3 参照）。
