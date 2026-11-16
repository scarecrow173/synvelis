# Verification — synvelis#293 RCP-04 semantic→selection integration slice

Date: 2026-11-16. Branch: `feat/293-semantic-selection-integration`
(base `origin/main` = `0b57299`)。

## 計測環境

| 項目 | 値 |
| --- | --- |
| OS | Windows 11 (Git Bash) |
| Toolchain | mise 管理 Rust（`mise run`/`mise exec` 経由、CI pin 1.99.x 系） |
| 検証入口 | `mise run` タスクのみ（cargo 直接実行は補助として rustfmt 事前確認に使用） |

## 実行結果（実測）

| コマンド | 結果 | 内容 |
| --- | --- | --- |
| `mise run fmt-check` | Pass (exit 0) | `cargo fmt --all -- --check` |
| `mise run check synvelis-workspace` | Pass (exit 0) | `cargo check --locked --all-targets` |
| `mise run lint synvelis-workspace` | Pass (exit 0) | `cargo clippy --locked --all-targets -p synvelis-workspace -- -D warnings`（test target 含む） |
| `mise run test synvelis-workspace` | Pass (exit 0) | `tests/rcp_selection_integration.rs` 10/10 green を含む全 workspace test |
| `mise run check synvelis-app` | Pass (exit 0) | workspace 公開 API 変更の app compile 非回帰 |
| `mise run check synvelis-render` | Pass (exit 0) | 消費する lookup/recipe/render_job 型との整合の非回帰 |

`tests/rcp_selection_integration.rs` の内訳（全 10 case green）:

- `rcp_ex01_missing_plugin_blocks_main_playback`
- `rcp_ex01_missing_plugin_exact_substitution_publishes`
- `rcp_ex01_main_playback_all_live_publishes`
- `rcp_ex01_offline_render_enforces_min_reproducibility`
- `rcp_ex02_topology_edge_kinds_preserved`
- `rcp_ex03_export_rejects_preview_only_artifact`
- `rcp_ex04_rename_norebuild_controlonly`
- `rcp_ex04_structural_edit_rebuilds_and_republishes`
- `rcp_ex05_stale_selection_never_publishes`
- `rcp_producer_fails_closed_on_bad_evidence`

lint 修正履歴: 初回 lint で `clippy::too_many_lines`（producer の
`build_edges`・test 2 関数）・`assert_is_empty`・
`match_wildcard_for_single_variants`・`unused_mut` 等を検出。
`build_edges` は `EdgeSink`/`chain_edges`/`connection_edges`/
`build_edges` に分割、長大 test は `PublishedFixture`/
`topology_project` fixture + ex04 2 分割で解消。`#[allow]` による
抑制は不使用。

## doc 検査

- `docs/specs|designs|plans|tasks|test-specs|impl/ir|impl/exp|
  discovery/sv293-semantic-selection-integration.md` と
  `docs/impl/exp/sv293-semantic-selection-integration-exp.jsonl`
  を新規作成（既存 `sv29x` 系構成を踏襲）。
- docs index（`docs/**/README.md`）は契約により未編集・未再生成
  （Coordinator が統合時に `doc-maintenance` で再生成）。

## merge 後再検証

`origin/main` `a98df54` を merge（conflict なし・`Cargo.lock`
差分は本 slice の dep 2 行のみ）した HEAD に対し、上記
`fmt-check` / `lint` / `test` / `check synvelis-app` /
`check synvelis-render` / `prek run --all-files` を全て再実行し
Pass（exit 0）。`rcp_selection_integration` は merge 後も
10/10 green。

## NotRun / Deferred（Case ID 付き・acceptance-ledger.md 参照）

- `RCP-EX-06` 実機分（PCM・実 Plugin・実 OS・実 RT の async
  nonRT prepare/no-alloc/seek/loop/tail/PDC/frame accuracy）:
  synvelis#296 RCP-GATE 行き。NotRun — headless green を実機
  Pass に換算しない。
- `SessionRoutingEngine`（`synvelis-app`）adapter 配線: lease 外。
  `RoutingEngine` レベルの headless oracle で代替。Deferred。
- BranchAudition/provenance 経路: #294 影響圏・本 slice 対象外。
  Deferred。
- 実 Plugin ABI（`scarecrow173/ruviform#630`）・実 signal/PCM
  clock（`scarecrow173/ruviform#628`）・仮想 OS endpoint
  （`scarecrow173/ruviform#629`）・実 Window/GPUI runner
  （`scarecrow173/ruviform#631`）・実 OS GUI（#114・#176）・
  実行/製品 gate（#23）: NotRun（別 gate）。
- `*-full`/`*-msrv`/3OS 横断/GUI lane（`test-ui`/`check-app-*`/
  `test-app-*`）: NotRun（Daily/手動 Full CI へ委譲）。
