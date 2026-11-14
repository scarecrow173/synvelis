# Verification — synvelis#279 CB-02 Audio Clip/Take/Comp headless スライス

Date: 2026-11-14. Canonical 検証は `mise run` 経由。
OS: Windows 11。Toolchain: mise 管理 Rust（repo pin、edition 2024 / MSRV 1.89）。

## 実行結果（実測）

| コマンド | OS | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | Windows 11 | Pass（exit 0） |
| `mise run check synvelis-project` | Windows 11 | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-project` | Windows 11 | Pass（exit 0、clippy `-D warnings`） |
| `mise run test synvelis-project` | Windows 11 | Pass（exit 0 — 全 test suite green。`cb01_variant.rs` 17 件・`cb02_audio_take.rs` 10 件・`rcd_04_vcs_stable_id.rs` 4 件を含む） |
| `mise run check synvelis-workspace` | Windows 11 | Pass（exit 0 — 直接依存の build 整合） |
| `mise run check synvelis-render` | Windows 11 | Pass（exit 0 — leased crate の source 互換確認） |

## 補助証拠（canonical = mise）

- `cargo test -p synvelis-project --test cb02_audio_take`（mise exec
  経由の反復実行）で fixture 失敗を個別に修正後、canonical
  `mise run test` で全件 green を確認。
- lint で検出・修正した点: `type_complexity`（`OpGuard`
  type alias 化）、`missing_panics_doc`（`capabilities()` に
  `# Panics` 節）、`match_same_arms`（comp group 解決 arm の
  統合）、`too_many_lines`（fixture/長い acceptance test に
  既存慣例の `#[allow]` 付与）、`similar_names`（`fxv2`→
  `fxv_alt`）、doc backticks。
- fixture 修正の要点: `CompCandidateRef::Take.source_offset`
  は絶対 source frame であり take の captured coverage
  （segment `source_range` union）内に存在する必要がある —
  `take_a1` の coverage は `[40000, 70000)` のため
  `source_offset: 40_000` を使用。region overlap は transition
  有無に関わらず禁止のため、resize 対象は Silence region
  （`fx.regions[2]`）を選択。transition overlap 変更時は
  近接 region の handle overhang（pre/post extra frames）が
  candidate window に収まる範囲に限定。

## NotRun

- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 — タスク
  指定により対象外（daily CI の責務）。
- 実 OS provider・実 Plugin host・実 RT・実 Render pipeline・
  実録音/Import pipeline・workspace/GPUI projection —
  headless fixture のみ。Fake/Domain Oracle の PASS を実機
  PASS に昇格しない（後続 Issue #287/#288 系で記録）。
- `doc-driven-dev-graph` route — 起動したが pre-existing の
  repo 全体 `artifact-graph` blocker（本タスク無関係の docs
  関係）で fail-closed。route 結果は本ファイルに記録し、
  spec/design/plan/task 相当の内容はリース済み
  `docs/impl/{ir,exp}`・`docs/test-specs` に集約した。

## doc-status / index

- `docs/impl/ir/cb-02-audio-clip-adapter.md`・
  `docs/impl/exp/cb-02-audio-clip-adapter.md`・
  `docs/test-specs/cb-02-audio-clip-adapter.md` を新設。
- 触れた dir の index（`docs/impl/ir/README.md`・
  `docs/impl/exp/README.md`・`docs/test-specs/README.md`）は
  `doc-maintenance`/`sync_impl_index` の `index apply` で
  再生成（手編集なし）。`doc-status` lint の結果は PR 本文に
  記録。
