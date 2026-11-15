# Verification — synvelis#296 RCP-07 cross-gate fixture/ledger slice

Date: 2026-11-15. Branch: `test/rcp07-cross-gate`.
Base: `origin/main` merge 取り込み済み（merge commit
`6c6d1a5`、spec 起稿 tip `877a3b6` 以降の main を含む）。
本ファイルは実測のみを記録する。未実行は NotRun と明記し、
推測で Pass にしない。

## 計測環境

| 項目 | 値 |
| --- | --- |
| OS | Windows 11（Git Bash 環境） |
| CPU | x86_64（ホスト既定） |
| Rust toolchain | `rustc 1.99.1 (58cf8f9c3 2026-10-13)`（mise 管理） |
| Cargo | mise 経由（`--locked` 使用） |
| Ruviform 解決 rev | `fd1d4d5f`（`Cargo.lock` git 依存記録値） |
| Synvelis HEAD（検証時） | `6c6d1a5abe57d9ca71247621d752b77fe455cb22` + 本 slice 未コミット差分（fixture/docs/evidence/index） |
| doc graph route | 既存の unrelated broken relations により `blocked`/`focus-required` を返し得るため、RCP-03/RCP-06/AUTX-07 と同じ標準 doc chain（test-specs→impl/ir→impl/exp→evidence）で進行 |

## 実行結果（正本）

| # | コマンド | 結果 | exit | 備考 |
| --- | --- | --- | --- | --- |
| 1 | `mise run fmt-check` | Pass | 0 | 初回は新規 test file の整形差分で fail → `cargo fmt -p synvelis-render` 適用後 Pass |
| 2 | `mise run lint synvelis-render` | Pass | 0 | 初回 5 error（`assert_is_empty`×3・`used_underscore_binding`・`match_wildcard_for_single_variants`）→ rcp01/rcp03 先例の記法に修正後 clean |
| 3 | `mise run check synvelis-render` | Pass | 0 | all-targets clean（約 40s） |
| 4 | `mise run test synvelis-render` | Pass | 0 | 全 test target green。`rcp07_gate` **7/7**（0.53s）。既存 target 非回帰: rcp01 11・rcp02 20・rcp03 21・rcp06 系・sf03・tim04・x_e2e・l2_e2e・rcd_04 等全緑 |
| 5 | `mise run test synvelis-player` | Pass | 0 | 全 test target green（rcp_04 resolver 含む — RCP-04 依存先の非回帰証拠） |
| 6 | `mise run test-rt` | Pass | 0 | workspace `rt_` contract 全緑（`rt_midi_delivery_hot_path_alloc_free`・`rt_process_boundary_hot_path_alloc_free`・`rt_trace_adapter_hot_path_alloc_free`・aut_04_rt・tim02_midi_rt 等、filter `rt_` 適用） |
| 7 | `mise run check-headless-deps` | Pass | 0 | `check-headless-deps: no GUI framework deps in 12 headless crates` — 新 test は GUI/GPU/WebView 依存を引き込まない |
| 8 | `mise exec -- prek run --all-files` | Pass | 0 | trailing whitespace・EOF・yaml/toml・line ending・BOM・private key・actionlint 全 Passed |
| 9 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan` → `index apply` | Pass | 0 | `docs/test-specs`（changes=2）・`docs/impl/ir`（changes=1）を apply。`docs/impl/exp` は in-sync（同 index は `.jsonl` log のみ管理 — rcp-01..06 の `.md` exp 先例と同じく対象外）。他 dir 全て in-sync |
| 10 | `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir` / `--kind exp` | IN-SYNC | 0 | preview で両 kind IN-SYNC（apply 不要） |
| 11 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --dir docs/impl/ir` | 本 slice 0 findings | 0 | `rcp-07-cross-gate.md` に finding なし。既存 unrelated findings（vcs-* の defers missing target 等）は main 由来・範囲外 |
| 12 | `node .agents/skills/impl-doc/scripts/audit_experiment_log.js --dir docs/impl/exp` | 本 slice 0 findings | 0 | `rcp-07-cross-gate.md` に finding なし。`issue-147-sccache.jsonl` の Invalid event shape は main 由来の既存問題・範囲外 |
| 13 | `cargo test --locked -p synvelis-render --test rcp07_gate`（補助証拠・反復用） | Pass | 0 | 7/7 green。canonical は #4 の `mise run test` |

