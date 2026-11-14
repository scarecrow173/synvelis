# Verification — synvelis#278 CB-01 headless スライス

Date: 2026-11-14. Canonical 検証は `mise run` 経由。

## 実行結果（実測）

| コマンド | OS | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | Windows 11 | Pass（exit 0） |
| `mise run check synvelis-project` | Windows 11 | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-project` | Windows 11 | Pass（exit 0、clippy `-D warnings`） |
| `mise run test synvelis-project` | Windows 11 | Pass（exit 0 — 全 test suite green。`cb01_variant.rs` 17 件 + `variant::engine` unit 1 件を含む） |
| `mise run check <downstream>` | Windows 11 | synvelis-project 直接依存クレート（synvelis-app / synvelis-gateway / synvelis-workspace / synvelis-player / synvelis-render / synvelis-library / synvelis-recording / mcp-poc / recording-poc）— 結果は PR 本文に記録 |

## 補助証拠（canonical = mise）

- `cargo fmt --all` 適用後 `fmt-check` green。
- lint で検出・修正した点: wildcard import、`large_enum_variant`
  （`VariantOp::Adopt::after` を `Box<Project>` 化）、
  `too_many_lines`（`adopt_branch` を guard/candidate/freeze
  helper に分割、`scope_objects` を per-kind helper に分割）、
  `struct_excessive_bools`（`StripDomainMask` を `StripField`
  enum 集合へ変更）、`clone_on_copy`・`map_unwrap_or`・
  `collapsible_if`・`never_loop`・`needless_pass_by_value`・
  `trivially_copy_pass_by_ref`・`unnecessary_wraps`（migration
  step は sibling 慣例の `#[allow]` 付き identity projection）。

## NotRun

- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 / 実 GUI・実
  OS・実 Plugin・実 RT レーン — タスク指定により対象外。
- docs index 再生成・`doc-status` lint — doc-maintenance 手順で
  実施（結果は PR 本文に記録）。
