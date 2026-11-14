# Verification — synvelis#280 CB-03 MIDI Clip/Source headless スライス

Date: 2026-11-14. Canonical 検証は `mise run` 経由。
OS: Windows 11。Toolchain: mise 管理 Rust（repo pin、edition 2024 / MSRV 1.89）。

## 実行結果（実測）

| コマンド | OS | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | Windows 11 | Pass（exit 0） |
| `mise run check synvelis-project` | Windows 11 | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-project` | Windows 11 | Pass（exit 0、clippy `-D warnings`） |
| `mise run test synvelis-project` | Windows 11 | Pass（exit 0 — 全 test suite green。45 test binary・`cb03_midi_clip.rs` 12 件を含む。`cb01_variant.rs`・`cb02_audio_take.rs` の回帰なし） |
| `mise run check synvelis-workspace` | Windows 11 | Pass（exit 0 — 直接依存の build 整合） |
| `mise run check synvelis-render` | Windows 11 | Pass（exit 0 — leased crate の source 互換確認） |

## 補助証拠（canonical = mise）

- `cargo test -p synvelis-project --test cb03_midi_clip`
  （mise exec 経由の反復実行）で fixture を個別に確認後、
  canonical `mise run test` で全件 green を確認。
- lint で検出・修正した点: `too_many_arguments`
  （`check_created_within_scope` の before 集合を
  `PreEditIds` struct に集約 — `#[allow]` 不使用）、
  test 側 `too_many_lines`（lifecycle fixture に既存慣例の
  `#[allow]` 付与 + delete+add を独立 test に分割）、
  `filter_next`/`let_and_return`/`unused_mut`/doc backticks/
  `non_snake_case`（`rCP`→`rcp`）。
- 実装上の要点: 共有 confined flow は `Operation::apply` を
  validate 無しで呼ぶため、`MidiClipAdapter::edit_candidate`
  が MIDI note op 4 種に限り `Operation::validate` を
  confinement 前に再実行し `reject_smf_backed` を candidate
  内でも enforce。`check_op_targets` に MIDI note op の
  `MidiSource`/`MidiNote` target 解決、`check_created_
  within_scope` に `MidiSource`/`MidiNote` 新規 entity 検査を
  追加し、confinement 規則は全 adapter で共有（drift なし）。

## NotRun

- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 — タスク
  指定により対象外（daily CI の責務）。
- 実 OS provider・実 Plugin host・実 RT・実 Render pipeline・
  実 MIDI device/SMF capture・実デバイス入力 pipeline・
  workspace/GPUI projection — headless fixture のみ。
  Fake/Domain Oracle の PASS を実機 PASS に昇格しない
  （後続 Issue #286/#287/#288 系で記録）。
- `doc-driven-dev-graph` route — spec/design/plan/task 相当の
  内容はリース済み `docs/impl/{ir,exp}`・
  `docs/test-specs/cb-03-midi-clip-adapter.md` に集約
  （リース外 docs への書き込みは行っていない）。
