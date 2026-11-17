# Verification — synvelis#283 CB-06 Track/Bus Strip headless スライス

Date: 2026-11-14. Canonical 検証は `mise run` 経由。
OS: Windows 11。Toolchain: mise 管理 Rust 1.99.1
（repo pin 1.99.0 系、edition 2024 / MSRV 1.89）。

## 実行結果（実測）

| コマンド | OS | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | Windows 11 | Pass（exit 0） |
| `mise run check synvelis-project` | Windows 11 | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-project` | Windows 11 | Pass（exit 0、clippy `-D warnings`） |
| `mise run test synvelis-project` | Windows 11 | Pass（exit 0 — 全 test suite green。`cb06_strip_fields.rs` 19 件を含む。`cb04_instrument_slot.rs` 8 件・`cb05_fx_range.rs` 13 件の回帰なし） |
| `mise run check synvelis-workspace` | Windows 11 | Pass（exit 0 — 直接依存の build 整合） |
| `mise run check synvelis-render` | Windows 11 | Pass（exit 0 — leased crate の source 互換確認） |

## 補助証拠（canonical = mise）

- lint で検出・修正した点: test 側 `map_unwrap_or`
  （`map_or_else` 化）、`needless_borrow`（`h.controller
  .live()` の参照渡し整理）、`too_many_lines`
  （`strip_fixture` を `track_strip_chain`/`return_bus`/
  `key_strip`/`boundary_wiring` へ分割、mask/採用 edit 列を
  `scoped_base_and_param_edits`/`scoped_automation_edit`/
  `scoped_modulation_edit`/`scoped_processor_modulation_edit`/
  `insert_mid_chain_processor`/`declared_mute_surface_edits`/
  `mute_automation_lane`/`rich_candidate_edits` の共有
  helper と `mask_external_reject_ops`/
  `undeclared_field_reject_ops`/`boundary_wiring_reject_ops`/
  `shared_context_reject_ops` の reject-op builder へ、
  adopt assertion を `assert_adopted_mask_state`/
  `assert_shared_state_untouched`/`assert_adopt_undo_redo`/
  `assert_mask_capabilities` へ分割）、`items_after_
  statements`（`NoPlugins`/`YesPlugins` を module level
  へ）。`#[allow]` は不使用。
- `cargo fmt --all` が `cb05_fx_range.rs` に formatting-only
  の差分を生成（rustfmt 1.99.1 — repo の canonical
  formatter 出力であり fmt-check 通過に必要）。意味変更
  なし。
- 実装上の要点: `adapter.rs` に `is_strip_fields_op`
  predicate と `check_strip_fields_op_targets` を追加し、
  `SetStripMute`/`SetStripSolo`/`InsertProcessor` を mask
  照合、control closure op を `scope_objects` membership
  で fail-closed 検証。`variant/strip.rs` の
  `StripFieldsAdapter` が `validate_scope`/`fork_candidate`/
  `edit_candidate`/`preflight_adopt`（recording gate は
  CB-05 と同一 `strip_recording_gate_lanes` 共有）/
  `capabilities`/`boundary_dependencies`（strip-affiliated
  `SemanticPortId` endpoint を持つ connection の typed
  inventory）を実装。ownership 導出は CB-01 の
  `scope_objects`/`strip_fields_objects` を再利用し
  adapter 側で再実装しない。
- fixture は `CounterIdGenerator::new(1)` + recording 注入
  controller は `new(10_000)`（CB-04/05 先例と同規約 —
  注入 runtime id が watermark を超えない）。
- review-protocol: 2 独立 reviewer が `adapter.rs`/
  `scope.rs`/`strip.rs`/`ops.rs`/`apply/entity.rs` の
  confinement 経路を adversarial に検査。検出・修正:
  - modulation assignment の effect-target confinement 欠落
    — 共有 `control_closure` の ANY-endpoint 規則により
    scope 所有 source から mask 外 target への既存
    assignment が scope 所有となり `Set`/`Remove`
    `ModulationAssignment` が通過し得た。`strip_fields_
    objects` に StripFields 固有の post-filter を追加し
    effect target/depth_source の所有を要求（共有 closure
    は CB-04/05 契約維持のため不変）。回帰 test:
    `cb_strip_02_assignment_effect_target_gates_ownership`・
    `cb_strip_02_processor_assignment_effect_gates_ownership`。
    post-filter は `depth_source`（第 2 の parameter endpoint）
    の scope 所有も要求 — 回帰 test:
    `cb_strip_02_assignment_depth_source_gates_ownership`。
  - live chain reorder の staleness 欠落 — `ProcessorChain`
    が scope object でなかったため live `ReorderProcessor`
    が scope digest を動かさず、adopt の positional merge が
    未 author の順序を生成し得た。`Inserts` mask 時に
    `ObjectKind::ProcessorChain` を scope 所有へ追加。
    回帰 test:
    `cb_strip_04_live_chain_reorder_stale_scope_rejects_adopt`。
  - P3（doc/comment）: `assess_insert_state` の mask 依存
    記述・missing-plugin の adopt preflight 文言を修正。
  - 共有 `strip_channel_field`（`ParameterRole`→`StripField`）
    を `scope.rs` に集約し `adapter.rs` と単一正本化。

