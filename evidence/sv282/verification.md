# Verification — synvelis#282 CB-05 FX Processing Range headless スライス

Date: 2026-11-14. Canonical 検証は `mise run` 経由。
OS: Windows 11。Toolchain: mise 管理 Rust（repo pin、edition 2024 / MSRV 1.89）。

## 実行結果（実測）

| コマンド | OS | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | Windows 11 | Pass（exit 0） |
| `mise run check synvelis-project` | Windows 11 | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-project` | Windows 11 | Pass（exit 0、clippy `-D warnings`） |
| `mise run test synvelis-project` | Windows 11 | Pass（exit 0 — 全 test suite green。`cb05_fx_range.rs` 13 件を含む。`cb01_variant.rs`・`cb02_audio_take.rs`・`cb03_midi_clip.rs`・`cb04_instrument_slot.rs` の回帰なし） |
| `mise run check synvelis-workspace` | Windows 11 | Pass（exit 0 — 直接依存の build 整合） |
| `mise run check synvelis-render` | Windows 11 | Pass（exit 0 — leased crate の source 互換確認） |

## 補助証拠（canonical = mise）

- `cargo test --locked -p synvelis-project --test
  cb05_fx_range` で fixture を個別確認後、canonical
  `mise run test` で全件 green を確認（13/13 pass）。
- lint で検出・修正した点: test 側 `similar_names`
  （pin 変数 rename）、`filter_next`（`find` 化）、
  `map_unwrap_or`（`map_or_else` 化）、`doc_markdown`
  （backticks）、`too_many_lines`（`rich_candidate_edits`
  を `scoped_base_and_param_edits`/`scoped_automation_edit`/
  `scoped_modulation_edit`/`insert_mid_range_processor` に、
  `cb_fx_04` の op 一覧を `external_wiring_ops` に、
  `cb_fx_06` の assessment/plugin-gate を
  `assert_range_state_assessments`/`assert_plugin_gate_adopt`
  に分割）、`let_and_return`、`items_after_statements`
  （`NoPlugins`/`YesPlugins` を module level へ）。
  `#[allow]` は不使用。
- 実装上の要点: `adapter.rs` の instrument 専用
  processor-control checker を `check_processor_
  control_op_targets` として共有化し `InstrumentSlot`/
  `ProcessingRange` 双方へ dispatch — `PreEditIds` に
  control entity 4 集合を追加し `check_created_within_
  scope` が新規 entity の scope 内包を fail-closed 検証。
  `ProcessingRangeAdapter` は `end` = chain tail を
  `validate_scope` で強制し、`InsertProcessor` のみを
  post-edit scope 再導出で受理。boundary dependency は
  `boundary_dependencies` が型付き inventory するが
  scope objects には含めず、配線 op は全て
  `Unsupported`。
- fixture は `CounterIdGenerator::new(1)` + recording 注入
  controller は `new(10_000)`（CB-04 先例と同規約 —
  注入 runtime id が watermark を超えない）。

## NotRun

- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 — タスク
  指定により対象外（daily CI の責務）。
- 実 OS provider・実 Plugin host・外部 Plugin GUI・実 RT・
  実 Render pipeline・実デバイス入力 pipeline・workspace/
  GPUI projection — headless fixture のみ。Fake/Domain
  Oracle の PASS を実機 PASS に昇格しない（後続 Issue
  #286/#287/#288 系・CB-FX-06 実機 lane で記録）。
- `doc-driven-dev-graph` route — spec/design/plan/task 相当の
  内容はリース済み `docs/impl/{ir,exp}`・
  `docs/test-specs/cb-05-fx-range-adapter.md` に集約
  （リース外 docs への書き込みは行っていない）。
