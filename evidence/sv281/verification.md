# Verification — synvelis#281 CB-04 Instrument Slot headless スライス

Date: 2026-11-14. Canonical 検証は `mise run` 経由。
OS: Windows 11。Toolchain: mise 管理 Rust（repo pin、edition 2024 / MSRV 1.89）。

## 実行結果（実測）

| コマンド | OS | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | Windows 11 | Pass（exit 0） |
| `mise run check synvelis-project` | Windows 11 | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-project` | Windows 11 | Pass（exit 0、clippy `-D warnings`） |
| `mise run test synvelis-project` | Windows 11 | Pass（exit 0 — 全 test suite green。`cb04_instrument_slot.rs` 8 件を含む。`cb01_variant.rs`・`cb02_audio_take.rs`・`cb03_midi_clip.rs` の回帰なし） |
| `mise run check synvelis-workspace` | Windows 11 | Pass（exit 0 — 直接依存の build 整合） |
| `mise run check synvelis-render` | Windows 11 | Pass（exit 0 — leased crate の source 互換確認） |

## 補助証拠（canonical = mise）

- `cargo test --locked -p synvelis-project --test
  cb04_instrument_slot` で fixture を個別確認後、canonical
  `mise run test` で全件 green を確認（8/8 pass）。
- lint で検出・修正した点: `match_same_arms`
  （`adapter.rs` の instrument op dispatch — 共通 match arm
  に統合）、test 側 `too_many_lines`（fixture を
  `midi_clip_fixture`/`strip_fixture`/`add_target`/
  `rich_candidate_edits`/`assert_state_assessments`/
  `assert_surface_rejections` に分割）、`needless_pass_by_
  value`（`with_open_recording` を参照渡し化）、doc
  backticks（`plugin_state.rs`/`BitExact`/`BestEffort`）。
  `#[allow]` は不使用。
- 実装上の要点: 共有 confined flow（`edit_scoped_project_op`）
  は全 adapter で `check_op_targets`/`check_created_within_
  scope` を共有 — `PreEditIds` に control entity 4 集合を
  追加し、instrument scope のみ control op の target 解決を
  許可。他 scope（Fake/Audio/MIDI）では従来どおり
  `Unsupported` whitelist で reject され drift しない。
- fixture で `CounterIdGenerator::new(10_000)` を使用 —
  recording test が直接注入する runtime id（9001-9003）が
  live entity id watermark を超えないようにするため
  （`Snapshot(InvalidSnapshot)` 回帰を解消）。

## NotRun

- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 — タスク
  指定により対象外（daily CI の責務）。
- 実 OS provider・実 Plugin host・外部 Plugin GUI・実 RT・
  実 Render pipeline・実デバイス入力 pipeline・workspace/
  GPUI projection — headless fixture のみ。Fake/Domain
  Oracle の PASS を実機 PASS に昇格しない（後続 Issue
  #286/#287/#288 系・CB-INST-06 で記録）。
- `doc-driven-dev-graph` route — spec/design/plan/task 相当の
  内容はリース済み `docs/impl/{ir,exp}`・
  `docs/test-specs/cb-04-instrument-adapter.md` に集約
  （リース外 docs への書き込みは行っていない）。