## 追補 slice 検証（gateway commit-gate exercise — 2026-11-17）

Date: 2026-11-17。Canonical 検証は `mise run` 経由。
OS: Windows 11。Toolchain: mise 管理 Rust（repo pin 系）。
対象: `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
（新規 2 test）+ cb-06 doc chain 追従 + 本 evidence 追記。
production code 変更なし。

| コマンド | OS | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | Windows 11 | Pass（exit 0 — 新規 file を `cargo fmt` 適用後） |
| `mise run check synvelis-gateway` | Windows 11 | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-gateway` | Windows 11 | Pass（exit 0、clippy `-D warnings`。初回 `assert_is_empty` 検出 → `len()==0` 比較へ修正） |
| `mise run test synvelis-gateway` | Windows 11 | Pass（exit 0 — 全 suite green。`cb06_strip_gateway.rs` 2 件: `cb06_strip_fields_gateway_commit_gate_lifecycle`・`cb06_strip_fields_scope_boundary_and_capabilities`。既存 cb10/cb11 他 test の回帰なし） |
| `mise run check-headless-deps` | Windows 11 | Pass（exit 0 — 12 headless crates に GUI framework deps なし） |
| `mise run check synvelis-project` | Windows 11 | Pass（exit 0 — 非回帰確認、source 変更なし） |
| `mise exec -- prek run --all-files` | Windows 11 | Pass（exit 0 — trailing whitespace/eof/yaml/toml/utf-8 BOM/merge marker/actionlint 全 Pass） |

## 補助証拠（canonical = mise）

- `mise exec -- cargo test --locked -p synvelis-gateway
  --test cb06_strip_gateway` — 2 test Pass（focused 実行、
  canonical は上記 `mise run test synvelis-gateway`）。
- 実装上の要点: fixture は `ProjectOp::AddTrackStrip` +
  `ProjectOp::InsertProcessor` を `ProjectController` で適用
  した test-local project（head revision 2）を
  `DirectoryBundleStore` + `MemorySnapshotSource` に pin —
  `tests/rig/mod.rs` の共有 fixture は非変更。
- wire 表面の実測: `lower_for` capability 表に strip-domain
  inner op への lowering は存在しない — candidate edit は
  ordinary op 3 種で `variant-unsupported` typed reject、
  stale `expected_scope_digest`/`expected_branch_revision`
  が `variant-stale-scope`/`variant-stale-branch-revision`
  で reject されることを確認。positive mask-gated edit は
  project 軸の責務（本経路では非表現 — spec の Deferred
  Design Concerns に記録）。
- adopt は `ApproveToApply` agent grant + human approval
  receipt 経路（cb10/cb11 先例）— provenance に
  `outcome=="applied"`・`project.committed==true`・
  `undo_steps==1`・`objects==0`・
  `runtime.state.state=="not-requested"`（VCS-off headless）。

## NotRun

- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 — タスク
  指定により対象外（daily CI の責務）。
- 実 OS provider・実 Plugin host・外部 Plugin GUI・実 RT・
  実 Render pipeline・実デバイス入力 pipeline・workspace/
  GPUI projection — headless fixture のみ。Fake/Domain
  Oracle の PASS を実機 PASS に昇格しない（後続 Issue
  #286/#287/#288 系・CB-STRIP-06 実機 lane で記録）。
- wire 表面の strip-domain op capability（`SetStripMute`/
  `SetParameterBase`/`InsertProcessor` 等を lower する
  gateway capability）— 本 slice 範囲外。必要時に別
  dispatch で op 表面設計とあわせて検討。
- `doc-driven-dev-graph` route — spec/design/plan/task 相当の
  内容はリース済み `docs/impl/{ir,exp}`・
  `docs/test-specs/cb-06-track-bus-adapter.md` に集約
  （リース外 docs への書き込みは行っていない）。

## 追補 — strip-domain inner op lowering（synvelis#548）

- `mise run test synvelis-gateway` — 全 suite Pass
  （`cb06_strip_gateway` 4 件: lifecycle・boundary/
  capabilities・mask-gated edit wire lowering・insert ops
  wire lowering。`tests/prepare.rs` 21 件: strip-domain
  ordinary prepare 3 件を含む）。
- wire 表面の実測（synvelis#548 後）: `lower_for` capability
  表に strip-domain op 5 件の lowering が存在し、狭 mask
  （channel 系）・全 mask（inserts 系）の両側で positive
  mask-gated edit が candidate payload に commit される
  ことを確認。mask 未宣言・scope 外・非 strip op は
  `variant-unsupported`/`variant-scope-violation` の
  typed reject を維持し revision を消費しない。
- 残る NotRun: routing op capability・scope entity
  removal・他 domain positive 化・実 PCM/RT/plugin・
  実 GUI lane（別 slice/別 gate）。
- 検証詳細: `evidence/sv548/verification.md`。