## Case → fixture → result 対応（headless Pass 分）

`acceptance-ledger.md` の inventory と 1:1。実測は #4。

| test fn | RCP-GATE | result |
| --- | --- | --- |
| `rcp07_gate01_recipe_key_is_cache_identity_and_false_valid_zero` | GATE-01 | Pass (headless) |
| `rcp07_gate02_closure_invalidation_is_sound_and_complete` | GATE-02 | Pass (headless) |
| `rcp07_gate03_verified_artifact_survives_faults_and_gc_roots` | GATE-03 | Pass (headless・manifest/digest レベルまで) |
| `rcp07_gate04_ondemand_denied_manual_idempotent_cancel_clean` | GATE-04 | Pass (headless) |
| `rcp07_gate05_branch_edit_moves_key_adopt_pins_live` | GATE-05 | Pass (headless, partial — #294 未整備部は Blocked) |
| `rcp07_gate06_store_eligibility_drives_resolver_selection` | GATE-06 | Pass (headless) |
| `rcp07_gate07_job_keys_separate_stale_and_current_recipes` | GATE-07 | Pass (headless contract level) |

## NotRun（実行しなかった lane — 理由と所有 gate）

- 実 Plugin ABI（VST3/CLAP/AU・stateful/fault plugin）: GATE-05/06/07/08/09 — `scarecrow173/ruviform#630`・synvelis#176
- 実 signal/PCM clock・固定 PCM: GATE-07/10 — `scarecrow173/ruviform#628`
- 仮想 OS endpoint: GATE-05/07/08 — `scarecrow173/ruviform#629`
- 実 Window/GPUI runner・実 OS GUI: GATE-06/08 — `scarecrow173/ruviform#631`・synvelis#114/#176
- 実 PCM reader readback・実 remote corrupt・実 ACL/lease: GATE-03 — headless は manifest/digest 検証レベルまで（`scarecrow173/ruviform#628`/`#630` 系）
- AudioLens consumer（f64/長尺/CLI/GUI）: GATE-10 — `scarecrow173/ruviform#294`・`scarecrow173/audio-lens#127`
- RCP-05 専用 test-spec/fixture 未整備部（late attach・2人協業・非RT renderer・Undo/Reload・old commit pin 完全語彙）: GATE-05 — synvelis#294（**Blocked**）
- 実行/製品 Gate: GATE-11 前提の実機評価 — synvelis#23
- 3OS 横断・MSRV・`*-full`・Daily/手動 Full CI: 全 case — POLICIES.md §5・ユーザー指定（MSRV は daily-ci 委譲）

## 既知の範囲外事項（本 slice 起因ではない既存所見）

- `docs/impl/ir` audit: vcs-* 系 file の `defers` missing
  target warning（main 由来・本 slice 範囲外）
- `docs/impl/exp` audit: `issue-147-sccache.jsonl` の Invalid
  event shape error（main 由来・本 slice 範囲外）
- repository 全体の doc lint/graph 関連の既存 blocking
  findings は本 slice の変更と無関係（触った dir の index は
  上記 #9/#10 で in-sync を確認済み）

## 判定規則の確認

- 実測（コマンド・exit code・SHA・toolchain・OS）のない
  Pass は 0 件 — 上表の Pass は全て exit 0 の実測に基づく。
- headless/Fake の Pass を実 Plugin・実 OS・実 GUI・RT・
  consumer 受入へ換算していない（NotRun 表の所有 gate 明記）。
- bit-exact・実 PCM・実 plugin determinism を fixture で
  偽装していない（Issue #296 対象外規定に準拠）。
